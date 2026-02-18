import { NextRequest, NextResponse } from 'next/server';
import { getAdminCookieName, verifyAdminCookieToken } from '@/lib/server/admin-cookie';

function resolveConsoleHosts(): Set<string> {
  const configured = String(process.env.ADMIN_CONSOLE_HOSTS || process.env.NEXT_PUBLIC_ADMIN_CONSOLE_HOSTS || '')
    .split(',')
    .map((entry) => entry.trim().toLowerCase())
    .filter(Boolean);

  if (configured.length > 0) {
    return new Set(configured);
  }

  return new Set([
    'console.sgauth0.com',
    'www.console.sgauth0.com',
    'console.starbot.cloud',
    'www.console.starbot.cloud',
  ]);
}

const CONSOLE_HOSTS = resolveConsoleHosts();

function normalizeHost(rawHost: string | null): string {
  if (!rawHost) return '';
  return rawHost.split(':')[0].toLowerCase();
}

const ADMIN_COOKIE_NAME = getAdminCookieName();

function isPublicPath(pathname: string): boolean {
  return pathname === '/login' || pathname.startsWith('/api/auth/session');
}

export async function proxy(request: NextRequest) {
  const host = normalizeHost(request.headers.get('host'));
  if (!CONSOLE_HOSTS.has(host)) {
    return NextResponse.next();
  }

  const { pathname } = request.nextUrl;
  const token = request.cookies.get(ADMIN_COOKIE_NAME)?.value;
  const isAdmin = await verifyAdminCookieToken(token);

  if (!isAdmin && !isPublicPath(pathname)) {
    const url = request.nextUrl.clone();
    url.pathname = '/login';
    const nextPath = `${request.nextUrl.pathname}${request.nextUrl.search}`;
    url.searchParams.set('next', nextPath);
    return NextResponse.redirect(url);
  }

  if (isAdmin && pathname === '/login') {
    const url = request.nextUrl.clone();
    url.pathname = '/admin';
    url.search = '';
    return NextResponse.redirect(url);
  }

  if (isAdmin && pathname === '/') {
    const url = request.nextUrl.clone();
    url.pathname = '/admin';
    return NextResponse.rewrite(url);
  }

  return NextResponse.next();
}

export const config = {
  matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
};
