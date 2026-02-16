/**
 * Inference Route - Legacy compatibility for TUI
 * Wraps the chat-based API in a simpler request/response format
 */

import { FastifyPluginAsync } from 'fastify';
import { z } from 'zod';
import { prisma } from '../db.js';
import { runTriage } from '../services/triage/index.js';
import {
  getBestModelForTier,
  getModelById,
  getModelByProviderAndName,
  listModels,
  type ModelDefinition,
} from '../services/model-catalog.js';
import { getProvider } from '../providers/index.js';
import { getRelevantContext } from '../services/retrieval.js';
import { env } from '../env.js';
import { enforceRateLimitIfEnabled, requireAuthIfEnabled } from '../security/route-guards.js';

const InferenceRequestSchema = z.object({
  messages: z.array(z.object({
    role: z.enum(['user', 'assistant', 'system']),
    content: z.string(),
  })),
  client: z.string().optional(),
  provider: z.string().optional(),
  model: z.string().optional(),
  max_tokens: z.number().optional(),
  conversationId: z.string().optional(),
});

const KNOWN_PROVIDERS = new Set(['kimi', 'vertex', 'azure', 'bedrock', 'cloudflare']);

function parseModelPrefs(raw?: string): { provider?: string; model?: string } {
  const trimmed = String(raw || '').trim();
  if (!trimmed) return {};
  if (trimmed.includes(':')) {
    const [providerRaw, modelRaw] = trimmed.split(':', 2);
    const provider = providerRaw.trim().toLowerCase();
    const model = modelRaw.trim();
    return {
      provider: provider || undefined,
      model: model || undefined,
    };
  }
  const lower = trimmed.toLowerCase();
  if (KNOWN_PROVIDERS.has(lower)) {
    return { provider: lower };
  }
  return { model: trimmed };
}

function sortByCost(models: ModelDefinition[]): ModelDefinition[] {
  return [...models].sort((a, b) => {
    const aCost = a.costPer1kInput || Number.POSITIVE_INFINITY;
    const bCost = b.costPer1kInput || Number.POSITIVE_INFINITY;
    return aCost - bCost;
  });
}

async function resolveRequestedModel(
  tier: number,
  capability: string,
  modelPrefs?: string,
): Promise<ModelDefinition | null> {
  const prefs = parseModelPrefs(modelPrefs);

  if (prefs.model) {
    if (prefs.provider) {
      const exact = await getModelByProviderAndName(prefs.provider, prefs.model);
      if (exact && exact.status === 'enabled') return exact;
    }

    const byId = await getModelById(prefs.model);
    if (byId && byId.status === 'enabled' && (!prefs.provider || byId.provider === prefs.provider)) {
      return byId;
    }

    const all = await listModels({
      status: 'enabled',
      capability,
      configuredOnly: true,
      ...(prefs.provider ? { provider: prefs.provider } : {}),
    });
    const byDeployment = all.find(m => m.deploymentName === prefs.model);
    if (byDeployment) return byDeployment;
  }

  if (prefs.provider) {
    const atTier = await listModels({
      status: 'enabled',
      tier,
      capability,
      configuredOnly: true,
      provider: prefs.provider,
    });
    if (atTier.length > 0) return sortByCost(atTier)[0];

    const anyTier = await listModels({
      status: 'enabled',
      capability,
      configuredOnly: true,
      provider: prefs.provider,
    });
    if (anyTier.length > 0) return sortByCost(anyTier)[0];
  }

  return getBestModelForTier(tier, capability, true);
}

function buildModelPrefs(provider?: string, model?: string): string | undefined {
  const providerTrimmed = String(provider || '').trim().toLowerCase();
  const modelTrimmed = String(model || '').trim();

  if (providerTrimmed && providerTrimmed !== 'auto' && modelTrimmed) {
    return `${providerTrimmed}:${modelTrimmed}`;
  }

  if (providerTrimmed && providerTrimmed !== 'auto') {
    return providerTrimmed;
  }

  if (modelTrimmed) {
    return modelTrimmed;
  }

  return undefined;
}

export const inferenceRoutes: FastifyPluginAsync = async (server) => {
  // POST /v1/inference/chat - Legacy endpoint for TUI
  server.post('/inference/chat', async (request, reply) => {
    if (!requireAuthIfEnabled(request, reply)) {
      return;
    }

    if (!enforceRateLimitIfEnabled(request, reply, {
      routeKey: 'inference_chat',
      maxRequests: env.RATE_LIMIT_INFERENCE_PER_WINDOW,
    })) {
      return;
    }

    const body = InferenceRequestSchema.parse(request.body);

    // Find or create a default project for CLI usage
    let project = await prisma.project.findFirst({
      where: { name: 'CLI Default' },
    });

    if (!project) {
      project = await prisma.project.create({
        data: { name: 'CLI Default' },
      });
    }

    // Find or create chat
    let chat;
    if (body.conversationId) {
      chat = await prisma.chat.findUnique({
        where: { id: body.conversationId },
        include: { messages: { orderBy: { createdAt: 'asc' } } },
      });
    }

    if (!chat) {
      chat = await prisma.chat.create({
        data: {
          projectId: project.id,
          title: 'CLI Chat',
        },
        include: { messages: true },
      });
    }

    // Add user messages from request
    for (const msg of body.messages) {
      await prisma.message.create({
        data: {
          chatId: chat.id,
          role: msg.role,
          content: msg.content,
        },
      });
    }

    // Get latest messages for context
    const messages = await prisma.message.findMany({
      where: { chatId: chat.id },
      orderBy: { createdAt: 'asc' },
    });

    const lastUserMsg = messages.filter(m => m.role === 'user').pop();
    if (!lastUserMsg) {
      return reply.code(400).send({ error: 'No user message found' });
    }

    // Run triage
    const triageResult = runTriage({
      user_message: lastUserMsg.content,
      mode: 'standard',
    });

    const tierMap = { quick: 1, standard: 2, deep: 3 };
    const tier = tierMap[triageResult.decision.lane];

    const modelPrefs = buildModelPrefs(body.provider, body.model);
    const requestedExplicitly = !!modelPrefs;

    // Select model (honor provider/model preferences when supplied)
    const selectedModel = await resolveRequestedModel(tier, 'text', modelPrefs);
    if (!selectedModel) {
      if (requestedExplicitly) {
        return reply.code(400).send({
          error: 'Requested provider/model is not available',
        });
      }
      return reply.code(500).send({ error: 'No models available' });
    }

    // Get provider
    const provider = getProvider(selectedModel.provider);

    // Get memory context
    let memoryContext = '';
    try {
      memoryContext = await getRelevantContext(
        lastUserMsg.content,
        project.id,
        undefined,
        5
      );
    } catch (err) {
      // Continue without memory
    }

    // Build provider messages
    const providerMessages: Array<{ role: 'user' | 'assistant' | 'system'; content: string }> = [];

    if (memoryContext) {
      providerMessages.push({ role: 'system', content: memoryContext });
    }

    providerMessages.push(...messages.map(m => ({
      role: m.role as 'user' | 'assistant' | 'system',
      content: m.content,
    })));

    // Generate response (non-streaming for CLI)
    let fullResponse = '';
    let usage = { promptTokens: 0, completionTokens: 0, totalTokens: 0 };

    for await (const chunk of provider.sendChatStream(providerMessages, {
      model: selectedModel.deploymentName,
      maxTokens: body.max_tokens || selectedModel.maxOutputTokens,
      temperature: 0.7,
    })) {
      if (chunk.text) {
        fullResponse += chunk.text;
      }
      if (chunk.usage) {
        usage = chunk.usage;
      }
    }

    // Save assistant message
    await prisma.message.create({
      data: {
        chatId: chat.id,
        role: 'assistant',
        content: fullResponse,
      },
    });

    // Return response in expected format
    return {
      reply: fullResponse,
      conversation_id: chat.id,
      provider: selectedModel.provider,
      model: selectedModel.deploymentName,
      usage: {
        prompt_tokens: usage.promptTokens,
        completion_tokens: usage.completionTokens,
        total_tokens: usage.totalTokens,
      },
    };
  });
};
