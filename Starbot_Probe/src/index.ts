#!/usr/bin/env node
// Starbot Probe — Provider credential probe CLI
// Detects credentials, validates completeness, discovers models, and tests them

import { Command } from 'commander';
import { bridgeEnvironment } from './env-bridge.js';
import { validateCredentials, getConfiguredProviders } from './validator.js';
import { discoverModels } from './discoverer.js';
import { testModels } from './tester.js';
import { formatFullReport, formatJSON, formatTestProgress } from './formatter.js';
import type { CLIOptions, ProbeResult } from './types.js';

const program = new Command();

program
  .name('starbot-probe')
  .description('Probe provider credentials: detect, validate, discover models, and test')
  .version('1.0.0')

  // Provider flags
  .option('--azure-endpoint <url>', 'Azure OpenAI endpoint')
  .option('--azure-key <key>', 'Azure OpenAI API key')
  .option('--azure-deployments <csv>', 'Allowed Azure deployment names (comma-separated)')
  .option('--aws-key <key>', 'AWS access key ID')
  .option('--aws-secret <secret>', 'AWS secret access key')
  .option('--aws-region <region>', 'AWS region (default: us-east-1)')
  .option('--vertex-project <id>', 'GCP project ID')
  .option('--vertex-location <loc>', 'Vertex AI location (default: us-central1)')
  .option('--vertex-credentials <path>', 'GCP service account JSON path')
  .option('--cf-account <id>', 'Cloudflare account ID')
  .option('--cf-token <token>', 'Cloudflare API token')
  .option('--moonshot-key <key>', 'Moonshot/Kimi API key')

  // Auto-detect
  .option('--cred <value...>', 'Auto-detect credential type (repeatable)')
  .option('--from-env', 'Read from current environment variables')
  .option('--env-file <path>', 'Load credentials from a .env file')

  // Behavior
  .option('--discover-only', 'List models only, skip test prompts')
  .option('--test-prompt <text>', 'Custom test prompt', 'Say hello in one sentence.')
  .option('--timeout <ms>', 'Per-model timeout in milliseconds', '15000')
  .option('--json', 'Output as JSON instead of formatted tables')
  .option('--verbose', 'Show full error details');

program.parse();
const opts = program.opts<CLIOptions>();

async function run() {
  // Phase 1: Bridge environment
  const { detections } = bridgeEnvironment(opts);

  // Phase 2: Validate credentials
  const validations = validateCredentials();
  const configuredProviders = getConfiguredProviders(validations);

  // Phase 3: Discover models
  let models = configuredProviders.length > 0
    ? await discoverModels(configuredProviders)
    : [];

  // Phase 4: Test models (unless --discover-only)
  let tests: ProbeResult['tests'] = [];

  if (!opts.discoverOnly && models.length > 0) {
    const timeoutMs = parseInt(opts.timeout || '15000', 10);
    const verbose = opts.verbose || false;

    if (!opts.json) {
      process.stderr.write('\n  Testing models...\n');
    }

    tests = await testModels(
      models,
      opts.testPrompt || 'Say hello in one sentence.',
      timeoutMs,
      verbose,
      opts.json ? undefined : (result, index, total) => {
        process.stderr.write(formatTestProgress(result, index, total) + '\n');
      },
    );
  }

  // Phase 5: Build result
  const result: ProbeResult = {
    detections,
    validations,
    models,
    tests,
    summary: {
      providersDetected: validations.length,
      providersConfigured: configuredProviders.length,
      modelsDiscovered: models.length,
      modelsTested: tests.length,
      modelsSucceeded: tests.filter(t => t.success).length,
      modelsFailed: tests.filter(t => !t.success).length,
    },
  };

  // Phase 6: Output
  if (opts.json) {
    console.log(formatJSON(result));
  } else {
    console.log(formatFullReport(result));
  }

  // Exit with error code if any tests failed
  if (tests.some(t => !t.success)) {
    process.exit(1);
  }
}

run().catch((err) => {
  if (opts.verbose) {
    console.error(err);
  } else {
    console.error(`Error: ${err.message || err}`);
  }
  process.exit(2);
});
