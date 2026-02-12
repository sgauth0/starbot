import { api } from '../api';
import { Chat, ChatSchema, CreateChatRequest, Message, MessageSchema } from '../types';
import { z } from 'zod';

export const chatsApi = {
  list: (projectId?: string) => {
    const query = projectId ? `?projectId=${projectId}` : '';
    return api.get<Chat[]>(`/chats${query}`, z.array(ChatSchema));
  },
  get: (id: string) => api.get<Chat>(`/chats/${id}`, ChatSchema),
  create: (data: CreateChatRequest) => api.post<Chat>('/chats', data, ChatSchema),
  update: (id: string, data: Partial<Chat>) => api.put<Chat>(`/chats/${id}`, data, ChatSchema),
  delete: (id: string) => api.delete<void>(`/chats/${id}`),
  getMessages: (chatId: string) => api.get<Message[]>(`/chats/${chatId}/messages`, z.array(MessageSchema)),
};
