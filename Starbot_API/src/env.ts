// Environment configuration for Starbot_API
// Load all provider credentials and settings from environment variables

export const env = {
  // Server
  PORT: parseInt(process.env.PORT || '3737', 10),
  HOST: process.env.HOST || '127.0.0.1',
  NODE_ENV: process.env.NODE_ENV || 'development',

  // Kimi/Moonshot
  MOONSHOT_API_KEY: process.env.MOONSHOT_API_KEY || '',
  MOONSHOT_BASE_URL: process.env.MOONSHOT_BASE_URL || 'https://api.moonshot.cn',

  // Google Vertex AI
  VERTEX_PROJECT_ID: process.env.VERTEX_PROJECT_ID || '',
  VERTEX_LOCATION: process.env.VERTEX_LOCATION || 'us-central1',
  GOOGLE_APPLICATION_CREDENTIALS: process.env.GOOGLE_APPLICATION_CREDENTIALS || '',

  // Azure AI Services
  AZURE_OPENAI_ENDPOINT: process.env.AZURE_OPENAI_ENDPOINT || '',
  AZURE_OPENAI_API_KEY: process.env.AZURE_OPENAI_API_KEY || '',
  AZURE_OPENAI_MODELS: (process.env.AZURE_OPENAI_MODELS || '').split(',').filter(Boolean),

  // AWS Bedrock
  AWS_ACCESS_KEY_ID: process.env.AWS_ACCESS_KEY_ID || '',
  AWS_SECRET_ACCESS_KEY: process.env.AWS_SECRET_ACCESS_KEY || '',
  AWS_REGION: process.env.AWS_REGION || process.env.BEDROCK_REGION || 'us-east-1',
  BEDROCK_REGION: process.env.BEDROCK_REGION || 'us-east-1',

  // Cloudflare Workers AI
  CLOUDFLARE_ACCOUNT_ID: process.env.CLOUDFLARE_ACCOUNT_ID || '',
  CLOUDFLARE_API_TOKEN: process.env.CLOUDFLARE_API_TOKEN || '',

  // Triage
  TRIAGE_MODEL_ENABLED: process.env.TRIAGE_MODEL_ENABLED === 'true',

  // Features
  TOOLS_ENABLED: process.env.TOOLS_ENABLED !== 'false', // Default true
  WEB_SEARCH_ENABLED: process.env.WEB_SEARCH_ENABLED === 'true',
  WEB_SEARCH_API_KEY: process.env.WEB_SEARCH_API_KEY || '',

  // Logging
  LOG_LEVEL: process.env.LOG_LEVEL || 'info',
};

// Validation helpers
export function isProviderConfigured(provider: string): boolean {
  switch (provider) {
    case 'kimi':
      return !!env.MOONSHOT_API_KEY;
    case 'vertex':
      return !!env.VERTEX_PROJECT_ID && !!env.GOOGLE_APPLICATION_CREDENTIALS;
    case 'azure':
      return !!env.AZURE_OPENAI_ENDPOINT && !!env.AZURE_OPENAI_API_KEY;
    case 'bedrock':
      return !!env.AWS_ACCESS_KEY_ID && !!env.AWS_SECRET_ACCESS_KEY;
    case 'cloudflare':
      return !!env.CLOUDFLARE_ACCOUNT_ID && !!env.CLOUDFLARE_API_TOKEN;
    default:
      return false;
  }
}

export function listConfiguredProviders(): string[] {
  const providers = ['kimi', 'vertex', 'azure', 'bedrock', 'cloudflare'];
  return providers.filter(isProviderConfigured);
}

// Log configuration on startup (redact secrets)
export function logConfiguration() {
  const configured = listConfiguredProviders();
  console.log('Starbot_API Configuration:');
  console.log(`  Environment: ${env.NODE_ENV}`);
  console.log(`  Server: ${env.HOST}:${env.PORT}`);
  console.log(`  Configured providers: ${configured.join(', ') || 'none'}`);
  console.log(`  Tools enabled: ${env.TOOLS_ENABLED}`);
  console.log(`  Web search enabled: ${env.WEB_SEARCH_ENABLED}`);
  console.log(`  Triage model enabled: ${env.TRIAGE_MODEL_ENABLED}`);
}
