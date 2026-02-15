/**
 * Inference Route - Legacy compatibility for TUI
 * Wraps the chat-based API in a simpler request/response format
 */

import { FastifyPluginAsync } from 'fastify';
import { z } from 'zod';
import { prisma } from '../db.js';
import { runTriage } from '../services/triage/index.js';
import { getBestModelForTier } from '../services/model-catalog.js';
import { getProvider } from '../providers/index.js';
import { getRelevantContext } from '../services/retrieval.js';

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

export const inferenceRoutes: FastifyPluginAsync = async (server) => {
  // POST /v1/inference/chat - Legacy endpoint for TUI
  server.post('/inference/chat', async (request, reply) => {
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

    // Select model
    const selectedModel = await getBestModelForTier(tier, 'text', true);
    if (!selectedModel) {
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
