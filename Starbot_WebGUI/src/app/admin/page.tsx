'use client';

import { useMemo } from 'react';
import Link from 'next/link';
import { useQuery } from '@tanstack/react-query';
import { z } from 'zod';
import { api } from '@/lib/api';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { BadgeCheck, Boxes, Database, IdCard, Shield, Sparkles } from 'lucide-react';

const HealthSchema = z.object({
  status: z.string(),
  timestamp: z.string(),
  version: z.string(),
});

const ProvidersSchema = z.object({
  defaultProvider: z.string(),
  providers: z.array(z.object({
    id: z.string(),
    label: z.string(),
    provider: z.string().optional(),
    model: z.string().optional(),
    tier: z.number().optional(),
    capabilities: z.array(z.string()).optional(),
  })),
});

function formatTimestamp(value?: string): string {
  if (!value) return 'Unknown';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return 'Unknown';
  return parsed.toLocaleString();
}

export default function AdminPage() {
  const healthQuery = useQuery({
    queryKey: ['admin-health'],
    queryFn: () => api.get('/health', HealthSchema),
    refetchInterval: 30_000,
  });

  const providersQuery = useQuery({
    queryKey: ['admin-providers'],
    queryFn: () => api.get('/models', ProvidersSchema),
    refetchInterval: 60_000,
  });

  const providerSummary = useMemo(() => {
    const providers = providersQuery.data?.providers || [];
    const concreteProviders = providers.filter((item) => item.provider);
    const byProvider = new Map<string, number>();

    for (const item of concreteProviders) {
      const key = item.provider!;
      byProvider.set(key, (byProvider.get(key) || 0) + 1);
    }

    return {
      modelCount: concreteProviders.length,
      providerCount: byProvider.size,
      byProvider: Array.from(byProvider.entries()).sort((a, b) => a[0].localeCompare(b[0])),
    };
  }, [providersQuery.data]);

  return (
    <div className="space-y-6">
      <header className="rounded-2xl border border-slate-200/90 bg-gradient-to-r from-slate-50 to-white p-5">
        <p className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">Overview</p>
        <h1 className="mt-1 text-2xl font-semibold text-slate-900">Admin Control Plane</h1>
        <p className="mt-2 text-sm text-slate-600">
          Runtime status and provider readiness for the admin console host.
        </p>
      </header>

      <section className="grid grid-cols-1 gap-4 md:grid-cols-3">
        <Card className="rounded-2xl border-slate-200/90 bg-white/90">
          <CardHeader className="pb-2">
            <CardDescription className="text-slate-500">API Health</CardDescription>
            <CardTitle className="flex items-center gap-2 text-xl text-slate-900">
              <Shield className="h-5 w-5 text-slate-600" />
              {healthQuery.data?.status?.toUpperCase() || 'UNKNOWN'}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-1 text-sm text-slate-600">
            <div>Version: {healthQuery.data?.version || 'n/a'}</div>
            <div>Updated: {formatTimestamp(healthQuery.data?.timestamp)}</div>
          </CardContent>
        </Card>

        <Card className="rounded-2xl border-slate-200/90 bg-white/90">
          <CardHeader className="pb-2">
            <CardDescription className="text-slate-500">Configured Providers</CardDescription>
            <CardTitle className="flex items-center gap-2 text-xl text-slate-900">
              <Boxes className="h-5 w-5 text-slate-600" />
              {providerSummary.providerCount}
            </CardTitle>
          </CardHeader>
          <CardContent className="text-sm text-slate-600">
            Distinct providers with at least one enabled model.
          </CardContent>
        </Card>

        <Card className="rounded-2xl border-slate-200/90 bg-white/90">
          <CardHeader className="pb-2">
            <CardDescription className="text-slate-500">Enabled Models</CardDescription>
            <CardTitle className="flex items-center gap-2 text-xl text-slate-900">
              <Database className="h-5 w-5 text-slate-600" />
              {providerSummary.modelCount}
            </CardTitle>
          </CardHeader>
          <CardContent className="text-sm text-slate-600">
            Total model entries available to routing.
          </CardContent>
        </Card>
      </section>

      <section className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card className="rounded-2xl border-slate-200/90 bg-white/90">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-lg text-slate-900">
              <BadgeCheck className="h-5 w-5 text-slate-700" />
              Provider Footprint
            </CardTitle>
            <CardDescription>
              Active provider/model footprint from `/v1/models`.
            </CardDescription>
          </CardHeader>
          <CardContent>
            {providerSummary.byProvider.length === 0 ? (
              <div className="rounded-xl border border-dashed border-slate-300 bg-slate-50 px-4 py-6 text-sm text-slate-500">
                No configured providers detected.
              </div>
            ) : (
              <div className="space-y-2">
                {providerSummary.byProvider.map(([provider, count]) => (
                  <div
                    key={provider}
                    className="flex items-center justify-between rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm"
                  >
                    <span className="font-medium text-slate-800">{provider}</span>
                    <span className="text-slate-600">{count} model{count === 1 ? '' : 's'}</span>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="rounded-2xl border-slate-200/90 bg-white/90">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-lg text-slate-900">
              <Sparkles className="h-5 w-5 text-slate-700" />
              Admin Workflows
            </CardTitle>
            <CardDescription>
              Quick access to provider coverage and identity controls.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <Link
              href="/admin/providers"
              className="inline-flex h-10 w-full items-center justify-start rounded-xl border border-slate-300 bg-white px-4 text-sm font-medium text-slate-700 hover:bg-slate-50"
            >
              Open Provider Inventory
            </Link>
            <Link
              href="/admin/identity"
              className="inline-flex h-10 w-full items-center justify-start rounded-xl border border-slate-300 bg-white px-4 text-sm font-medium text-slate-700 hover:bg-slate-50"
            >
              <IdCard className="mr-2 h-4 w-4" />
              Edit Global Identity
            </Link>
            <p className="rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-xs text-slate-500">
              Chat-specific memory editing remains in the main chat Settings panel.
            </p>
          </CardContent>
        </Card>
      </section>
    </div>
  );
}
