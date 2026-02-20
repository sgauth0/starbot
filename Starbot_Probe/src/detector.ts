// Auto-detect credential type from bare values
// Runs each value against known patterns and returns the best match

import { readFileSync } from 'node:fs';
import { CREDENTIAL_PATTERNS } from './credential-patterns.js';
import type { DetectionResult } from './types.js';

export function detectCredential(value: string): DetectionResult | null {
  const trimmed = value.trim();

  // Special case: check if it's a JSON file containing service_account
  if (trimmed.endsWith('.json')) {
    try {
      const content = readFileSync(trimmed, 'utf-8');
      const parsed = JSON.parse(content);
      if (parsed.type === 'service_account') {
        return {
          provider: 'vertex',
          envVar: 'GOOGLE_APPLICATION_CREDENTIALS',
          value: trimmed,
          confidence: 'high',
          reason: 'JSON file contains service_account type — GCP credentials',
        };
      }
    } catch {
      // Not a valid JSON file or not readable, continue with pattern matching
    }
  }

  // Try each pattern in order (most specific first)
  for (const pat of CREDENTIAL_PATTERNS) {
    if (pat.pattern.test(trimmed)) {
      return {
        provider: pat.provider,
        envVar: pat.envVar,
        value: trimmed,
        confidence: pat.confidence,
        reason: pat.reason,
      };
    }
  }

  return null;
}

export function detectMultiple(values: string[]): DetectionResult[] {
  const results: DetectionResult[] = [];

  for (const value of values) {
    const result = detectCredential(value);
    if (result) {
      results.push(result);
    }
  }

  return results;
}
