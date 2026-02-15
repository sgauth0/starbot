import { FastifyPluginAsync } from 'fastify';
import { prisma } from '../db.js';
import { processContent } from '../services/chunking.js';
import { generateEmbeddingsBatch, areEmbeddingsAvailable } from '../services/embeddings.js';

const DEFAULT_PMEMORY = `# Project Memory

This document stores project-level context and conventions.

## Purpose
- Document architectural decisions
- Store coding conventions
- Track important project-wide patterns

## Guidelines
- Keep content organized with markdown headings
- Update this document as the project evolves
- Use this to guide AI assistants about project preferences
`;

const DEFAULT_MEMORY = `# Workspace Memory

This document stores workspace-specific context.

## Purpose
- Document workspace structure
- Store frequently used commands
- Track workspace-specific patterns

## Guidelines
- Keep content focused on this workspace
- Update as you discover patterns
- Use this to guide context-aware operations
`;

export const memoryRoutes: FastifyPluginAsync = async (server) => {
  // Get project memory (PMEMORY.md)
  server.get('/projects/:projectId/memory', async (request, reply) => {
    const { projectId } = request.params as { projectId: string };

    // Verify project exists
    const project = await prisma.project.findUnique({
      where: { id: projectId },
    });

    if (!project) {
      return reply.status(404).send({ error: 'Project not found' });
    }

    // Find or create memory document
    let memory = await prisma.memoryDocument.findFirst({
      where: {
        scope: 'project',
        projectId,
        workspaceId: null,
      },
    });

    if (!memory) {
      // Auto-create with default content
      memory = await prisma.memoryDocument.create({
        data: {
          scope: 'project',
          projectId,
          workspaceId: null,
          content: DEFAULT_PMEMORY,
        },
      });
    }

    return {
      memory: {
        id: memory.id,
        content: memory.content,
        updatedAt: memory.updatedAt,
      },
    };
  });

  // Update project memory
  server.put('/projects/:projectId/memory', async (request, reply) => {
    const { projectId } = request.params as { projectId: string };
    const { content } = request.body as { content: string };

    // Verify project exists
    const project = await prisma.project.findUnique({
      where: { id: projectId },
    });

    if (!project) {
      return reply.status(404).send({ error: 'Project not found' });
    }

    // Upsert memory document
    let memory = await prisma.memoryDocument.findFirst({
      where: {
        scope: 'project',
        projectId,
        workspaceId: null,
      },
    });

    if (memory) {
      memory = await prisma.memoryDocument.update({
        where: { id: memory.id },
        data: { content },
      });
    } else {
      memory = await prisma.memoryDocument.create({
        data: {
          scope: 'project',
          projectId,
          workspaceId: null,
          content,
        },
      });
    }

    return {
      memory: {
        id: memory.id,
        content: memory.content,
        updatedAt: memory.updatedAt,
      },
    };
  });

  // Get workspace memory (MEMORY.md)
  server.get('/workspaces/:workspaceId/memory', async (request, reply) => {
    const { workspaceId } = request.params as { workspaceId: string };

    // Verify workspace exists
    const workspace = await prisma.workspace.findUnique({
      where: { id: workspaceId },
    });

    if (!workspace) {
      return reply.status(404).send({ error: 'Workspace not found' });
    }

    // Find or create memory document
    let memory = await prisma.memoryDocument.findFirst({
      where: {
        scope: 'workspace',
        projectId: null,
        workspaceId,
      },
    });

    if (!memory) {
      // Auto-create with default content
      memory = await prisma.memoryDocument.create({
        data: {
          scope: 'workspace',
          projectId: null,
          workspaceId,
          content: DEFAULT_MEMORY,
        },
      });
    }

    return {
      memory: {
        id: memory.id,
        content: memory.content,
        updatedAt: memory.updatedAt,
      },
    };
  });

  // Update workspace memory
  server.put('/workspaces/:workspaceId/memory', async (request, reply) => {
    const { workspaceId } = request.params as { workspaceId: string };
    const { content } = request.body as { content: string };

    // Verify workspace exists
    const workspace = await prisma.workspace.findUnique({
      where: { id: workspaceId },
    });

    if (!workspace) {
      return reply.status(404).send({ error: 'Workspace not found' });
    }

    // Upsert memory document
    let memory = await prisma.memoryDocument.findFirst({
      where: {
        scope: 'workspace',
        projectId: null,
        workspaceId,
      },
    });

    if (memory) {
      memory = await prisma.memoryDocument.update({
        where: { id: memory.id },
        data: { content },
      });
    } else {
      memory = await prisma.memoryDocument.create({
        data: {
          scope: 'workspace',
          projectId: null,
          workspaceId,
          content,
        },
      });
    }

    return {
      memory: {
        id: memory.id,
        content: memory.content,
        updatedAt: memory.updatedAt,
      },
    };
  });

  // Process project memory (generate chunks and embeddings)
  server.post('/projects/:projectId/memory/process', async (request, reply) => {
    const { projectId } = request.params as { projectId: string };

    // Get memory document
    const memory = await prisma.memoryDocument.findFirst({
      where: {
        scope: 'project',
        projectId,
        workspaceId: null,
      },
    });

    if (!memory) {
      return reply.status(404).send({ error: 'Memory document not found' });
    }

    // Delete existing chunks
    await prisma.memoryChunk.deleteMany({
      where: { memoryId: memory.id },
    });

    // Chunk the content
    const chunks = processContent(memory.content);

    if (chunks.length === 0) {
      return { status: 'success', chunks: 0, embeddings: 0 };
    }

    // Generate embeddings if available
    let embeddingsGenerated = 0;
    const embeddings = areEmbeddingsAvailable()
      ? await generateEmbeddingsBatch(chunks.map((c) => c.text))
      : chunks.map(() => null);

    // Store chunks with embeddings using bulk insert
    const chunkData = chunks.map((chunk, i) => {
      const embedding = embeddings[i];
      if (embedding) {
        embeddingsGenerated++;
      }
      return {
        memoryId: memory.id,
        text: chunk.text,
        embeddingVector: embedding ? JSON.stringify(embedding) : null,
      };
    });

    await prisma.memoryChunk.createMany({
      data: chunkData,
    });

    return {
      status: 'success',
      chunks: chunks.length,
      embeddings: embeddingsGenerated,
    };
  });

  // Process workspace memory
  server.post('/workspaces/:workspaceId/memory/process', async (request, reply) => {
    const { workspaceId } = request.params as { workspaceId: string };

    // Get memory document
    const memory = await prisma.memoryDocument.findFirst({
      where: {
        scope: 'workspace',
        projectId: null,
        workspaceId,
      },
    });

    if (!memory) {
      return reply.status(404).send({ error: 'Memory document not found' });
    }

    // Delete existing chunks
    await prisma.memoryChunk.deleteMany({
      where: { memoryId: memory.id },
    });

    // Chunk the content
    const chunks = processContent(memory.content);

    if (chunks.length === 0) {
      return { status: 'success', chunks: 0, embeddings: 0 };
    }

    // Generate embeddings if available
    let embeddingsGenerated = 0;
    const embeddings = areEmbeddingsAvailable()
      ? await generateEmbeddingsBatch(chunks.map((c) => c.text))
      : chunks.map(() => null);

    // Store chunks with embeddings using bulk insert
    const chunkData = chunks.map((chunk, i) => {
      const embedding = embeddings[i];
      if (embedding) {
        embeddingsGenerated++;
      }
      return {
        memoryId: memory.id,
        text: chunk.text,
        embeddingVector: embedding ? JSON.stringify(embedding) : null,
      };
    });

    await prisma.memoryChunk.createMany({
      data: chunkData,
    });

    return {
      status: 'success',
      chunks: chunks.length,
      embeddings: embeddingsGenerated,
    };
  });

  // Search memory
  server.post('/projects/:projectId/memory/search', async (request, reply) => {
    const { projectId } = request.params as { projectId: string };
    const { query, workspaceId, topK = 5 } = request.body as {
      query: string;
      workspaceId?: string;
      topK?: number;
    };

    const { searchMemory } = await import('../services/retrieval.js');
    const results = await searchMemory(query, projectId, workspaceId, topK);

    return { results };
  });
};
