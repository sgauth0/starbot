// Validate credential completeness for each provider
// Reports what's present, what's missing, and whether a provider can work

import { PROVIDER_CREDENTIALS } from './credential-patterns.js';
import type { ProviderName, ValidationResult } from './types.js';

const ALL_PROVIDERS: ProviderName[] = ['azure', 'bedrock', 'vertex', 'cloudflare', 'kimi'];

export function validateCredentials(): ValidationResult[] {
  const results: ValidationResult[] = [];

  for (const provider of ALL_PROVIDERS) {
    const creds = PROVIDER_CREDENTIALS[provider];
    const present: ValidationResult['present'] = [];
    const missing: ValidationResult['missing'] = [];

    for (const cred of creds) {
      const value = process.env[cred.envVar];
      if (value && value.trim()) {
        present.push({ envVar: cred.envVar, label: cred.label });
      } else {
        missing.push({
          envVar: cred.envVar,
          label: cred.label,
          required: cred.required,
          defaultValue: cred.defaultValue,
        });
      }
    }

    // Only include providers where at least one credential is present
    if (present.length > 0) {
      const requiredMissing = missing.filter(m => m.required);
      results.push({
        provider,
        configured: requiredMissing.length === 0,
        present,
        missing,
      });
    }
  }

  return results;
}

export function getConfiguredProviders(validations: ValidationResult[]): ProviderName[] {
  return validations
    .filter(v => v.configured)
    .map(v => v.provider);
}
