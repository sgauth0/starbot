/**
 * Retrieval Service
 * Performs semantic search using cosine similarity
 */

import { prisma } from '../db.js';
import { generateEmbedding } from './embeddings.js';

export interface RetrievalResult {
  chunkId: string;
  text: string;
  similarity: number;
  memoryId: string;
  scope: string;
}

/**
 * Calculate cosine similarity between two vectors
 */
function cosineSimilarity(a: number[], b: number[]): number {
  if (a.length !== b.length) {
    throw new Error('Vectors must have the same length');
  }

  let dotProduct = 0;
  let normA = 0;
  let normB = 0;

  for (let i = 0; i < a.length; i++) {
    dotProduct += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }

  normA = Math.sqrt(normA);
  normB = Math.sqrt(normB);

  if (normA === 0 || normB === 0) {
    return 0;
  }

  return dotProduct / (normA * normB);
}

/**
 * Search for relevant memory chunks using semantic similarity
 *
 * @param query - The search query text
 * @param projectId - Project ID to scope search
 * @param workspaceId - Optional workspace ID to scope search
 * @param topK - Number of top results to return
 * @param minSimilarity - Minimum similarity threshold (0-1)
 */
export async function searchMemory(
  query: string,
  projectId: string,
  workspaceId?: string,
  topK = 5,
  minSimilarity = 0.5
): Promise<RetrievalResult[]> {
  // Generate query embedding
  const queryEmbedding = await generateEmbedding(query);
  if (!queryEmbedding) {
    console.warn('Could not generate query embedding');
    return [];
  }

  // Get all memory documents for this project/workspace
  const memoryDocs = await prisma.memoryDocument.findMany({
    where: {
      OR: [
        { projectId, scope: 'project' },
        ...(workspaceId ? [{ workspaceId, scope: 'workspace' }] : []),
      ],
    },
    include: {
      chunks: true,
    },
  });

  // Calculate similarities for all chunks with embeddings
  const results: RetrievalResult[] = [];

  for (const doc of memoryDocs) {
    for (const chunk of doc.chunks) {
      if (!chunk.embeddingVector) {
        continue; // Skip chunks without embeddings
      }

      try {
        const chunkEmbedding = JSON.parse(chunk.embeddingVector);
        const similarity = cosineSimilarity(queryEmbedding, chunkEmbedding);

        if (similarity >= minSimilarity) {
          results.push({
            chunkId: chunk.id,
            text: chunk.text,
            similarity,
            memoryId: doc.id,
            scope: doc.scope,
          });
        }
      } catch (error) {
        console.error(`Error parsing embedding for chunk ${chunk.id}:`, error);
      }
    }
  }

  // Sort by similarity (highest first) and take top K
  results.sort((a, b) => b.similarity - a.similarity);
  return results.slice(0, topK);
}

/**
 * Get relevant context from memory for a chat
 * Combines project and workspace memory
 */
export async function getRelevantContext(
  query: string,
  projectId: string,
  workspaceId?: string,
  maxChunks = 5
): Promise<string> {
  const results = await searchMemory(query, projectId, workspaceId, maxChunks);

  if (results.length === 0) {
    return '';
  }

  // Format results into context
  let context = '# Relevant Memory\n\n';

  for (const result of results) {
    context += `## From ${result.scope === 'project' ? 'Project' : 'Workspace'} Memory (${(result.similarity * 100).toFixed(1)}% match)\n\n`;
    context += `${result.text}\n\n`;
  }

  return context;
}

/**
 * Search within a specific memory document
 */
export async function searchInMemory(
  memoryId: string,
  query: string,
  topK = 3,
  minSimilarity = 0.5
): Promise<RetrievalResult[]> {
  const queryEmbedding = await generateEmbedding(query);
  if (!queryEmbedding) {
    return [];
  }

  const chunks = await prisma.memoryChunk.findMany({
    where: { memoryId },
    include: {
      memory: true,
    },
  });

  const results: RetrievalResult[] = [];

  for (const chunk of chunks) {
    if (!chunk.embeddingVector) {
      continue;
    }

    try {
      const chunkEmbedding = JSON.parse(chunk.embeddingVector);
      const similarity = cosineSimilarity(queryEmbedding, chunkEmbedding);

      if (similarity >= minSimilarity) {
        results.push({
          chunkId: chunk.id,
          text: chunk.text,
          similarity,
          memoryId: chunk.memory.id,
          scope: chunk.memory.scope,
        });
      }
    } catch (error) {
      console.error(`Error parsing embedding for chunk ${chunk.id}:`, error);
    }
  }

  results.sort((a, b) => b.similarity - a.similarity);
  return results.slice(0, topK);
}
