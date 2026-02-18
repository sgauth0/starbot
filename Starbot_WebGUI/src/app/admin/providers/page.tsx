'use client';

import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { z } from 'zod';
import { Boxes, Database } from 'lucide-react';
import { api } from '@/lib/api';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';

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

export default function AdminProvidersPage() {
  const providersQuery = useQuery({
    queryKey: ['admin-providers-all'],
    queryFn: () => api.get('/models', ProvidersSchema),
    refetchInterval: 60_000,
  });

  const groupedProviders = useMemo(() => {
    const byProvider = new Map<string, z.infer<typeof ProvidersSchema>['providers']>();
    const allProviders = providersQuery.data?.providers || [];

    for (const item of allProviders) {
      if (!item.provider) continue;
      const key = item.provider;
      const next = byProvider.get(key) || [];
      next.push(item);
      byProvider.set(key, next);
    }

    return Array.from(byProvider.entries())
      .map(([provider, models]) => ({
        provider,
        models: models.sort((a, b) => (a.model || '').localeCompare(b.model || '')),
      }))
      .sort((a, b) => a.provider.localeCompare(b.provider));
  }, [providersQuery.data]);

  return (
    <div className="space-y-6">
      <header className="rounded-2xl border border-slate-200/90 bg-gradient-to-r from-slate-50 to-white p-5">
        <p className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">Providers</p>
        <h1 className="mt-1 text-2xl font-semibold text-slate-900">Model Inventory</h1>
        <p className="mt-2 text-sm text-slate-600">
          Default provider: <span className="font-medium text-slate-800">{providersQuery.data?.defaultProvider || 'n/a'}</span>
        </p>
      </header>

      <section className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
        {groupedProviders.map(({ provider, models }) => (
          <Card key={provider} className="rounded-2xl border-slate-200/90 bg-white/90">
            <CardHeader className="pb-2">
              <CardTitle className="flex items-center gap-2 text-slate-900">
                <Boxes className="h-4 w-4 text-slate-600" />
                {provider}
              </CardTitle>
              <CardDescription>{models.length} model{models.length === 1 ? '' : 's'}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-2">
              {models.map((model) => (
                <div key={model.id} className="rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-xs text-slate-700">
                  <div className="font-medium text-slate-900">{model.model || model.label || model.id}</div>
                  <div className="mt-1 flex flex-wrap gap-1">
                    {(model.capabilities || []).slice(0, 5).map((capability) => (
                      <span
                        key={capability}
                        className="rounded-md border border-slate-300 bg-white px-1.5 py-0.5 text-[11px] text-slate-600"
                      >
                        {capability}
                      </span>
                    ))}
                    {typeof model.tier === 'number' && (
                      <span className="rounded-md border border-slate-300 bg-white px-1.5 py-0.5 text-[11px] text-slate-600">
                        tier {model.tier}
                      </span>
                    )}
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
        ))}
      </section>

      {!providersQuery.isLoading && groupedProviders.length === 0 && (
        <div className="rounded-2xl border border-dashed border-slate-300 bg-slate-50 px-4 py-8 text-center text-sm text-slate-500">
          <Database className="mx-auto mb-2 h-5 w-5 text-slate-400" />
          No provider models were returned by `/v1/models`.
        </div>
      )}
    </div>
  );
}
