import { api } from '../api';
import { Chat, ChatSchema, CreateChatRequest, Message, MessageSchema } from '../types';
import { z } from 'zod';

// API response wrapper schemas
const ChatsResponseSchema = z.object({
  chats: z.array(ChatSchema),
});

const ChatResponseSchema = z.object({
  chat: ChatSchema,
});

const MessagesResponseSchema = z.object({
  messages: z.array(MessageSchema),
});

export const chatsApi = {
  list: async (projectId: string) => {
    const response = await api.get(`/projects/${projectId}/chats`, ChatsResponseSchema);
    return response.chats;
  },

  get: async (id: string) => {
    const response = await api.get(`/chats/${id}`, ChatResponseSchema);
    return response.chat;
  },

  create: async (projectId: string, data: CreateChatRequest) => {
    const response = await api.post(`/projects/${projectId}/chats`, data, ChatResponseSchema);
    return response.chat;
  },

  update: async (id: string, data: Partial<Chat>) => {
    const response = await api.put(`/chats/${id}`, data, ChatResponseSchema);
    return response.chat;
  },

  delete: (id: string) => api.delete<void>(`/chats/${id}`),

  getMessages: async (chatId: string) => {
    const response = await api.get(`/chats/${chatId}/messages`, MessagesResponseSchema);
    return response.messages;
  },
};
