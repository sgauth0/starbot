'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { ArrowLeft, Clock3, Mail, UserRound } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  AUTH_CHANGED_EVENT,
  clearAuthSession,
  clearServerSession,
  readAuthSession,
  type AuthSession,
} from '@/lib/auth-session';

export default function AccountPage() {
  const router = useRouter();
  const [session, setSession] = useState<AuthSession | null>(null);

  useEffect(() => {
    const refresh = () => setSession(readAuthSession());
    refresh();

    window.addEventListener(AUTH_CHANGED_EVENT, refresh);
    return () => window.removeEventListener(AUTH_CHANGED_EVENT, refresh);
  }, []);

  if (!session) {
    return (
      <main className="mx-auto flex min-h-screen w-full max-w-3xl items-center justify-center p-6">
        <div className="w-full rounded-3xl border border-slate-200 bg-white p-8 shadow-sm">
          <h1 className="text-xl font-semibold text-slate-900">No active session</h1>
          <p className="mt-2 text-sm text-slate-600">Sign in or continue as guest to access account details.</p>
          <div className="mt-6">
            <Button onClick={() => router.push('/login')} className="bg-slate-900 text-white hover:bg-slate-800">
              Go to login
            </Button>
          </div>
        </div>
      </main>
    );
  }

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-3xl items-start p-6 pt-12">
      <section className="w-full rounded-3xl border border-slate-200 bg-white p-8 shadow-sm">
        <button
          type="button"
          onClick={() => router.push('/')}
          className="mb-6 inline-flex items-center gap-2 rounded-lg border border-slate-300 bg-white px-3 py-1.5 text-sm text-slate-700 hover:bg-slate-100"
        >
          <ArrowLeft className="h-4 w-4" />
          Back
        </button>

        <h1 className="text-2xl font-semibold text-slate-900">Account</h1>
        <p className="mt-1 text-sm text-slate-600">Session details for this browser.</p>

        <div className="mt-6 space-y-4">
          <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4">
            <p className="text-xs font-medium uppercase tracking-wide text-slate-500">Name</p>
            <p className="mt-1 flex items-center gap-2 text-sm text-slate-900">
              <UserRound className="h-4 w-4 text-slate-500" />
              {session.name}
            </p>
          </div>

          <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4">
            <p className="text-xs font-medium uppercase tracking-wide text-slate-500">Email</p>
            <p className="mt-1 flex items-center gap-2 text-sm text-slate-900">
              <Mail className="h-4 w-4 text-slate-500" />
              {session.email}
            </p>
          </div>

          <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4">
            <p className="text-xs font-medium uppercase tracking-wide text-slate-500">Signed in at</p>
            <p className="mt-1 flex items-center gap-2 text-sm text-slate-900">
              <Clock3 className="h-4 w-4 text-slate-500" />
              {new Date(session.loggedInAt).toLocaleString()}
            </p>
          </div>

          <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4">
            <p className="text-xs font-medium uppercase tracking-wide text-slate-500">Role</p>
            <p className="mt-1 text-sm text-slate-900">{session.role || 'user'}</p>
          </div>
        </div>

        <div className="mt-8">
          <Button
            variant="outline"
            className="border-red-300 text-red-600 hover:bg-red-50 hover:text-red-700"
            onClick={async () => {
              await clearServerSession();
              clearAuthSession();
              router.push('/login');
            }}
          >
            Sign out
          </Button>
        </div>
      </section>
    </main>
  );
}
