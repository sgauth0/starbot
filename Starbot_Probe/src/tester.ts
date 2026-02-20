// Test each discovered model with a probe prompt
// Uses dynamic import of Starbot_API providers

import { resolve } from 'node:path';
import type { DiscoveredModel, TestResult } from './types.js';

const API_ROOT = resolve(import.meta.dirname, '../../Starbot_API/src');

export async function testModels(
  models: DiscoveredModel[],
  testPrompt: string,
  timeoutMs: number,
  verbose: boolean,
  onResult?: (result: TestResult, index: number, total: number) => void,
): Promise<TestResult[]> {
  // Dynamic import of provider registry after env is set
  const providerPath = resolve(API_ROOT, 'providers/index.ts');
  const { getProvider } = await import(providerPath);

  const results: TestResult[] = [];

  for (let i = 0; i < models.length; i++) {
    const model = models[i];
    const result = await testSingleModel(getProvider, model, testPrompt, timeoutMs, verbose);
    results.push(result);
    onResult?.(result, i, models.length);
  }

  return results;
}

async function testSingleModel(
  getProvider: (name: string) => any,
  model: DiscoveredModel,
  testPrompt: string,
  timeoutMs: number,
  verbose: boolean,
): Promise<TestResult> {
  const start = Date.now();

  try {
    const provider = getProvider(model.provider);

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);

    try {
      const response = await provider.sendChat(
        [{ role: 'user' as const, content: testPrompt }],
        {
          model: model.deploymentName,
          maxTokens: 50,
          temperature: 0.3,
          signal: controller.signal,
        },
      );

      clearTimeout(timer);
      const latencyMs = Date.now() - start;

      return {
        model,
        success: true,
        latencyMs,
        responsePreview: response.content.slice(0, 120),
        usage: response.usage,
      };
    } finally {
      clearTimeout(timer);
    }
  } catch (err: any) {
    const latencyMs = Date.now() - start;
    const errorMsg = verbose
      ? (err.stack || err.message || String(err))
      : (err.message || String(err));

    return {
      model,
      success: false,
      latencyMs,
      error: errorMsg,
    };
  }
}
