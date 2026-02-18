function resolveApiBaseUrl() {
  const explicit = process.env.NEXT_PUBLIC_API_URL?.trim();
  if (typeof window !== 'undefined') {
    const host = window.location.hostname;

    if (explicit) {
      // Keep same-origin behavior when the configured URL points at a different host
      // (for example console.starbot.cloud using an old starbot.cloud env value).
      if (explicit.startsWith('/')) {
        return explicit;
      }

      try {
        const parsed = new URL(explicit);
        if (parsed.hostname !== host) {
          return '/v1';
        }
      } catch {
        // Fall through to explicit value if URL parsing fails.
      }

      return explicit;
    }

    // In production we want browser calls to stay same-origin so Caddy can route /v1.
    if (host === 'localhost' || host === '127.0.0.1') {
      return 'http://localhost:3737/v1';
    }
    return '/v1';
  }

  if (explicit) return explicit;
  return 'http://localhost:3737/v1';
}

export const API_BASE_URL = resolveApiBaseUrl();
