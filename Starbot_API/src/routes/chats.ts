// Chat routes
import type { FastifyInstance } from 'fastify';
import { z } from 'zod';
import { prisma } from '../db.js';

const CreateChatSchema = z.object({
  title: z.string().min(1).max(255).optional(),
});

export async function chatRoutes(server: FastifyInstance) {
  // GET /v1/projects/:projectId/chats - List chats in a project
  server.get<{ Params: { projectId: string } }>(
    '/projects/:projectId/chats',
    async (request, reply) => {
      const { projectId } = request.params;

      const chats = await prisma.chat.findMany({
        where: { projectId },
        orderBy: { updatedAt: 'desc' },
        include: {
          _count: {
            select: { messages: true },
          },
        },
      });

      return { chats };
    }
  );

  // POST /v1/projects/:projectId/chats - Create a new chat
  server.post<{ Params: { projectId: string } }>(
    '/projects/:projectId/chats',
    async (request, reply) => {
      const { projectId } = request.params;
      const body = CreateChatSchema.parse(request.body);

      // Verify project exists
      const project = await prisma.project.findUnique({
        where: { id: projectId },
      });

      if (!project) {
        return reply.code(404).send({ error: 'Project not found' });
      }

      const chat = await prisma.chat.create({
        data: {
          projectId,
          title: body.title || 'New Chat',
        },
      });

      return reply.code(201).send({ chat });
    }
  );

  // GET /v1/chats/:id - Get a specific chat
  server.get<{ Params: { id: string } }>('/chats/:id', async (request, reply) => {
    const { id } = request.params;

    const chat = await prisma.chat.findUnique({
      where: { id },
      include: {
        project: true,
        messages: {
          orderBy: { createdAt: 'asc' },
        },
      },
    });

    if (!chat) {
      return reply.code(404).send({ error: 'Chat not found' });
    }

    return { chat };
  });

  // DELETE /v1/chats/:id - Delete a chat
  server.delete<{ Params: { id: string } }>('/chats/:id', async (request, reply) => {
    const { id } = request.params;

    try {
      await prisma.chat.delete({
        where: { id },
      });
      return { ok: true };
    } catch (err) {
      return reply.code(404).send({ error: 'Chat not found' });
    }
  });
}
