// Types for Starbot Probe

export type ProviderName = 'azure' | 'bedrock' | 'vertex' | 'cloudflare' | 'kimi';

export interface CredentialMapping {
  envVar: string;
  provider: ProviderName;
  label: string;
  required: boolean;
  defaultValue?: string;
}

export interface DetectionResult {
  provider: ProviderName;
  envVar: string;
  value: string;
  confidence: 'high' | 'medium' | 'low';
  reason: string;
}

export interface ValidationResult {
  provider: ProviderName;
  configured: boolean;
  present: { envVar: string; label: string }[];
  missing: { envVar: string; label: string; required: boolean; defaultValue?: string }[];
}

export interface DiscoveredModel {
  id: string;
  provider: string;
  deploymentName: string;
  displayName: string;
  tier: number;
  capabilities: string[];
  contextWindow: number;
  maxOutputTokens: number;
}

export interface TestResult {
  model: DiscoveredModel;
  success: boolean;
  latencyMs: number;
  responsePreview?: string;
  error?: string;
  usage?: {
    promptTokens: number;
    completionTokens: number;
    totalTokens: number;
  };
}

export interface ProbeResult {
  detections: DetectionResult[];
  validations: ValidationResult[];
  models: DiscoveredModel[];
  tests: TestResult[];
  summary: {
    providersDetected: number;
    providersConfigured: number;
    modelsDiscovered: number;
    modelsTested: number;
    modelsSucceeded: number;
    modelsFailed: number;
  };
}

export interface CLIOptions {
  // Provider-specific flags
  azureEndpoint?: string;
  azureKey?: string;
  azureDeployments?: string;
  awsKey?: string;
  awsSecret?: string;
  awsRegion?: string;
  vertexProject?: string;
  vertexLocation?: string;
  vertexCredentials?: string;
  cfAccount?: string;
  cfToken?: string;
  moonshotKey?: string;

  // Auto-detect
  cred?: string[];
  fromEnv?: boolean;
  envFile?: string;

  // Behavior
  discoverOnly?: boolean;
  testPrompt?: string;
  timeout?: string;
  json?: boolean;
  verbose?: boolean;
}
