// Environment bridge: maps CLI flags and detections to process.env
// Must run BEFORE any dynamic imports from Starbot_API

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { detectMultiple } from './detector.js';
import type { CLIOptions, DetectionResult } from './types.js';

// Parse dotenv file manually (avoid importing dotenv before we set env)
function parseDotenv(filePath: string): Record<string, string> {
  const content = readFileSync(resolve(filePath), 'utf-8');
  const result: Record<string, string> = {};

  for (const line of content.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;

    const eqIndex = trimmed.indexOf('=');
    if (eqIndex === -1) continue;

    const key = trimmed.slice(0, eqIndex).trim();
    let value = trimmed.slice(eqIndex + 1).trim();

    // Remove surrounding quotes
    if ((value.startsWith('"') && value.endsWith('"')) ||
        (value.startsWith("'") && value.endsWith("'"))) {
      value = value.slice(1, -1);
    }

    result[key] = value;
  }

  return result;
}

export interface BridgeResult {
  detections: DetectionResult[];
  envVarsSet: string[];
}

export function bridgeEnvironment(opts: CLIOptions): BridgeResult {
  const detections: DetectionResult[] = [];
  const envVarsSet: string[] = [];

  function setEnv(key: string, value: string) {
    if (value) {
      process.env[key] = value;
      envVarsSet.push(key);
    }
  }

  // 1. Load --env-file first (lowest priority)
  if (opts.envFile) {
    const parsed = parseDotenv(opts.envFile);
    for (const [key, value] of Object.entries(parsed)) {
      setEnv(key, value);
    }
  }

  // 2. --from-env: env vars already in process.env, nothing to do
  //    (they're already there from the parent shell)

  // 3. Explicit provider flags (higher priority, overwrite env-file)
  if (opts.azureEndpoint) setEnv('AZURE_OPENAI_ENDPOINT', opts.azureEndpoint);
  if (opts.azureKey) setEnv('AZURE_OPENAI_API_KEY', opts.azureKey);
  if (opts.azureDeployments) setEnv('AZURE_ALLOWED_DEPLOYMENTS', opts.azureDeployments);
  if (opts.awsKey) setEnv('AWS_ACCESS_KEY_ID', opts.awsKey);
  if (opts.awsSecret) setEnv('AWS_SECRET_ACCESS_KEY', opts.awsSecret);
  if (opts.awsRegion) {
    setEnv('AWS_REGION', opts.awsRegion);
    setEnv('BEDROCK_REGION', opts.awsRegion);
  }
  if (opts.vertexProject) setEnv('VERTEX_PROJECT_ID', opts.vertexProject);
  if (opts.vertexLocation) setEnv('VERTEX_LOCATION', opts.vertexLocation);
  if (opts.vertexCredentials) setEnv('GOOGLE_APPLICATION_CREDENTIALS', opts.vertexCredentials);
  if (opts.cfAccount) setEnv('CLOUDFLARE_ACCOUNT_ID', opts.cfAccount);
  if (opts.cfToken) setEnv('CLOUDFLARE_API_TOKEN', opts.cfToken);
  if (opts.moonshotKey) setEnv('MOONSHOT_API_KEY', opts.moonshotKey);

  // 4. Auto-detect --cred values (highest priority)
  if (opts.cred && opts.cred.length > 0) {
    const detected = detectMultiple(opts.cred);
    for (const det of detected) {
      setEnv(det.envVar, det.value);
      detections.push(det);
    }
  }

  return { detections, envVarsSet };
}
