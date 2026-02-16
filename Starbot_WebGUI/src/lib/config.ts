function resolveApiBaseUrl() {
  const explicit = process.env.NEXT_PUBLIC_API_URL?.trim();
  if (explicit) return explicit;

  // In production we want browser calls to stay same-origin so Caddy can route /v1.
  if (typeof window !== 'undefined') {
    const host = window.location.hostname;
    if (host === 'localhost' || host === '127.0.0.1') {
      return 'http://localhost:3737/v1';
    }
    return '/v1';
  }

  return 'http://localhost:3737/v1';
}

export const API_BASE_URL = resolveApiBaseUrl();
