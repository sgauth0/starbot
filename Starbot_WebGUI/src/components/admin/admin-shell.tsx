'use client';

import Link from 'next/link';
import { usePathname, useRouter } from 'next/navigation';
import { useSyncExternalStore } from 'react';
import { Boxes, ExternalLink, IdCard, LayoutDashboard, LogOut, Shield } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import {
  AUTH_CHANGED_EVENT,
  AUTH_SESSION_KEY,
  clearAuthSession,
  clearServerSession,
  readAuthSession,
} from '@/lib/auth-session';

const CHAT_APP_URL = process.env.NEXT_PUBLIC_CHAT_APP_URL || 'https://starbot.cloud';

const NAV_ITEMS = [
  {
    href: '/admin',
    label: 'Overview',
    icon: LayoutDashboard,
  },
  {
    href: '/admin/providers',
    label: 'Providers',
    icon: Boxes,
  },
  {
    href: '/admin/identity',
    label: 'Identity',
    icon: IdCard,
  },
];

function subscribeAuthSession(onStoreChange: () => void) {
  if (typeof window === 'undefined') return () => {};

  const onStorage = (event: StorageEvent) => {
    if (!event.key || event.key === AUTH_SESSION_KEY) {
      onStoreChange();
    }
  };

  window.addEventListener('storage', onStorage);
  window.addEventListener(AUTH_CHANGED_EVENT, onStoreChange);
  return () => {
    window.removeEventListener('storage', onStorage);
    window.removeEventListener(AUTH_CHANGED_EVENT, onStoreChange);
  };
}

export function AdminShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const router = useRouter();
  const session = useSyncExternalStore(subscribeAuthSession, readAuthSession, () => null);

  const handleSignOut = async () => {
    await clearServerSession();
    clearAuthSession();
    router.push('/login');
  };

  return (
    <div className="min-h-screen bg-[radial-gradient(circle_at_top_right,rgba(14,165,233,0.14),transparent_45%),linear-gradient(180deg,#f8fafc_0%,#eef2ff_100%)]">
      <div className="mx-auto flex min-h-screen w-full max-w-[1200px] flex-col gap-4 p-4 md:flex-row md:gap-6 md:p-6">
        <aside className="w-full rounded-3xl border border-slate-200/90 bg-white/90 p-4 shadow-[0_18px_45px_rgba(15,23,42,0.08)] md:w-72 md:p-5">
          <div className="mb-5 rounded-2xl border border-slate-200 bg-slate-50 px-4 py-4">
            <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">Starbot Console</p>
            <h1 className="mt-1 flex items-center gap-2 text-lg font-semibold text-slate-900">
              <Shield className="h-5 w-5 text-slate-700" />
              Admin
            </h1>
            <p className="mt-2 text-xs text-slate-600">
              Signed in as {session?.name || session?.email || 'Admin'}
            </p>
          </div>

          <nav className="space-y-1.5">
            {NAV_ITEMS.map((item) => {
              const isActive = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={cn(
                    'flex items-center gap-2 rounded-xl border px-3 py-2 text-sm transition',
                    isActive
                      ? 'border-slate-900 bg-slate-900 text-white'
                      : 'border-slate-200 bg-white text-slate-700 hover:bg-slate-50'
                  )}
                >
                  <item.icon className="h-4 w-4" />
                  {item.label}
                </Link>
              );
            })}
          </nav>

          <div className="mt-6 space-y-2">
            <Button
              type="button"
              variant="outline"
              className="w-full justify-start rounded-xl border-slate-300 text-slate-700 hover:bg-slate-100"
              onClick={() => {
                window.location.href = CHAT_APP_URL;
              }}
            >
              <ExternalLink className="mr-2 h-4 w-4" />
              Open Chat App
            </Button>
            <Button
              type="button"
              variant="outline"
              className="w-full justify-start rounded-xl border-red-300 text-red-600 hover:bg-red-50 hover:text-red-700"
              onClick={handleSignOut}
            >
              <LogOut className="mr-2 h-4 w-4" />
              Sign out
            </Button>
          </div>
        </aside>

        <main className="flex-1 rounded-3xl border border-slate-200/90 bg-white/90 p-4 shadow-[0_18px_45px_rgba(15,23,42,0.08)] md:p-6">
          {children}
        </main>
      </div>
    </div>
  );
}
