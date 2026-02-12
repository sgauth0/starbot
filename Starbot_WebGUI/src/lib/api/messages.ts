import { api } from '../api';
import { Message, MessageSchema, SendMessageRequest } from '../types';

export const messagesApi = {
  send: (data: SendMessageRequest) => api.post<Message>('/messages', data, MessageSchema),
  update: (id: string, data: Partial<Message>) => api.put<Message>(`/messages/${id}`, data, MessageSchema),
};
