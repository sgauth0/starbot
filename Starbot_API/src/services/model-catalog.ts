// Model Catalog for Starbot_API
// Defines all available models across providers with capabilities, tiers, and costs

import { env, isProviderConfigured } from '../env.js';

export interface ModelDefinition {
  id: string;                    // Unique identifier
  provider: string;              // kimi, vertex, azure, bedrock, cloudflare
  deploymentName: string;        // Actual model name used by provider API
  displayName: string;           // Human-readable name
  tier: number;                  // 1=cheap/fast, 2=standard, 3=premium
  capabilities: string[];        // ['text', 'vision', 'tools', 'streaming']
  contextWindow: number;         // Max input tokens
  maxOutputTokens: number;       // Max output tokens
  costPer1kInput?: number;       // USD per 1k input tokens
  costPer1kOutput?: number;      // USD per 1k output tokens
  latencyMs?: number;            // Typical latency
  status: 'enabled' | 'disabled' | 'beta';
  notes?: string;
}

const MODELS: ModelDefinition[] = [
  // ===== TIER 1: Cheap & Fast =====

  // Kimi K1S (cheapest option)
  {
    id: 'kimi-k1s',
    provider: 'kimi',
    deploymentName: 'moonshot-v1-8k',
    displayName: 'Kimi K1S (8k)',
    tier: 1,
    capabilities: ['text', 'streaming'],
    contextWindow: 8192,
    maxOutputTokens: 4096,
    costPer1kInput: 0.0002,
    costPer1kOutput: 0.0004,
    latencyMs: 500,
    status: 'enabled',
  },

  // Kimi K2 (standard fast option)
  {
    id: 'kimi-k2',
    provider: 'kimi',
    deploymentName: 'kimi-k2-0711-preview',
    displayName: 'Kimi K2',
    tier: 1,
    capabilities: ['text', 'streaming', 'tools'],
    contextWindow: 128000,
    maxOutputTokens: 4096,
    costPer1kInput: 0.002,
    costPer1kOutput: 0.006,
    latencyMs: 600,
    status: 'enabled',
  },

  // Gemini 2.5 Flash Lite
  {
    id: 'gemini-2.5-flash-lite',
    provider: 'vertex',
    deploymentName: 'gemini-2.5-flash-lite',
    displayName: 'Gemini 2.5 Flash Lite',
    tier: 1,
    capabilities: ['text', 'vision', 'streaming', 'tools'],
    contextWindow: 1048576,
    maxOutputTokens: 8192,
    costPer1kInput: 0.0001,
    costPer1kOutput: 0.0003,
    latencyMs: 400,
    status: 'enabled',
    notes: '1M context window',
  },

  // Cloudflare Mistral Small
  {
    id: 'cf-mistral-small',
    provider: 'cloudflare',
    deploymentName: '@cf/mistralai/mistral-small-3.1-24b-instruct',
    displayName: 'Mistral Small 3.1 (24B)',
    tier: 1,
    capabilities: ['text', 'streaming'],
    contextWindow: 32768,
    maxOutputTokens: 4096,
    costPer1kInput: 0.0001,
    costPer1kOutput: 0.0003,
    latencyMs: 700,
    status: 'enabled',
  },

  // ===== TIER 2: Standard =====

  // Gemini 2.5 Flash (BEST VALUE for tier 2)
  {
    id: 'gemini-2.5-flash',
    provider: 'vertex',
    deploymentName: 'gemini-2.5-flash',
    displayName: 'Gemini 2.5 Flash',
    tier: 2,
    capabilities: ['text', 'vision', 'streaming', 'tools'],
    contextWindow: 1048576,
    maxOutputTokens: 8192,
    costPer1kInput: 0.0004,
    costPer1kOutput: 0.0012,
    latencyMs: 600,
    status: 'enabled',
    notes: '1M context window, excellent balance',
  },

  // Azure GPT-4.1
  {
    id: 'gpt-4.1',
    provider: 'azure',
    deploymentName: 'gpt-4.1',
    displayName: 'GPT-4.1',
    tier: 2,
    capabilities: ['text', 'streaming', 'tools'],
    contextWindow: 128000,
    maxOutputTokens: 16384,
    costPer1kInput: 0.003,
    costPer1kOutput: 0.01,
    latencyMs: 800,
    status: 'enabled',
    notes: 'Solid reasoning, supports custom temperature',
  },

  // Azure Mistral Large 3
  {
    id: 'mistral-large-3',
    provider: 'azure',
    deploymentName: 'Mistral-Large-3',
    displayName: 'Mistral Large 3',
    tier: 2,
    capabilities: ['text', 'streaming', 'tools'],
    contextWindow: 131072,
    maxOutputTokens: 4096,
    costPer1kInput: 0.003,
    costPer1kOutput: 0.01,
    latencyMs: 700,
    status: 'enabled',
    notes: 'Strong multilingual, function calling',
  },

  // Azure DeepSeek-R1 (reasoning model)
  {
    id: 'deepseek-r1',
    provider: 'azure',
    deploymentName: 'DeepSeek-R1',
    displayName: 'DeepSeek-R1',
    tier: 2,
    capabilities: ['text', 'streaming'],
    contextWindow: 64000,
    maxOutputTokens: 8192,
    costPer1kInput: 0.002,
    costPer1kOutput: 0.008,
    latencyMs: 1200,
    status: 'enabled',
    notes: 'Deep reasoning, shows thinking process',
  },

  // Azure DeepSeek-V3.1
  {
    id: 'deepseek-v3.1',
    provider: 'azure',
    deploymentName: 'DeepSeek-V3.1',
    displayName: 'DeepSeek V3.1',
    tier: 2,
    capabilities: ['text', 'streaming', 'tools'],
    contextWindow: 64000,
    maxOutputTokens: 8192,
    costPer1kInput: 0.002,
    costPer1kOutput: 0.008,
    latencyMs: 1000,
    status: 'enabled',
    notes: 'Latest DeepSeek model, excellent for chat',
  },

  // Azure Kimi-K2.5
  {
    id: 'kimi-k2.5',
    provider: 'azure',
    deploymentName: 'Kimi-K2.5',
    displayName: 'Kimi K2.5',
    tier: 2,
    capabilities: ['text', 'streaming', 'tools'],
    contextWindow: 128000,
    maxOutputTokens: 4096,
    costPer1kInput: 0.003,
    costPer1kOutput: 0.009,
    latencyMs: 750,
    status: 'enabled',
    notes: 'Latest Moonshot model, good balance',
  },

  // Azure GPT-5.2
  {
    id: 'gpt-5.2',
    provider: 'azure',
    deploymentName: 'gpt-5.2-chat',
    displayName: 'GPT-5.2',
    tier: 2,
    capabilities: ['text', 'streaming', 'tools'],
    contextWindow: 128000,
    maxOutputTokens: 16384,
    costPer1kInput: 0.005,
    costPer1kOutput: 0.015,
    latencyMs: 900,
    status: 'enabled',
    notes: 'Best for reasoning and agentic tasks (no custom temp)',
  },

  // Azure Claude Haiku 4.5
  {
    id: 'claude-haiku-4.5',
    provider: 'azure',
    deploymentName: 'claude-haiku-4-5',
    displayName: 'Claude Haiku 4.5',
    tier: 1,
    capabilities: ['text', 'streaming', 'tools'],
    contextWindow: 200000,
    maxOutputTokens: 8192,
    costPer1kInput: 0.001,
    costPer1kOutput: 0.005,
    latencyMs: 650,
    status: 'enabled',
    notes: 'Fast Claude model via Azure Anthropic',
  },

  // Azure Claude Sonnet 4.5
  {
    id: 'claude-sonnet-4.5',
    provider: 'azure',
    deploymentName: 'claude-sonnet-4-5',
    displayName: 'Claude Sonnet 4.5',
    tier: 3,
    capabilities: ['text', 'streaming', 'tools'],
    contextWindow: 200000,
    maxOutputTokens: 8192,
    costPer1kInput: 0.003,
    costPer1kOutput: 0.015,
    latencyMs: 900,
    status: 'enabled',
    notes: 'Strong general reasoning via Azure Anthropic',
  },

  // Azure Claude Sonnet 4.5 (second deployment)
  {
    id: 'claude-sonnet-4.5-2',
    provider: 'azure',
    deploymentName: 'claude-sonnet-4-5-2',
    displayName: 'Claude Sonnet 4.5 (x2)',
    tier: 3,
    capabilities: ['text', 'streaming', 'tools'],
    contextWindow: 200000,
    maxOutputTokens: 8192,
    costPer1kInput: 0.003,
    costPer1kOutput: 0.015,
    latencyMs: 900,
    status: 'enabled',
    notes: 'Second Sonnet 4.5 deployment for additional capacity',
  },

  // ===== TIER 3: Premium (for most complex tasks) =====

  // Gemini 3.0 Flash Preview (FIRST CHOICE for tier 3)
  {
    id: 'gemini-3.0-flash',
    provider: 'vertex',
    deploymentName: 'gemini-3-flash-preview',
    displayName: 'Gemini 3.0 Flash (Preview)',
    tier: 3,
    capabilities: ['text', 'vision', 'streaming', 'tools'],
    contextWindow: 1048576,
    maxOutputTokens: 8192,
    costPer1kInput: 0.001,
    costPer1kOutput: 0.003,
    latencyMs: 700,
    status: 'enabled',
    notes: 'Complex agentic workflows, thinking_level control',
  },

  // Gemini 2.5 Pro
  {
    id: 'gemini-2.5-pro',
    provider: 'vertex',
    deploymentName: 'gemini-2.5-pro',
    displayName: 'Gemini 2.5 Pro',
    tier: 3,
    capabilities: ['text', 'vision', 'streaming', 'tools'],
    contextWindow: 1048576,
    maxOutputTokens: 8192,
    costPer1kInput: 0.002,
    costPer1kOutput: 0.006,
    latencyMs: 1200,
    status: 'enabled',
    notes: 'Most advanced reasoning, huge context',
  },

  // Azure Kimi-K2-Thinking (reasoning specialist)
  {
    id: 'kimi-k2-thinking',
    provider: 'azure',
    deploymentName: 'Kimi-K2-Thinking',
    displayName: 'Kimi K2 Thinking',
    tier: 3,
    capabilities: ['text', 'streaming'],
    contextWindow: 128000,
    maxOutputTokens: 4096,
    costPer1kInput: 0.004,
    costPer1kOutput: 0.012,
    latencyMs: 1400,
    status: 'enabled',
    notes: 'Deep reasoning, shows thinking process',
  },

  // Azure GPT-5.1-Codex-Mini
  {
    id: 'gpt-5.1-codex-mini',
    provider: 'azure',
    deploymentName: 'gpt-5.1-codex-mini',
    displayName: 'GPT-5.1 Codex Mini',
    tier: 3,
    capabilities: ['text', 'streaming', 'tools'],
    contextWindow: 128000,
    maxOutputTokens: 16384,
    costPer1kInput: 0.004,
    costPer1kOutput: 0.016,
    latencyMs: 800,
    status: 'enabled',
    notes: 'Excellent coding, faster than full Codex',
  },

  // Claude Opus 4.6 (LAST RESORT - most expensive)
  {
    id: 'claude-opus-4.6',
    provider: 'bedrock',
    deploymentName: 'anthropic.claude-opus-4-6-v1',
    displayName: 'Claude Opus 4.6 ⚠️',
    tier: 3,
    capabilities: ['text', 'vision', 'streaming', 'tools'],
    contextWindow: 200000,
    maxOutputTokens: 4096,
    costPer1kInput: 0.015,
    costPer1kOutput: 0.075,
    latencyMs: 1500,
    status: 'enabled',
    notes: 'USE SPARINGLY - Most expensive, strongest reasoning',
  },

  // Cloudflare Qwen 2.5 Coder
  {
    id: 'cf-qwen-coder',
    provider: 'cloudflare',
    deploymentName: '@cf/qwen/qwen2.5-coder-32b-instruct',
    displayName: 'Qwen 2.5 Coder (32B)',
    tier: 2,
    capabilities: ['text', 'streaming'],
    contextWindow: 32768,
    maxOutputTokens: 8192,
    costPer1kInput: 0.0002,
    costPer1kOutput: 0.0006,
    latencyMs: 800,
    status: 'enabled',
    notes: 'Open-weight coding specialist',
  },
];

export interface ListModelsOptions {
  status?: 'enabled' | 'disabled' | 'beta';
  provider?: string;
  tier?: number;
  capability?: string;
  configuredOnly?: boolean; // Only return models from configured providers
}

export async function listModels(options: ListModelsOptions = {}): Promise<ModelDefinition[]> {
  let filtered = [...MODELS];

  if (options.status) {
    filtered = filtered.filter(m => m.status === options.status);
  }

  if (options.provider) {
    filtered = filtered.filter(m => m.provider === options.provider);
  }

  if (options.tier !== undefined) {
    filtered = filtered.filter(m => m.tier === options.tier);
  }

  if (options.capability) {
    filtered = filtered.filter(m => m.capabilities.includes(options.capability!));
  }

  if (options.configuredOnly) {
    filtered = filtered.filter(m => isProviderConfigured(m.provider));
  }

  // Optional provider-specific deployment allow-lists.
  if (env.VERTEX_ALLOWED_MODELS.length > 0) {
    const allowed = new Set(env.VERTEX_ALLOWED_MODELS.map(s => s.toLowerCase()));
    filtered = filtered.filter(m => m.provider !== 'vertex' || allowed.has(m.deploymentName.toLowerCase()));
  }
  if (env.AZURE_ALLOWED_DEPLOYMENTS.length > 0) {
    const allowed = new Set(env.AZURE_ALLOWED_DEPLOYMENTS.map(s => s.toLowerCase()));
    filtered = filtered.filter(m => m.provider !== 'azure' || allowed.has(m.deploymentName.toLowerCase()));
  }

  return filtered;
}

export async function getModelById(id: string): Promise<ModelDefinition | null> {
  return MODELS.find(m => m.id === id) || null;
}

export async function getModelByProviderAndName(
  provider: string,
  deploymentName: string
): Promise<ModelDefinition | null> {
  return MODELS.find(m => m.provider === provider && m.deploymentName === deploymentName) || null;
}

export async function getCheapestModel(capability: string = 'text'): Promise<ModelDefinition | null> {
  const models = await listModels({ status: 'enabled', capability });

  if (models.length === 0) return null;

  return models.sort((a, b) => {
    const aCost = a.costPer1kInput || Number.POSITIVE_INFINITY;
    const bCost = b.costPer1kInput || Number.POSITIVE_INFINITY;
    return aCost - bCost;
  })[0];
}

export async function getBestModelForTier(
  tier: number,
  capability: string = 'text',
  configuredOnly: boolean = true
): Promise<ModelDefinition | null> {
  const models = await listModels({
    status: 'enabled',
    tier,
    capability,
    configuredOnly,
  });

  if (models.length === 0) {
    // Fallback: try adjacent tiers
    if (tier > 1) {
      return getBestModelForTier(tier - 1, capability, configuredOnly);
    }
    return null;
  }

  // Prefer lower cost within same tier
  return models.sort((a, b) => {
    const aCost = a.costPer1kInput || Number.POSITIVE_INFINITY;
    const bCost = b.costPer1kInput || Number.POSITIVE_INFINITY;
    return aCost - bCost;
  })[0];
}

// Get all models, grouped by tier
export async function getModelsByTier(): Promise<Record<number, ModelDefinition[]>> {
  const models = await listModels({ status: 'enabled', configuredOnly: true });
  const byTier: Record<number, ModelDefinition[]> = { 1: [], 2: [], 3: [] };

  for (const model of models) {
    if (!byTier[model.tier]) byTier[model.tier] = [];
    byTier[model.tier].push(model);
  }

  return byTier;
}
