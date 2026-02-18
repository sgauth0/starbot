'use client';

import { useEffect, useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';
import { CircleUserRound, LogIn, LogOut, Settings, ShieldCheck, TerminalSquare, UserRound } from 'lucide-react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';
import { useUIStore } from '@/store/ui-store';
import { toast } from 'sonner';
import {
  AUTH_CHANGED_EVENT,
  clearAuthSession,
  clearServerSession,
  readAuthSession,
  type AuthSession,
} from '@/lib/auth-session';

const ADMIN_CONSOLE_URL = process.env.NEXT_PUBLIC_ADMIN_CONSOLE_URL || 'https://console.starbot.cloud';

export function AccountMenu() {
  const router = useRouter();
  const { openSettings, openLogs } = useUIStore();
  const [session, setSession] = useState<AuthSession | null>(null);

  useEffect(() => {
    const refresh = () => setSession(readAuthSession());
    refresh();

    const onStorage = (event: StorageEvent) => {
      if (!event.key || event.key === 'starbot_session') {
        refresh();
      }
    };

    window.addEventListener('storage', onStorage);
    window.addEventListener(AUTH_CHANGED_EVENT, refresh);
    return () => {
      window.removeEventListener('storage', onStorage);
      window.removeEventListener(AUTH_CHANGED_EVENT, refresh);
    };
  }, []);

  const initials = useMemo(() => {
    const label = session?.name || session?.email || 'Guest';
    return label
      .split(/\s+/)
      .map((part) => part[0]?.toUpperCase())
      .filter(Boolean)
      .slice(0, 2)
      .join('') || 'G';
  }, [session]);

  const handleSignOut = async () => {
    await clearServerSession();
    clearAuthSession();
    toast.success('Signed out');
    router.push('/login');
  };

  const openAdminConsole = () => {
    if (typeof window !== 'undefined') {
      window.location.href = ADMIN_CONSOLE_URL;
    }
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          className="h-10 rounded-xl px-2 text-slate-700 hover:bg-slate-100"
          aria-label="Open account menu"
        >
          <span className="mr-2 inline-flex h-7 w-7 items-center justify-center rounded-lg bg-slate-900 text-xs font-semibold text-white">
            {initials}
          </span>
          <CircleUserRound className="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>

      <DropdownMenuContent align="end" className="w-60 rounded-xl border-slate-200 p-2">
        <DropdownMenuLabel className="space-y-0.5 px-2 py-2">
          <div className="text-xs font-semibold uppercase tracking-wide text-slate-500">Account</div>
          <div className="truncate text-sm text-slate-900">{session?.name || 'Guest'}</div>
          <div className="truncate text-xs text-slate-500">{session?.email || 'Not signed in'}</div>
        </DropdownMenuLabel>

        <DropdownMenuSeparator />

        <DropdownMenuItem onClick={() => router.push(session ? '/account' : '/login')}>
          {session ? <UserRound className="mr-2 h-4 w-4" /> : <LogIn className="mr-2 h-4 w-4" />}
          {session ? 'Account' : 'Log in'}
        </DropdownMenuItem>

        <DropdownMenuItem onClick={openSettings}>
          <Settings className="mr-2 h-4 w-4" />
          Settings
        </DropdownMenuItem>

        <DropdownMenuItem onClick={openLogs}>
          <TerminalSquare className="mr-2 h-4 w-4" />
          Diagnostics
        </DropdownMenuItem>

        {session?.role === 'admin' && (
          <DropdownMenuItem onClick={openAdminConsole}>
            <ShieldCheck className="mr-2 h-4 w-4" />
            Admin Console
          </DropdownMenuItem>
        )}

        {session && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={handleSignOut} className="text-red-600 focus:text-red-600">
              <LogOut className="mr-2 h-4 w-4" />
              Sign out
            </DropdownMenuItem>
          </>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
