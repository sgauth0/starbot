// Generation route (streaming with real model routing)
import type { FastifyInstance } from 'fastify';
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

const RunChatSchema = z.object({
  mode: z.enum(['quick', 'standard', 'deep']).optional().default('standard'),
  model_prefs: z.string().optional(),
  speed: z.boolean().optional().default(false),
  auto: z.boolean().optional().default(true),
});

interface RunParams {
  Params: {
    chatId: string;
  };
}

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

export async function generationRoutes(server: FastifyInstance) {
  // POST /v1/chats/:chatId/run - Start generation (SSE streaming)
  server.post<RunParams>('/chats/:chatId/run', async (request, reply) => {
    const { chatId } = request.params;
    const body = RunChatSchema.parse(request.body);

    // Verify chat exists
    const chat = await prisma.chat.findUnique({
      where: { id: chatId },
      include: {
        messages: {
          orderBy: { createdAt: 'asc' },
          take: 50, // Last 50 messages for context
        },
        project: true,
        workspace: true,
      },
    });

    if (!chat) {
      return reply.code(404).send({ error: 'Chat not found' });
    }

    // Set up SSE streaming
    reply.raw.setHeader('Content-Type', 'text/event-stream');
    reply.raw.setHeader('Cache-Control', 'no-cache');
    reply.raw.setHeader('Connection', 'keep-alive');

    // Helper to send SSE events
    const sendEvent = (type: string, data: any) => {
      reply.raw.write(`event: ${type}\n`);
      reply.raw.write(`data: ${JSON.stringify(data)}\n\n`);
    };

    try {
      // 0. Get last user message for memory retrieval
      const lastUserMsg = chat.messages.filter(m => m.role === 'user').pop();
      if (!lastUserMsg) {
        throw new Error('No user message found in chat');
      }

      // 1. Retrieve relevant memory context
      sendEvent('status', { message: 'Retrieving relevant memory...' });

      let memoryContext = '';
      try {
        memoryContext = await getRelevantContext(
          lastUserMsg.content,
          chat.projectId,
          chat.workspaceId || undefined,
          5 // Top 5 most relevant chunks
        );
      } catch (err) {
        server.log.warn('Memory retrieval failed:', err);
        // Continue without memory if retrieval fails
      }

      sendEvent('status', { message: 'Running triage...' });

      // 2. Run triage on last user message
      const triageResult = runTriage({
        user_message: lastUserMsg.content,
        mode: body.mode,
      });

      const { category, lane, complexity } = triageResult.decision;

      sendEvent('status', {
        message: `Routing (${category}/${lane}, complexity: ${complexity})...`,
      });

      // 3. Map lane to tier (quick=1, standard=2, deep=3)
      const tierMap = { quick: 1, standard: 2, deep: 3 };
      const tier = tierMap[lane];

      // 4. Select model from catalog (respect optional explicit preference)
      const selectedModel = await resolveRequestedModel(tier, 'text', body.model_prefs);

      if (!selectedModel) {
        throw new Error('No models available. Please configure at least one provider.');
      }

      sendEvent('status', {
        message: `Using ${selectedModel.displayName} (${selectedModel.provider})...`,
      });

      // 5. Get provider
      const provider = getProvider(selectedModel.provider);

      // 6. Convert messages to provider format and inject memory
      const providerMessages: Array<{ role: 'user' | 'assistant' | 'system'; content: string }> = [];

      // Inject memory context as system message if available
      if (memoryContext) {
        providerMessages.push({
          role: 'system',
          content: memoryContext,
        });
      }

      // Add conversation messages
      providerMessages.push(...chat.messages.map(m => ({
        role: m.role as 'user' | 'assistant' | 'system',
        content: m.content,
      })));

      // 7. Stream response from provider
      let fullResponse = '';
      let usage = { promptTokens: 0, completionTokens: 0, totalTokens: 0 };

      for await (const chunk of provider.sendChatStream(providerMessages, {
        model: selectedModel.deploymentName,
        maxTokens: selectedModel.maxOutputTokens,
        temperature: 0.7,
      })) {
        if (chunk.text) {
          fullResponse += chunk.text;
          sendEvent('token.delta', { text: chunk.text });
        }

        if (chunk.usage) {
          usage = chunk.usage;
        }
      }

      // 8. Save assistant message
      const assistantMessage = await prisma.message.create({
        data: {
          chatId,
          role: 'assistant',
          content: fullResponse,
        },
      });

      // 9. Update chat title if needed
      await prisma.chat.update({
        where: { id: chatId },
        data: {
          updatedAt: new Date(),
          title: chat.title === 'New Chat'
            ? lastUserMsg.content.slice(0, 50) + (lastUserMsg.content.length > 50 ? '...' : '')
            : chat.title,
        },
      });

      // 10. Send final event
      sendEvent('message.final', {
        id: assistantMessage.id,
        role: 'assistant',
        content: fullResponse,
        provider: selectedModel.provider,
        model: selectedModel.deploymentName,
        modelDisplayName: selectedModel.displayName,
        usage: {
          promptTokens: usage.promptTokens,
          completionTokens: usage.completionTokens,
          totalTokens: usage.totalTokens,
        },
        triage: {
          category,
          lane,
          complexity,
          elapsed_ms: triageResult.elapsed_ms,
        },
      });

      sendEvent('chat.updated', {
        id: chatId,
        title: chat.title,
        updatedAt: new Date().toISOString(),
      });

      reply.raw.end();
    } catch (err) {
      server.log.error(err);
      sendEvent('error', {
        message: err instanceof Error ? err.message : 'Unknown error',
        fatal: true,
      });
      reply.raw.end();
    }
  });

  // POST /v1/chats/:chatId/cancel - Cancel ongoing generation
  server.post<RunParams>('/chats/:chatId/cancel', async (request, reply) => {
    const { chatId } = request.params;

    // TODO: Implement cancellation logic with AbortController
    // For now, just return success
    return { ok: true, message: 'Cancellation not yet implemented' };
  });
}
