// Azure AI Services Provider
// Uses direct REST API calls (Azure OpenAI uses standard OpenAI format)

import type { Provider, ProviderMessage, ProviderOptions, ProviderResponse, StreamChunk } from './types.js';
import { env } from '../env.js';

export class AzureProvider implements Provider {
  name = 'azure';

  private formatMessages(messages: ProviderMessage[]): Array<{ role: string; content: string }> {
    return messages.map(m => ({
      role: m.role,
      content: m.content,
    }));
  }

  private getModelConfig(model: string, options: ProviderOptions) {
    // Handle model-specific quirks
    const isGPT52 = model.includes('gpt-5.2');
    const isGPT41 = model.includes('gpt-4.1');

    // GPT-5.2 and GPT-4.1 require max_completion_tokens instead of max_tokens
    const useCompletionTokens = isGPT52 || isGPT41;

    // GPT-5.2 does NOT accept custom temperature (only default 1.0)
    const supportsTemperature = !isGPT52;

    const config: any = {};

    if (useCompletionTokens) {
      config.max_completion_tokens = options.maxTokens ?? 4096;
    } else {
      config.max_tokens = options.maxTokens ?? 4096;
    }

    if (supportsTemperature) {
      config.temperature = options.temperature ?? 0.7;
    }

    return config;
  }

  async sendChat(messages: ProviderMessage[], options: ProviderOptions): Promise<ProviderResponse> {
    if (!env.AZURE_OPENAI_ENDPOINT || !env.AZURE_OPENAI_API_KEY) {
      throw new Error('AZURE_OPENAI_ENDPOINT and AZURE_OPENAI_API_KEY are required');
    }

    const config = this.getModelConfig(options.model, options);

    // Azure OpenAI URL format: {endpoint}/openai/deployments/{deployment-name}/chat/completions?api-version=...
    const url = `${env.AZURE_OPENAI_ENDPOINT}/openai/deployments/${options.model}/chat/completions?api-version=2024-12-01-preview`;

    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'api-key': env.AZURE_OPENAI_API_KEY,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        messages: this.formatMessages(messages),
        ...config,
      }),
      signal: options.signal,
    });

    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Azure OpenAI API error (${response.status}): ${error}`);
    }

    const data = await response.json() as any;
    const content = data.choices[0]?.message?.content || '';
    const usage = data.usage;

    return {
      content,
      usage: {
        promptTokens: usage?.prompt_tokens || 0,
        completionTokens: usage?.completion_tokens || 0,
        totalTokens: usage?.total_tokens || 0,
      },
    };
  }

  async *sendChatStream(messages: ProviderMessage[], options: ProviderOptions): AsyncIterable<StreamChunk> {
    if (!env.AZURE_OPENAI_ENDPOINT || !env.AZURE_OPENAI_API_KEY) {
      throw new Error('AZURE_OPENAI_ENDPOINT and AZURE_OPENAI_API_KEY are required');
    }

    const config = this.getModelConfig(options.model, options);

    const url = `${env.AZURE_OPENAI_ENDPOINT}/openai/deployments/${options.model}/chat/completions?api-version=2024-12-01-preview`;

    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'api-key': env.AZURE_OPENAI_API_KEY,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        messages: this.formatMessages(messages),
        ...config,
        stream: true,
      }),
      signal: options.signal,
    });

    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Azure OpenAI API error (${response.status}): ${error}`);
    }

    if (!response.body) {
      throw new Error('Response body is null');
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';
    let promptTokens = 0;
    let completionTokens = 0;

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split('\n');
      buffer = lines.pop() || '';

      for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed || !trimmed.startsWith('data: ')) continue;

        const data = trimmed.slice(6);
        if (data === '[DONE]') {
          // Send final usage if available
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
          return;
        }

        try {
          const parsed = JSON.parse(data);
          const delta = parsed.choices?.[0]?.delta?.content;

          if (delta) {
            yield { text: delta };
          }

          // Azure sometimes includes usage in stream
          if (parsed.usage) {
            promptTokens = parsed.usage.prompt_tokens || 0;
            completionTokens = parsed.usage.completion_tokens || 0;
          }
        } catch (e) {
          // Skip malformed JSON
          continue;
        }
      }
    }
  }
}
