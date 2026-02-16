import { api } from '../api';
import { Message, MessageSchema } from '../types';
import { z } from 'zod';

// API response wrapper schema
const MessageResponseSchema = z.object({
  message: MessageSchema,
});

export const messagesApi = {
  send: async (chatId: string, content: string, role: 'user' | 'assistant' | 'system' = 'user') => {
    const response = await api.post(
      `/chats/${chatId}/messages`,
      { role, content },
      MessageResponseSchema
    );
    return response.message;
  },

  update: async (id: string, data: Partial<Message>) => {
    const response = await api.put(`/messages/${id}`, data, MessageResponseSchema);
    return response.message;
  },

  delete: async (id: string) => {
    await api.delete(`/messages/${id}`);
  },

  deleteAfter: async (chatId: string, messageId: string) => {
    await api.delete(`/chats/${chatId}/messages/after/${messageId}`);
  },
};
