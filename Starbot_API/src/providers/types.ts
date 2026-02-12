// Provider Interface for Starbot_API
// Common interface that all providers must implement

export interface ProviderMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

export interface ProviderOptions {
  model: string;
  maxTokens?: number;
  temperature?: number;
  signal?: AbortSignal;
}

export interface ProviderUsage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}

export interface ProviderResponse {
  content: string;
  usage: ProviderUsage;
}

export interface StreamChunk {
  text: string;
  usage?: ProviderUsage;
}

export interface Provider {
  name: string;
  sendChat(messages: ProviderMessage[], options: ProviderOptions): Promise<ProviderResponse>;
  sendChatStream(messages: ProviderMessage[], options: ProviderOptions): AsyncIterable<StreamChunk>;
}
