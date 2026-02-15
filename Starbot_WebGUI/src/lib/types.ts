import { z } from 'zod';

export const ProjectSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string().optional(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export const ChatSchema = z.object({
  id: z.string(),
  projectId: z.string().optional(),
  title: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export const MessageSchema = z.object({
  id: z.string(),
  chatId: z.string(),
  role: z.enum(['user', 'assistant', 'system', 'tool']),
  content: z.string(),
  createdAt: z.string(),
  metadata: z.record(z.string(), z.any()).optional(),
});

export const SettingsSchema = z.object({
  mode: z.enum(['quick', 'standard', 'deep']).default('standard'),
  auto: z.boolean().default(true),
  speed: z.boolean().default(false), // true = fast mode, false = quality mode
  model_prefs: z.string().optional(),
});

export type Project = z.infer<typeof ProjectSchema>;
export type Chat = z.infer<typeof ChatSchema>;
export type Message = z.infer<typeof MessageSchema>;
export type Settings = z.infer<typeof SettingsSchema>;

export interface CreateChatRequest {
    title?: string;
    projectId?: string;
}

export interface SendMessageRequest {
    content: string;
    chatId: string;
    settings?: Partial<Settings>;
}
