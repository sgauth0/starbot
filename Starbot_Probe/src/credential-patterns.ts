// Credential pattern matching for auto-detection
// Maps regex patterns to provider credentials

import type { ProviderName, CredentialMapping } from './types.js';

export interface CredentialPattern {
  pattern: RegExp;
  provider: ProviderName;
  envVar: string;
  confidence: 'high' | 'medium' | 'low';
  reason: string;
}

// Patterns ordered from most specific to least specific
export const CREDENTIAL_PATTERNS: CredentialPattern[] = [
  // AWS Access Key ID - starts with AKIA, 20 chars
  {
    pattern: /^AKIA[0-9A-Z]{16}$/,
    provider: 'bedrock',
    envVar: 'AWS_ACCESS_KEY_ID',
    confidence: 'high',
    reason: 'Matches AWS access key format (AKIA prefix, 20 chars)',
  },

  // AWS Secret Access Key - 40 char base64-ish string
  {
    pattern: /^[A-Za-z0-9/+=]{40}$/,
    provider: 'bedrock',
    envVar: 'AWS_SECRET_ACCESS_KEY',
    confidence: 'medium',
    reason: 'Matches AWS secret key format (40 chars, base64-like)',
  },

  // Azure OpenAI Endpoint - URL containing openai.azure.com
  {
    pattern: /^https?:\/\/[^/]*\.openai\.azure\.com/i,
    provider: 'azure',
    envVar: 'AZURE_OPENAI_ENDPOINT',
    confidence: 'high',
    reason: 'URL matches Azure OpenAI endpoint pattern (*.openai.azure.com)',
  },

  // Azure API Key - 32 char hex string
  {
    pattern: /^[a-f0-9]{32}$/i,
    provider: 'azure',
    envVar: 'AZURE_OPENAI_API_KEY',
    confidence: 'medium',
    reason: 'Matches Azure API key format (32-char hex)',
  },

  // GCP Service Account JSON file path
  {
    pattern: /\.json$/i,
    provider: 'vertex',
    envVar: 'GOOGLE_APPLICATION_CREDENTIALS',
    confidence: 'medium',
    reason: 'JSON file path — likely a GCP service account credentials file',
  },

  // AWS Region format
  {
    pattern: /^[a-z]{2}-[a-z]+-\d$/,
    provider: 'bedrock',
    envVar: 'AWS_REGION',
    confidence: 'high',
    reason: 'Matches AWS region format (e.g., us-east-1)',
  },

  // Moonshot/Kimi API Key - starts with sk-
  {
    pattern: /^sk-[A-Za-z0-9]{20,}$/,
    provider: 'kimi',
    envVar: 'MOONSHOT_API_KEY',
    confidence: 'high',
    reason: 'Matches Moonshot API key format (sk- prefix)',
  },

  // GCP Project ID - lowercase, hyphens, 6-30 chars
  {
    pattern: /^[a-z][a-z0-9-]{4,28}[a-z0-9]$/,
    provider: 'vertex',
    envVar: 'VERTEX_PROJECT_ID',
    confidence: 'low',
    reason: 'Matches GCP project ID format (lowercase alphanumeric with hyphens)',
  },
];

// All provider credential requirements for validation
export const PROVIDER_CREDENTIALS: Record<ProviderName, CredentialMapping[]> = {
  azure: [
    { envVar: 'AZURE_OPENAI_ENDPOINT', provider: 'azure', label: 'Azure OpenAI Endpoint', required: true },
    { envVar: 'AZURE_OPENAI_API_KEY', provider: 'azure', label: 'Azure OpenAI API Key', required: true },
    { envVar: 'AZURE_ALLOWED_DEPLOYMENTS', provider: 'azure', label: 'Allowed Deployments (CSV)', required: false },
  ],
  bedrock: [
    { envVar: 'AWS_ACCESS_KEY_ID', provider: 'bedrock', label: 'AWS Access Key ID', required: true },
    { envVar: 'AWS_SECRET_ACCESS_KEY', provider: 'bedrock', label: 'AWS Secret Access Key', required: true },
    { envVar: 'AWS_REGION', provider: 'bedrock', label: 'AWS Region', required: false, defaultValue: 'us-east-1' },
    { envVar: 'BEDROCK_REGION', provider: 'bedrock', label: 'Bedrock Region', required: false, defaultValue: 'us-east-1' },
  ],
  vertex: [
    { envVar: 'VERTEX_PROJECT_ID', provider: 'vertex', label: 'GCP Project ID', required: true },
    { envVar: 'VERTEX_LOCATION', provider: 'vertex', label: 'Vertex Location', required: false, defaultValue: 'us-central1' },
    { envVar: 'GOOGLE_APPLICATION_CREDENTIALS', provider: 'vertex', label: 'Service Account JSON Path', required: false },
  ],
  cloudflare: [
    { envVar: 'CLOUDFLARE_ACCOUNT_ID', provider: 'cloudflare', label: 'Cloudflare Account ID', required: true },
    { envVar: 'CLOUDFLARE_API_TOKEN', provider: 'cloudflare', label: 'Cloudflare API Token', required: true },
  ],
  kimi: [
    { envVar: 'MOONSHOT_API_KEY', provider: 'kimi', label: 'Moonshot API Key', required: true },
    { envVar: 'MOONSHOT_BASE_URL', provider: 'kimi', label: 'Moonshot Base URL', required: false, defaultValue: 'https://api.moonshot.cn' },
  ],
};
