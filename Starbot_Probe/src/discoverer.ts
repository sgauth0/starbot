// Model discovery via dynamic import of Starbot_API's model catalog
// Discovers all enabled models for configured providers

import { resolve } from 'node:path';
import type { DiscoveredModel, ProviderName } from './types.js';

const API_ROOT = resolve(import.meta.dirname, '../../Starbot_API/src');

export async function discoverModels(providers: ProviderName[]): Promise<DiscoveredModel[]> {
  // Dynamic import of model catalog after env is set
  const catalogPath = resolve(API_ROOT, 'services/model-catalog.ts');
  const { listModels } = await import(catalogPath);

  const allModels: DiscoveredModel[] = [];

  for (const provider of providers) {
    const models = await listModels({ status: 'enabled', provider, configuredOnly: true });

    for (const m of models) {
      allModels.push({
        id: m.id,
        provider: m.provider,
        deploymentName: m.deploymentName,
        displayName: m.displayName,
        tier: m.tier,
        capabilities: m.capabilities,
        contextWindow: m.contextWindow,
        maxOutputTokens: m.maxOutputTokens,
      });
    }
  }

  return allModels;
}
