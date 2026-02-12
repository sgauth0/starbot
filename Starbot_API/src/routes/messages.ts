// Message routes
import type { FastifyInstance } from 'fastify';
import { z } from 'zod';
import { prisma } from '../db.js';

const CreateMessageSchema = z.object({
  role: z.enum(['user', 'assistant', 'tool', 'system']),
  content: z.string().min(1),
});

export async function messageRoutes(server: FastifyInstance) {
  // GET /v1/chats/:chatId/messages - List messages in a chat
  server.get<{ Params: { chatId: string } }>(
    '/chats/:chatId/messages',
    async (request, reply) => {
      const { chatId } = request.params;

      const messages = await prisma.message.findMany({
        where: { chatId },
        orderBy: { createdAt: 'asc' },
      });

      return { messages };
    }
  );

  // POST /v1/chats/:chatId/messages - Add a message to a chat
  server.post<{ Params: { chatId: string } }>(
    '/chats/:chatId/messages',
    async (request, reply) => {
      const { chatId } = request.params;
      const body = CreateMessageSchema.parse(request.body);

      // Verify chat exists
      const chat = await prisma.chat.findUnique({
        where: { id: chatId },
      });

      if (!chat) {
        return reply.code(404).send({ error: 'Chat not found' });
      }

      const message = await prisma.message.create({
        data: {
          chatId,
          role: body.role,
          content: body.content,
        },
      });

      // Update chat's updatedAt
      await prisma.chat.update({
        where: { id: chatId },
        data: { updatedAt: new Date() },
      });

      return reply.code(201).send({ message });
    }
  );
}
