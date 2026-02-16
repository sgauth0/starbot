export interface AuthSession {
  email: string;
  name: string;
  loggedInAt: string;
}

export const AUTH_SESSION_KEY = 'starbot_session';
export const AUTH_CHANGED_EVENT = 'starbot-auth-changed';

function isBrowser() {
  return typeof window !== 'undefined';
}

export function readAuthSession(): AuthSession | null {
  if (!isBrowser()) return null;
  const raw = localStorage.getItem(AUTH_SESSION_KEY);
  if (!raw) return null;

  try {
    const parsed = JSON.parse(raw) as AuthSession;
    if (!parsed.email || !parsed.name) return null;
    return parsed;
  } catch {
    return null;
  }
}

export function writeAuthSession(session: AuthSession) {
  if (!isBrowser()) return;
  localStorage.setItem(AUTH_SESSION_KEY, JSON.stringify(session));
  window.dispatchEvent(new Event(AUTH_CHANGED_EVENT));
}

export function clearAuthSession() {
  if (!isBrowser()) return;
  localStorage.removeItem(AUTH_SESSION_KEY);
  window.dispatchEvent(new Event(AUTH_CHANGED_EVENT));
}
