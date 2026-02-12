// Google Vertex AI Provider
// Uses @google-cloud/vertexai SDK for Gemini models

import { VertexAI } from '@google-cloud/vertexai';
import type { Provider, ProviderMessage, ProviderOptions, ProviderResponse, StreamChunk } from './types.js';
import { env } from '../env.js';

export class VertexProvider implements Provider {
  name = 'vertex';
  private vertexAI: VertexAI;

  constructor() {
    if (!env.VERTEX_PROJECT_ID || !env.VERTEX_LOCATION) {
      throw new Error('VERTEX_PROJECT_ID and VERTEX_LOCATION are required');
    }

    // SDK will automatically use gcloud default credentials
    this.vertexAI = new VertexAI({
      project: env.VERTEX_PROJECT_ID,
      location: env.VERTEX_LOCATION,
    });
  }

  private formatMessages(messages: ProviderMessage[]): Array<{ role: string; parts: Array<{ text: string }> }> {
    // Vertex AI uses 'user' and 'model' roles (not 'assistant')
    return messages
      .filter(m => m.role !== 'system') // System messages handled separately
      .map(m => ({
        role: m.role === 'assistant' ? 'model' : m.role,
        parts: [{ text: m.content }],
      }));
  }

  private getSystemInstruction(messages: ProviderMessage[]): string | undefined {
    const systemMsg = messages.find(m => m.role === 'system');
    return systemMsg?.content;
  }

  async sendChat(messages: ProviderMessage[], options: ProviderOptions): Promise<ProviderResponse> {
    const model = this.vertexAI.getGenerativeModel({
      model: options.model,
      systemInstruction: this.getSystemInstruction(messages),
      generationConfig: {
        maxOutputTokens: options.maxTokens ?? 8192,
        temperature: options.temperature ?? 0.7,
      },
    });

    const result = await model.generateContent({
      contents: this.formatMessages(messages),
    });

    const response = result.response;
    const content = response.candidates?.[0]?.content?.parts?.[0]?.text || '';

    // Extract token usage if available
    const usageMetadata = response.usageMetadata;
    const promptTokens = usageMetadata?.promptTokenCount || 0;
    const completionTokens = usageMetadata?.candidatesTokenCount || 0;

    return {
      content,
      usage: {
        promptTokens,
        completionTokens,
        totalTokens: promptTokens + completionTokens,
      },
    };
  }

  async *sendChatStream(messages: ProviderMessage[], options: ProviderOptions): AsyncIterable<StreamChunk> {
    const model = this.vertexAI.getGenerativeModel({
      model: options.model,
      systemInstruction: this.getSystemInstruction(messages),
      generationConfig: {
        maxOutputTokens: options.maxTokens ?? 8192,
        temperature: options.temperature ?? 0.7,
      },
    });

    const result = await model.generateContentStream({
      contents: this.formatMessages(messages),
    });

    let promptTokens = 0;
    let completionTokens = 0;

    // Stream text chunks
    for await (const chunk of result.stream) {
      const text = chunk.candidates?.[0]?.content?.parts?.[0]?.text || '';

      if (text) {
        yield { text };
      }

      // Update token counts if available
      if (chunk.usageMetadata) {
        promptTokens = chunk.usageMetadata.promptTokenCount || 0;
        completionTokens = chunk.usageMetadata.candidatesTokenCount || 0;
      }
    }

    // Final usage report
    if (promptTokens > 0 || completionTokens > 0) {
      yield {
        text: '',
        usage: {
          promptTokens,
          completionTokens,
          totalTokens: promptTokens + completionTokens,
        },
      };
    }
  }
}
