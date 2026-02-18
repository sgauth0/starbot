import { z } from 'zod';
import { api } from '../api';

const MemorySchema = z.object({
  id: z.string(),
  content: z.string(),
  updatedAt: z.string(),
});

const MemoryEnvelopeSchema = z.object({
  memory: MemorySchema,
});

export type MemoryDocument = z.infer<typeof MemorySchema>;

export const memoryApi = {
  getIdentity: async () => {
    const response = await api.get('/identity', MemoryEnvelopeSchema);
    return response.memory;
  },

  updateIdentity: async (content: string) => {
    const response = await api.put('/identity', { content }, MemoryEnvelopeSchema);
    return response.memory;
  },

  processIdentity: async () => {
    return api.post('/identity/process', {});
  },

  getChatMemory: async (chatId: string) => {
    const response = await api.get(`/chats/${chatId}/memory`, MemoryEnvelopeSchema);
    return response.memory;
  },

  updateChatMemory: async (chatId: string, content: string) => {
    const response = await api.put(`/chats/${chatId}/memory`, { content }, MemoryEnvelopeSchema);
    return response.memory;
  },

  processChatMemory: async (chatId: string) => {
    return api.post(`/chats/${chatId}/memory/process`, {});
  },
};
