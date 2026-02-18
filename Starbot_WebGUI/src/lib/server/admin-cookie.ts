const ADMIN_COOKIE_NAME = 'starbot_admin';
const DEFAULT_TTL_SECONDS = 60 * 60 * 8;

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function toBase64Url(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }

  return btoa(binary)
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/g, '');
}

function fromBase64Url(input: string): Uint8Array | null {
  try {
    const normalized = input.replace(/-/g, '+').replace(/_/g, '/');
    const padded = normalized + '==='.slice((normalized.length + 3) % 4);
    const binary = atob(padded);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return bytes;
  } catch {
    return null;
  }
}

function getSecret(): string {
  const explicit = String(process.env.ADMIN_COOKIE_SECRET || '').trim();
  if (explicit) return explicit;
  return String(process.env.ADMIN_CONSOLE_CODE || '').trim();
}

function getAllowedAdminEmails(): Set<string> {
  const raw = String(process.env.ADMIN_EMAILS || '').trim();
  if (!raw) return new Set<string>();

  return new Set(
    raw
      .split(',')
      .map((entry) => entry.trim().toLowerCase())
      .filter(Boolean)
  );
}

function isTimingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) {
    diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return diff === 0;
}

async function sign(data: string, secret: string): Promise<string> {
  const key = await crypto.subtle.importKey(
    'raw',
    encoder.encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign']
  );

  const signature = await crypto.subtle.sign('HMAC', key, encoder.encode(data));
  return toBase64Url(new Uint8Array(signature));
}

export function getAdminCookieName(): string {
  return ADMIN_COOKIE_NAME;
}

export function evaluateAdmin(email: string, providedCode?: string): boolean {
  const normalizedEmail = email.trim().toLowerCase();
  const allowedEmails = getAllowedAdminEmails();
  const isEmailAllowed = allowedEmails.has(normalizedEmail);
  if (!isEmailAllowed) return false;

  const requiredCode = String(process.env.ADMIN_CONSOLE_CODE || '').trim();
  if (!requiredCode) return true;

  return String(providedCode || '').trim() === requiredCode;
}

export async function issueAdminCookieToken(email: string): Promise<string | null> {
  const secret = getSecret();
  if (!secret) return null;

  const payload = {
    email: email.trim().toLowerCase(),
    exp: Math.floor(Date.now() / 1000) + DEFAULT_TTL_SECONDS,
  };
  const encodedPayload = toBase64Url(encoder.encode(JSON.stringify(payload)));
  const signature = await sign(encodedPayload, secret);
  return `${encodedPayload}.${signature}`;
}

export async function verifyAdminCookieToken(token?: string): Promise<boolean> {
  if (!token) return false;

  const secret = getSecret();
  if (!secret) return false;

  const parts = token.split('.');
  if (parts.length !== 2) return false;
  const [encodedPayload, providedSignature] = parts;
  if (!encodedPayload || !providedSignature) return false;

  const expectedSignature = await sign(encodedPayload, secret);
  if (!isTimingSafeEqual(providedSignature, expectedSignature)) return false;

  const decodedBytes = fromBase64Url(encodedPayload);
  if (!decodedBytes) return false;

  try {
    const payload = JSON.parse(decoder.decode(decodedBytes)) as {
      email?: string;
      exp?: number;
    };

    if (!payload.email || !payload.exp) return false;
    if (payload.exp <= Math.floor(Date.now() / 1000)) return false;
    const allowedEmails = getAllowedAdminEmails();
    if (!allowedEmails.has(payload.email.trim().toLowerCase())) return false;
    return true;
  } catch {
    return false;
  }
}
