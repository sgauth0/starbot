// Message routes
import type { FastifyInstance } from 'fastify';
import { z } from 'zod';
import { prisma } from '../db.js';

const CreateMessageSchema = z.object({
  role: z.enum(['user', 'assistant', 'tool', 'system']),
  content: z.string().min(1),
});

const UpdateMessageSchema = z.object({
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

  // PUT /v1/messages/:id - Update an existing message
  server.put<{ Params: { id: string } }>(
    '/messages/:id',
    async (request, reply) => {
      const { id } = request.params;
      const body = UpdateMessageSchema.parse(request.body);

      const existingMessage = await prisma.message.findUnique({
        where: { id },
        select: { id: true, chatId: true },
      });

      if (!existingMessage) {
        return reply.code(404).send({ error: 'Message not found' });
      }

      const updatedAt = new Date();
      const [message] = await prisma.$transaction([
        prisma.message.update({
          where: { id },
          data: {
            content: body.content,
          },
        }),
        prisma.chat.update({
          where: { id: existingMessage.chatId },
          data: { updatedAt },
        }),
      ]);

      return { message };
    }
  );

  // DELETE /v1/messages/:id - Delete a single message
  server.delete<{ Params: { id: string } }>(
    '/messages/:id',
    async (request, reply) => {
      const { id } = request.params;

      const existingMessage = await prisma.message.findUnique({
        where: { id },
        select: { id: true, chatId: true },
      });

      if (!existingMessage) {
        return reply.code(404).send({ error: 'Message not found' });
      }

      const updatedAt = new Date();
      await prisma.$transaction([
        prisma.message.delete({
          where: { id },
        }),
        prisma.chat.update({
          where: { id: existingMessage.chatId },
          data: { updatedAt },
        }),
      ]);

      return { ok: true };
    }
  );

  // DELETE /v1/chats/:chatId/messages/after/:messageId - Delete this message and all subsequent messages
  server.delete<{ Params: { chatId: string; messageId: string } }>(
    '/chats/:chatId/messages/after/:messageId',
    async (request, reply) => {
      const { chatId, messageId } = request.params;

      const target = await prisma.message.findUnique({
        where: { id: messageId },
        select: { id: true, chatId: true },
      });

      if (!target || target.chatId !== chatId) {
        return reply.code(404).send({ error: 'Message not found in chat' });
      }

      const orderedMessages = await prisma.message.findMany({
        where: { chatId },
        orderBy: [{ createdAt: 'asc' }, { id: 'asc' }],
        select: { id: true },
      });

      const startIndex = orderedMessages.findIndex((m) => m.id === messageId);
      if (startIndex < 0) {
        return reply.code(404).send({ error: 'Message not found in chat' });
      }

      const idsToDelete = orderedMessages.slice(startIndex).map((m) => m.id);
      const updatedAt = new Date();

      const [deleteResult] = await prisma.$transaction([
        prisma.message.deleteMany({
          where: { id: { in: idsToDelete } },
        }),
        prisma.chat.update({
          where: { id: chatId },
          data: { updatedAt },
        }),
      ]);

      return {
        ok: true,
        deleted: deleteResult.count,
      };
    }
  );
}
