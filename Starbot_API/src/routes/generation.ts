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
import { env } from '../env.js';
import { enforceRateLimitIfEnabled, requireAuthIfEnabled } from '../security/route-guards.js';

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

function sortFallbackCandidates(models: ModelDefinition[], targetTier: number): ModelDefinition[] {
  return [...models].sort((a, b) => {
    const aTierDistance = Math.abs(a.tier - targetTier);
    const bTierDistance = Math.abs(b.tier - targetTier);
    if (aTierDistance !== bTierDistance) return aTierDistance - bTierDistance;

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

    if (!requireAuthIfEnabled(request, reply)) {
      return;
    }

    if (!enforceRateLimitIfEnabled(request, reply, {
      routeKey: 'run',
      maxRequests: env.RATE_LIMIT_RUN_PER_WINDOW,
    })) {
      return;
    }

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
        server.log.warn({ err }, 'Memory retrieval failed');
        // Continue without memory if retrieval fails
      }

      sendEvent('status', { message: 'Running triage...' });

      // 2. Run triage on last user message
      const triageResult = runTriage({
        user_message: lastUserMsg.content,
        mode: body.mode,
      });

      const { category, lane, complexity } = triageResult.decision;

      // 3. Map lane to tier (quick=1, standard=2, deep=3)
      const tierMap = { quick: 1, standard: 2, deep: 3 };
      const triageTier = tierMap[lane];
      const requestedTier = tierMap[body.mode];
      const baseTier = body.auto ? triageTier : requestedTier;
      const selectionTier = body.speed ? Math.max(1, baseTier - 1) : baseTier;

      sendEvent('status', {
        message: body.auto
          ? `Routing auto (${category}/${lane}, complexity: ${complexity})...`
          : `Routing manual (${body.mode}, complexity: ${complexity})...`,
      });

      if (body.speed) {
        sendEvent('status', {
          message: 'Speed mode enabled: preferring a faster model tier...',
        });
      }

      // 4. Select model from catalog (respect optional explicit preference)
      const primaryModel = await resolveRequestedModel(selectionTier, 'text', body.model_prefs);
      if (!primaryModel) {
        throw new Error('No models available. Please configure at least one provider.');
      }

      const fallbackPool = await listModels({
        status: 'enabled',
        capability: 'text',
        configuredOnly: true,
      });
      const fallbackCandidates = sortFallbackCandidates(
        fallbackPool.filter((model) => model.id !== primaryModel.id),
        selectionTier,
      );
      const candidateModels = [primaryModel, ...fallbackCandidates];

      // 5. Convert messages to provider format and inject memory
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

      // 6. Stream response from provider with automatic model/provider failover
      let fullResponse = '';
      let usage = { promptTokens: 0, completionTokens: 0, totalTokens: 0 };
      let selectedModel: ModelDefinition | null = null;
      let lastProviderError: unknown = null;
      const blockedProviders = new Set<string>();

      for (const candidate of candidateModels) {
        if (blockedProviders.has(candidate.provider)) {
          continue;
        }

        sendEvent('status', {
          message: `Using ${candidate.displayName} (${candidate.provider})...`,
        });

        try {
          const provider = getProvider(candidate.provider);
          fullResponse = '';
          usage = { promptTokens: 0, completionTokens: 0, totalTokens: 0 };

          for await (const chunk of provider.sendChatStream(providerMessages, {
            model: candidate.deploymentName,
            maxTokens: body.speed
              ? Math.min(candidate.maxOutputTokens, 1024)
              : candidate.maxOutputTokens,
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

          if (!fullResponse.trim()) {
            throw new Error(`Model "${candidate.displayName}" returned an empty response`);
          }

          selectedModel = candidate;
          break;
        } catch (err) {
          lastProviderError = err;
          const errorMessage = err instanceof Error ? err.message : String(err);
          if (/(googleautherror|invalid authentication|unauthorized|no route for that uri|not configured|api key|403|401)/i.test(errorMessage)) {
            blockedProviders.add(candidate.provider);
          }
          server.log.warn(
            { err, provider: candidate.provider, model: candidate.deploymentName },
            'Model run failed, trying fallback',
          );
          sendEvent('status', {
            message: `${candidate.displayName} unavailable, trying fallback...`,
          });
        }
      }

      if (!selectedModel) {
        throw (
          lastProviderError instanceof Error
            ? lastProviderError
            : new Error('All configured models failed to respond')
        );
      }

      // 7. Save assistant message
      const assistantMessage = await prisma.message.create({
        data: {
          chatId,
          role: 'assistant',
          content: fullResponse,
        },
      });

      // 8. Update chat title if needed
      const newTitle = chat.title === 'New Chat'
        ? lastUserMsg.content.slice(0, 50) + (lastUserMsg.content.length > 50 ? '...' : '')
        : chat.title;

      const updatedAt = new Date();

      await prisma.chat.update({
        where: { id: chatId },
        data: {
          updatedAt,
          title: newTitle,
        },
      });

      // 9. Send final event
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
        title: newTitle,
        updatedAt: updatedAt.toISOString(),
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
