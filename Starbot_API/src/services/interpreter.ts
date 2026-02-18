import { env } from '../env.js';
import { getProvider } from '../providers/index.js';

export type InterpreterIntent = 'chat' | 'browse' | 'filesystem' | 'code' | 'tool' | 'clarify';

export interface InterpretationResult {
  shouldClarify: boolean;
  clarificationQuestion?: string;
  normalizedUserMessage: string;
  primaryIntent: InterpreterIntent;
  intents: InterpreterIntent[];
  confidence: number;
  reason?: string;
}

function clampConfidence(value: unknown, fallback = 0.5): number {
  const num = typeof value === 'number' ? value : Number(value);
  if (!Number.isFinite(num)) return fallback;
  return Math.max(0, Math.min(1, num));
}

function extractJsonObject(raw: string): string | null {
  const start = raw.indexOf('{');
  const end = raw.lastIndexOf('}');
  if (start === -1 || end === -1 || end <= start) return null;
  return raw.slice(start, end + 1);
}

function normalizeIntent(raw: unknown): InterpreterIntent | null {
  const value = String(raw || '').trim().toLowerCase();
  if (!value) return null;
  if (value === 'clarify') return 'clarify';
  if (['browse', 'web', 'web_search', 'research', 'search'].includes(value)) return 'browse';
  if (
    ['filesystem', 'files', 'file', 'directory', 'workspace', 'repo', 'folder', 'local'].includes(
      value,
    )
  ) {
    return 'filesystem';
  }
  if (['code', 'coding', 'programming', 'edit', 'refactor', 'debug'].includes(value)) {
    return 'code';
  }
  if (['tool', 'tools', 'function', 'function_call', 'action'].includes(value)) return 'tool';
  if (['chat', 'general', 'qa', 'question'].includes(value)) return 'chat';
  return null;
}

function dedupeIntents(intents: InterpreterIntent[]): InterpreterIntent[] {
  const out: InterpreterIntent[] = [];
  for (const intent of intents) {
    if (!out.includes(intent)) out.push(intent);
  }
  return out;
}

function heuristicIntents(message: string): InterpreterIntent[] {
  const text = message.toLowerCase();
  const intents: InterpreterIntent[] = [];

  if (
    /\b(ls|pwd)\b/.test(text) ||
    text.includes('directory') ||
    text.includes('folder') ||
    text.includes('files in') ||
    text.includes('file list') ||
    text.includes('workspace')
  ) {
    intents.push('filesystem');
  }

  if (
    text.includes('browse') ||
    text.includes('search the web') ||
    text.includes('look up') ||
    text.includes('latest') ||
    text.includes('news')
  ) {
    intents.push('browse');
  }

  if (
    text.includes('code') ||
    text.includes('debug') ||
    text.includes('refactor') ||
    text.includes('fix this')
  ) {
    intents.push('code');
  }

  if (intents.length === 0) intents.push('chat');
  return dedupeIntents(intents);
}

export async function interpretUserMessage(message: string): Promise<InterpretationResult> {
  const original = String(message || '').trim();
  if (!original) {
    return {
      shouldClarify: true,
      clarificationQuestion: 'What would you like me to do?',
      normalizedUserMessage: '',
      primaryIntent: 'clarify',
      intents: ['clarify'],
      confidence: 1,
      reason: 'empty_message',
    };
  }

  if (!env.INTERPRETER_ENABLED) {
    const intents = heuristicIntents(original);
    return {
      shouldClarify: false,
      normalizedUserMessage: original,
      primaryIntent: intents[0] || 'chat',
      intents,
      confidence: 1,
      reason: 'interpreter_disabled',
    };
  }

  try {
    const provider = getProvider('cloudflare');
    const response = await provider.sendChat(
      [
        {
          role: 'system',
          content:
            'You are a request interpreter and router. Return JSON only. ' +
            'Decide if the request is executable or needs clarification, and classify intent(s). ' +
            'Schema: {"action":"execute|clarify","primary_intent":"chat|browse|filesystem|code|tool|clarify","intents":["chat|browse|filesystem|code|tool|clarify"],"normalized_user_message":"string","clarification_question":"string","confidence":0..1,"reason":"string"}. ' +
            'Use primary_intent=browse for web lookup/research requests. ' +
            'Use primary_intent=filesystem for local files/directories/workspace requests. ' +
            'Use action=clarify only when critical details are missing to proceed safely.',
        },
        {
          role: 'user',
          content: `User message:\n${original}`,
        },
      ],
      {
        model: env.INTERPRETER_MODEL,
        maxTokens: env.INTERPRETER_MAX_TOKENS,
        temperature: 0.1,
      },
    );

    const jsonText = extractJsonObject(response.content);
    if (!jsonText) {
      const intents = heuristicIntents(original);
      return {
        shouldClarify: false,
        normalizedUserMessage: original,
        primaryIntent: intents[0] || 'chat',
        intents,
        confidence: 0.35,
        reason: 'interpreter_non_json',
      };
    }

    const parsed = JSON.parse(jsonText) as {
      action?: string;
      primary_intent?: string;
      intent?: string;
      intents?: string[];
      normalized_user_message?: string;
      clarification_question?: string;
      confidence?: number;
      reason?: string;
    };

    const action = String(parsed.action || 'execute').trim().toLowerCase();
    const normalized = String(parsed.normalized_user_message || '').trim() || original;
    const clarification = String(parsed.clarification_question || '').trim();
    const intents = dedupeIntents(
      [
        normalizeIntent(parsed.primary_intent),
        normalizeIntent(parsed.intent),
        ...((Array.isArray(parsed.intents) ? parsed.intents : []).map((v) => normalizeIntent(v))),
      ].filter((v): v is InterpreterIntent => !!v),
    );
    const fallbackIntents = intents.length > 0 ? intents : heuristicIntents(normalized);
    const primaryIntent =
      normalizeIntent(parsed.primary_intent) || fallbackIntents[0] || (action === 'clarify' ? 'clarify' : 'chat');

    const lowerOriginal = original.toLowerCase();
    const explicitFilesystemSignal =
      /\b(ls|pwd)\b/.test(lowerOriginal) ||
      lowerOriginal.includes('directory') ||
      lowerOriginal.includes('folder') ||
      lowerOriginal.includes('files') ||
      lowerOriginal.includes('contents') ||
      lowerOriginal.includes('working directory') ||
      lowerOriginal.includes('current directory');
    const heuristicSuggestsFilesystem = fallbackIntents.includes('filesystem');
    const weakClarifyForFilesystem =
      action === 'clarify' && explicitFilesystemSignal && heuristicSuggestsFilesystem;

    if (weakClarifyForFilesystem) {
      return {
        shouldClarify: false,
        normalizedUserMessage: normalized,
        primaryIntent: 'filesystem',
        intents: dedupeIntents(['filesystem', ...fallbackIntents.filter((v) => v !== 'clarify')]),
        confidence: clampConfidence(parsed.confidence, 0.65),
        reason: 'clarify_overridden_filesystem_heuristic',
      };
    }

    if (action === 'clarify' && clarification) {
      return {
        shouldClarify: true,
        clarificationQuestion: clarification,
        normalizedUserMessage: normalized,
        primaryIntent: 'clarify',
        intents: dedupeIntents(['clarify', ...fallbackIntents]),
        confidence: clampConfidence(parsed.confidence, 0.7),
        reason: parsed.reason,
      };
    }

    return {
      shouldClarify: false,
      normalizedUserMessage: normalized,
      primaryIntent,
      intents: fallbackIntents,
      confidence: clampConfidence(parsed.confidence, 0.8),
      reason: parsed.reason,
    };
  } catch (error) {
    const intents = heuristicIntents(original);
    return {
      shouldClarify: false,
      normalizedUserMessage: original,
      primaryIntent: intents[0] || 'chat',
      intents,
      confidence: 0.25,
      reason: `interpreter_error:${error instanceof Error ? error.message : String(error)}`,
    };
  }
}
