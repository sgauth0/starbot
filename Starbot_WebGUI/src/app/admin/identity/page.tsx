'use client';

import { useRef } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { IdCard, Save } from 'lucide-react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Textarea } from '@/components/ui/textarea';
import { memoryApi } from '@/lib/api/memory';

function formatTimestamp(value?: string) {
  if (!value) return 'Never';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return 'Unknown';
  return parsed.toLocaleString();
}

export default function AdminIdentityPage() {
  const editorRef = useRef<HTMLTextAreaElement | null>(null);

  const identityQuery = useQuery({
    queryKey: ['admin-identity'],
    queryFn: memoryApi.getIdentity,
  });

  const saveMutation = useMutation({
    mutationFn: async () => {
      const content = editorRef.current?.value || '';
      await memoryApi.updateIdentity(content);
      await memoryApi.processIdentity();
    },
    onSuccess: async () => {
      await identityQuery.refetch();
      toast.success('Identity saved');
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Failed to save identity');
    },
  });

  return (
    <div className="space-y-6">
      <header className="rounded-2xl border border-slate-200/90 bg-gradient-to-r from-slate-50 to-white p-5">
        <p className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">Identity</p>
        <h1 className="mt-1 flex items-center gap-2 text-2xl font-semibold text-slate-900">
          <IdCard className="h-6 w-6 text-slate-700" />
          Global `IDENTITY.md`
        </h1>
        <p className="mt-2 text-sm text-slate-600">
          Last updated: {formatTimestamp(identityQuery.data?.updatedAt)}
        </p>
      </header>

      <Card className="rounded-2xl border-slate-200/90 bg-white/90">
        <CardHeader>
          <CardTitle className="text-slate-900">Identity Document</CardTitle>
          <CardDescription>
            This content is shared across all chats and injected into generation context.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <Textarea
            key={identityQuery.data?.updatedAt || 'identity-default'}
            ref={editorRef}
            defaultValue={identityQuery.data?.content || ''}
            placeholder="Define system identity, behavior constraints, and response style."
            className="min-h-[420px] rounded-xl border-slate-300 font-mono text-sm"
          />
          <div className="flex justify-end">
            <Button
              type="button"
              onClick={() => saveMutation.mutate()}
              disabled={saveMutation.isPending || identityQuery.isLoading}
              className="rounded-xl bg-slate-900 text-white hover:bg-slate-800"
            >
              <Save className="mr-2 h-4 w-4" />
              {saveMutation.isPending ? 'Saving...' : 'Save Identity'}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
