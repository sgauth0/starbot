'use client';

import { FormEvent, useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';
import { ArrowLeft, Lock, Mail, UserRound } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { syncServerSession, writeAuthSession } from '@/lib/auth-session';
import { toast } from 'sonner';

export default function SignupPage() {
  const router = useRouter();
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');

  const canSubmit = useMemo(() => {
    return (
      name.trim().length > 1 &&
      email.trim().length > 3 &&
      password.length >= 6 &&
      password === confirmPassword
    );
  }, [name, email, password, confirmPassword]);

  const handleSignup = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!canSubmit) return;

    const role = await syncServerSession({
      name: name.trim(),
      email: email.trim(),
    });

    writeAuthSession({
      name: name.trim(),
      email: email.trim(),
      loggedInAt: new Date().toISOString(),
      role,
    });

    toast.success('Account created');
    router.push('/');
  };

  return (
    <div className="relative min-h-screen overflow-hidden bg-background">
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(56,189,248,0.18),transparent_40%),radial-gradient(circle_at_bottom_right,rgba(2,132,199,0.16),transparent_45%)]" />

      <div className="relative mx-auto flex min-h-screen w-full max-w-6xl items-center justify-center p-6">
        <div className="grid w-full max-w-5xl overflow-hidden rounded-3xl border border-slate-200/80 bg-white/85 shadow-[0_24px_55px_rgba(15,23,42,0.10)] md:grid-cols-[1.1fr_1fr]">
          <section className="hidden bg-gradient-to-br from-slate-900 via-slate-800 to-sky-900 p-10 text-slate-100 md:flex md:flex-col md:justify-between">
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-300">Starbot</p>
              <h1 className="mt-4 text-3xl font-semibold leading-tight">Create your account</h1>
              <p className="mt-3 text-sm text-slate-300">
                Sign up once to keep your workspace and preferences available on this device.
              </p>
            </div>
            <p className="text-xs text-slate-300">Built for focused AI workflows</p>
          </section>

          <section className="p-6 sm:p-8">
            <button
              type="button"
              onClick={() => router.push('/login')}
              className="mb-5 inline-flex items-center gap-2 rounded-lg border border-slate-300 bg-white px-3 py-1.5 text-sm text-slate-700 hover:bg-slate-100"
            >
              <ArrowLeft className="h-4 w-4" />
              Back to login
            </button>

            <h2 className="text-2xl font-semibold text-slate-900">Sign up</h2>
            <p className="mt-1 text-sm text-slate-600">Create your account to continue.</p>

            <form className="mt-6 space-y-4" onSubmit={handleSignup}>
              <label className="block space-y-1">
                <span className="text-xs font-medium uppercase tracking-wide text-slate-500">Name</span>
                <div className="relative">
                  <UserRound className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400" />
                  <Input
                    required
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    placeholder="Your name"
                    className="h-11 rounded-xl border-slate-300 pl-9 focus-visible:ring-slate-400"
                  />
                </div>
              </label>

              <label className="block space-y-1">
                <span className="text-xs font-medium uppercase tracking-wide text-slate-500">Email</span>
                <div className="relative">
                  <Mail className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400" />
                  <Input
                    type="email"
                    required
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    placeholder="you@example.com"
                    className="h-11 rounded-xl border-slate-300 pl-9 focus-visible:ring-slate-400"
                  />
                </div>
              </label>

              <label className="block space-y-1">
                <span className="text-xs font-medium uppercase tracking-wide text-slate-500">Password</span>
                <div className="relative">
                  <Lock className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400" />
                  <Input
                    type="password"
                    required
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="At least 6 characters"
                    className="h-11 rounded-xl border-slate-300 pl-9 focus-visible:ring-slate-400"
                  />
                </div>
              </label>

              <label className="block space-y-1">
                <span className="text-xs font-medium uppercase tracking-wide text-slate-500">Confirm password</span>
                <div className="relative">
                  <Lock className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400" />
                  <Input
                    type="password"
                    required
                    value={confirmPassword}
                    onChange={(e) => setConfirmPassword(e.target.value)}
                    placeholder="Repeat password"
                    className="h-11 rounded-xl border-slate-300 pl-9 focus-visible:ring-slate-400"
                  />
                </div>
              </label>

              <div className="space-y-2 pt-2">
                <Button
                  type="submit"
                  disabled={!canSubmit}
                  className="h-11 w-full rounded-xl bg-slate-900 text-white hover:bg-slate-800"
                >
                  Create account
                </Button>
              </div>

              <p className="pt-1 text-center text-sm text-slate-600">
                Already have an account?{' '}
                <button
                  type="button"
                  onClick={() => router.push('/login')}
                  className="font-medium text-slate-900 underline underline-offset-4 hover:text-slate-700"
                >
                  Sign in
                </button>
              </p>
            </form>
          </section>
        </div>
      </div>
    </div>
  );
}
