// Terminal output formatting
// Supports both rich table output (chalk + cli-table3) and JSON mode

import chalk from 'chalk';
import Table from 'cli-table3';
import type { ProbeResult, DetectionResult, ValidationResult, DiscoveredModel, TestResult } from './types.js';

// --- Detection Phase ---

export function formatDetections(detections: DetectionResult[]): string {
  if (detections.length === 0) return '';

  const lines: string[] = [
    '',
    chalk.bold.cyan('  Credential Detection'),
    chalk.dim('  ─'.repeat(30)),
  ];

  for (const det of detections) {
    const conf = det.confidence === 'high'
      ? chalk.green('high')
      : det.confidence === 'medium'
        ? chalk.yellow('med')
        : chalk.red('low');

    const redacted = redactValue(det.value);
    lines.push(
      `  ${chalk.green('+')} ${chalk.bold(det.envVar)} ${chalk.dim('=')} ${redacted}` +
      `  ${chalk.dim('[')}${conf}${chalk.dim(']')} ${chalk.dim(det.reason)}`
    );
  }

  return lines.join('\n');
}

// --- Validation Phase ---

export function formatValidations(validations: ValidationResult[]): string {
  if (validations.length === 0) {
    return '\n' + chalk.yellow('  No provider credentials detected.') +
      '\n' + chalk.dim('  Use --cred, --env-file, or --from-env to provide credentials.');
  }

  const lines: string[] = [
    '',
    chalk.bold.cyan('  Provider Validation'),
    chalk.dim('  ─'.repeat(30)),
  ];

  for (const v of validations) {
    const status = v.configured
      ? chalk.green('READY')
      : chalk.red('INCOMPLETE');

    lines.push(`\n  ${chalk.bold(v.provider.toUpperCase())}  ${status}`);

    for (const p of v.present) {
      lines.push(`    ${chalk.green('\u2713')} ${p.label} ${chalk.dim('(' + p.envVar + ')')}`);
    }

    for (const m of v.missing) {
      if (m.required) {
        lines.push(`    ${chalk.red('\u2717')} ${m.label} ${chalk.dim('(' + m.envVar + ')')} ${chalk.red('required')}`);
      } else {
        const def = m.defaultValue ? chalk.dim(`: ${m.defaultValue} (default)`) : '';
        lines.push(`    ${chalk.dim('\u25CB')} ${m.label} ${chalk.dim('(' + m.envVar + ')')}${def}`);
      }
    }

    // Suggestion for incomplete providers
    if (!v.configured) {
      const needed = v.missing
        .filter(m => m.required)
        .map(m => `--${flagForEnvVar(m.envVar)} <value>`)
        .join(' ');
      if (needed) {
        lines.push(`    ${chalk.yellow('\u26A0\uFE0F')}  Also need: ${chalk.yellow(needed)}`);
      }
    }
  }

  return lines.join('\n');
}

// --- Discovery Phase ---

export function formatDiscovery(models: DiscoveredModel[]): string {
  if (models.length === 0) {
    return '\n' + chalk.yellow('  No models discovered.');
  }

  const lines: string[] = [
    '',
    chalk.bold.cyan(`  Model Discovery  ${chalk.dim(`(${models.length} models)`)}`),
    chalk.dim('  ─'.repeat(30)),
  ];

  const table = new Table({
    head: ['Model', 'Provider', 'Tier', 'Capabilities', 'Context'].map(h => chalk.bold(h)),
    style: { head: [], border: ['dim'] },
    chars: {
      top: '', 'top-mid': '', 'top-left': '', 'top-right': '',
      bottom: '', 'bottom-mid': '', 'bottom-left': '', 'bottom-right': '',
      left: '  ', 'left-mid': '', mid: '', 'mid-mid': '',
      right: '', 'right-mid': '', middle: ' ',
    },
  });

  for (const m of models) {
    const tierLabel = m.tier === 1 ? chalk.green('T1') : m.tier === 2 ? chalk.yellow('T2') : chalk.red('T3');
    table.push([
      m.displayName,
      chalk.dim(m.provider),
      tierLabel,
      chalk.dim(m.capabilities.join(', ')),
      chalk.dim(formatNumber(m.contextWindow)),
    ]);
  }

  lines.push(table.toString());
  return lines.join('\n');
}

// --- Test Phase ---

export function formatTests(tests: TestResult[]): string {
  if (tests.length === 0) return '';

  const lines: string[] = [
    '',
    chalk.bold.cyan('  Model Testing'),
    chalk.dim('  ─'.repeat(30)),
  ];

  for (const t of tests) {
    if (t.success) {
      const usage = t.usage ? chalk.dim(` [${t.usage.totalTokens} tok]`) : '';
      lines.push(
        `  ${chalk.green('\u2713')} ${chalk.bold(t.model.displayName)} ` +
        `${chalk.dim(t.model.provider)} ` +
        `${chalk.green(t.latencyMs + 'ms')}${usage}`
      );
      if (t.responsePreview) {
        lines.push(`    ${chalk.dim('\u2514')} ${chalk.dim(t.responsePreview)}`);
      }
    } else {
      lines.push(
        `  ${chalk.red('\u2717')} ${chalk.bold(t.model.displayName)} ` +
        `${chalk.dim(t.model.provider)} ` +
        `${chalk.red(t.latencyMs + 'ms')}`
      );
      if (t.error) {
        lines.push(`    ${chalk.dim('\u2514')} ${chalk.red(t.error.split('\n')[0])}`);
      }
    }
  }

  return lines.join('\n');
}

// --- Test progress callback ---

export function formatTestProgress(result: TestResult, index: number, total: number): string {
  const num = `[${index + 1}/${total}]`;
  const icon = result.success ? chalk.green('\u2713') : chalk.red('\u2717');
  return `  ${chalk.dim(num)} ${icon} ${result.model.displayName} ${chalk.dim(result.latencyMs + 'ms')}`;
}

// --- Summary ---

export function formatSummary(result: ProbeResult): string {
  const s = result.summary;
  const lines: string[] = [
    '',
    chalk.dim('  ─'.repeat(30)),
    chalk.bold.cyan('  Summary'),
    `  Providers: ${chalk.bold(String(s.providersConfigured))} configured` +
      (s.providersDetected > s.providersConfigured
        ? chalk.dim(` (${s.providersDetected - s.providersConfigured} incomplete)`)
        : ''),
    `  Models:    ${chalk.bold(String(s.modelsDiscovered))} discovered`,
  ];

  if (s.modelsTested > 0) {
    lines.push(
      `  Tests:     ${chalk.green(String(s.modelsSucceeded) + ' passed')}` +
      (s.modelsFailed > 0 ? `, ${chalk.red(String(s.modelsFailed) + ' failed')}` : '') +
      ` of ${s.modelsTested}`
    );
  }

  lines.push('');
  return lines.join('\n');
}

// --- JSON Output ---

export function formatJSON(result: ProbeResult): string {
  return JSON.stringify(result, null, 2);
}

// --- Full Report ---

export function formatFullReport(result: ProbeResult): string {
  const parts: string[] = [
    '',
    chalk.bold('  starbot-probe'),
  ];

  if (result.detections.length > 0) {
    parts.push(formatDetections(result.detections));
  }

  parts.push(formatValidations(result.validations));

  if (result.models.length > 0) {
    parts.push(formatDiscovery(result.models));
  }

  if (result.tests.length > 0) {
    parts.push(formatTests(result.tests));
  }

  parts.push(formatSummary(result));

  return parts.join('\n');
}

// --- Helpers ---

function redactValue(value: string): string {
  if (value.length <= 8) return chalk.dim('***');
  return value.slice(0, 4) + chalk.dim('*'.repeat(Math.min(value.length - 8, 20))) + value.slice(-4);
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
  if (n >= 1_000) return (n / 1_000).toFixed(0) + 'k';
  return String(n);
}

function flagForEnvVar(envVar: string): string {
  const map: Record<string, string> = {
    'AZURE_OPENAI_ENDPOINT': 'azure-endpoint',
    'AZURE_OPENAI_API_KEY': 'azure-key',
    'AZURE_ALLOWED_DEPLOYMENTS': 'azure-deployments',
    'AWS_ACCESS_KEY_ID': 'aws-key',
    'AWS_SECRET_ACCESS_KEY': 'aws-secret',
    'AWS_REGION': 'aws-region',
    'BEDROCK_REGION': 'aws-region',
    'VERTEX_PROJECT_ID': 'vertex-project',
    'VERTEX_LOCATION': 'vertex-location',
    'GOOGLE_APPLICATION_CREDENTIALS': 'vertex-credentials',
    'CLOUDFLARE_ACCOUNT_ID': 'cf-account',
    'CLOUDFLARE_API_TOKEN': 'cf-token',
    'MOONSHOT_API_KEY': 'moonshot-key',
    'MOONSHOT_BASE_URL': 'moonshot-key',
  };
  return map[envVar] || envVar.toLowerCase().replace(/_/g, '-');
}
