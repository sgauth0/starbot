import { useEffect, useState } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { useUIStore } from '@/store/ui-store';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { memoryApi } from '@/lib/api/memory';
import { toast } from 'sonner';
import { Bot, Brain, Gauge, IdCard, MessageSquareText, Sparkles, X } from 'lucide-react';

type SettingsTab = 'routing' | 'identity' | 'chat';

function formatTimestamp(value?: string) {
  if (!value) return 'Never';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return 'Unknown';
  return parsed.toLocaleString();
}

export function SettingsPanel() {
  const {
    isSettingsOpen,
    setSettingsOpen,
    settings,
    updateSettings,
    selectedChatId,
  } = useUIStore();

  const [activeTab, setActiveTab] = useState<SettingsTab>('routing');
  const [modelPrefs, setModelPrefs] = useState(settings.model_prefs || '');
  const [identityDraft, setIdentityDraft] = useState('');
  const [chatMemoryDraft, setChatMemoryDraft] = useState('');

  useEffect(() => {
    setModelPrefs(settings.model_prefs || '');
  }, [settings.model_prefs]);

  const identityQuery = useQuery({
    queryKey: ['identity-memory'],
    queryFn: memoryApi.getIdentity,
    enabled: isSettingsOpen,
  });

  const chatMemoryQuery = useQuery({
    queryKey: ['chat-memory', selectedChatId],
    queryFn: () => memoryApi.getChatMemory(selectedChatId!),
    enabled: isSettingsOpen && !!selectedChatId,
  });

  useEffect(() => {
    if (identityQuery.data) {
      setIdentityDraft(identityQuery.data.content);
    }
  }, [identityQuery.data]);

  useEffect(() => {
    if (chatMemoryQuery.data) {
      setChatMemoryDraft(chatMemoryQuery.data.content);
    }
  }, [chatMemoryQuery.data]);

  const saveIdentityMutation = useMutation({
    mutationFn: async (content: string) => {
      await memoryApi.updateIdentity(content);
      await memoryApi.processIdentity();
      return memoryApi.getIdentity();
    },
    onSuccess: () => {
      toast.success('Identity saved');
      identityQuery.refetch();
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Failed to save identity');
    },
  });

  const saveChatMemoryMutation = useMutation({
    mutationFn: async (content: string) => {
      if (!selectedChatId) throw new Error('Select a chat first');
      await memoryApi.updateChatMemory(selectedChatId, content);
      await memoryApi.processChatMemory(selectedChatId);
      return memoryApi.getChatMemory(selectedChatId);
    },
    onSuccess: () => {
      toast.success('Chat memory saved');
      chatMemoryQuery.refetch();
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Failed to save chat memory');
    },
  });

  const saveModelPrefs = () => {
    updateSettings({ model_prefs: modelPrefs.trim() || undefined });
    toast.success('Model preference saved');
  };

  return (
    <Dialog open={isSettingsOpen} onOpenChange={setSettingsOpen}>
      <DialogContent
        className="sm:max-w-4xl border-slate-200 bg-white p-0"
        onEscapeKeyDown={(event) => event.preventDefault()}
        onInteractOutside={(event) => event.preventDefault()}
        onPointerDownOutside={(event) => event.preventDefault()}
      >
        <button
          type="button"
          aria-label="Close settings"
          onClick={() => setSettingsOpen(false)}
          className="absolute right-4 top-4 z-20 inline-flex h-8 w-8 items-center justify-center rounded-lg border border-slate-300 bg-white text-slate-700 shadow-sm transition hover:bg-slate-100 hover:text-slate-900 focus:outline-none focus-visible:ring-2 focus-visible:ring-slate-400"
        >
          <X className="h-4 w-4" />
        </button>

        <DialogHeader className="border-b border-slate-200 bg-gradient-to-r from-slate-50 to-white px-6 py-5">
          <DialogTitle className="text-xl text-slate-900">Settings</DialogTitle>
          <DialogDescription className="text-slate-600">
            Configure routing, global identity, and per-chat memory.
          </DialogDescription>
        </DialogHeader>

        <div className="border-b border-slate-200 px-6 py-3">
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
            <Button
              type="button"
              variant="outline"
              onClick={() => setActiveTab('routing')}
              className={activeTab === 'routing'
                ? 'rounded-xl border-slate-900 bg-slate-900 text-white hover:bg-slate-800 hover:text-white'
                : 'rounded-xl border-slate-300 bg-white text-slate-700 hover:bg-slate-50'}
            >
              <Brain className="mr-2 h-4 w-4" />
              Routing
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => setActiveTab('identity')}
              className={activeTab === 'identity'
                ? 'rounded-xl border-slate-900 bg-slate-900 text-white hover:bg-slate-800 hover:text-white'
                : 'rounded-xl border-slate-300 bg-white text-slate-700 hover:bg-slate-50'}
            >
              <IdCard className="mr-2 h-4 w-4" />
              Identity
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => setActiveTab('chat')}
              className={activeTab === 'chat'
                ? 'rounded-xl border-slate-900 bg-slate-900 text-white hover:bg-slate-800 hover:text-white'
                : 'rounded-xl border-slate-300 bg-white text-slate-700 hover:bg-slate-50'}
            >
              <MessageSquareText className="mr-2 h-4 w-4" />
              Chat Memory
            </Button>
          </div>
        </div>

        <div className="space-y-5 px-6 py-6 max-h-[72vh] overflow-y-auto">
          {activeTab === 'routing' && (
            <>
              <section className="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm">
                <div className="mb-3">
                  <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                    <Brain className="h-4 w-4 text-slate-600" />
                    Reasoning Mode
                  </h3>
                  <p className="text-xs text-slate-500">Controls depth and cost profile.</p>
                </div>

                <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
                  {[
                    { id: 'quick', label: 'Quick', description: 'Fastest responses, minimal depth.' },
                    { id: 'standard', label: 'Standard', description: 'Balanced speed and quality.' },
                    { id: 'deep', label: 'Deep', description: 'Most deliberate and thorough.' },
                  ].map((mode) => (
                    <Button
                      key={mode.id}
                      type="button"
                      variant="outline"
                      onClick={() => updateSettings({ mode: mode.id as 'quick' | 'standard' | 'deep' })}
                      className={`h-auto items-start justify-start px-3 py-3 text-left rounded-xl border ${
                        settings.mode === mode.id
                          ? 'bg-slate-900 text-white border-slate-900 hover:bg-slate-800 hover:text-white'
                          : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-50'
                      }`}
                    >
                      <span className="space-y-1">
                        <span className="block text-sm font-medium">{mode.label}</span>
                        <span className="block text-xs opacity-80">{mode.description}</span>
                      </span>
                    </Button>
                  ))}
                </div>
              </section>

              <section className="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm">
                <div className="mb-3">
                  <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                    <Gauge className="h-4 w-4 text-slate-600" />
                    Execution Speed
                  </h3>
                  <p className="text-xs text-slate-500">Trade output length for faster turnaround.</p>
                </div>
                <div className="flex gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => updateSettings({ speed: true })}
                    className={`flex-1 rounded-xl ${settings.speed
                      ? 'bg-slate-900 text-white border-slate-900 hover:bg-slate-800 hover:text-white'
                      : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-50'}`}
                  >
                    Fast
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => updateSettings({ speed: false })}
                    className={`flex-1 rounded-xl ${!settings.speed
                      ? 'bg-slate-900 text-white border-slate-900 hover:bg-slate-800 hover:text-white'
                      : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-50'}`}
                  >
                    Quality
                  </Button>
                </div>
              </section>

              <section className="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm">
                <div className="mb-3">
                  <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                    <Sparkles className="h-4 w-4 text-slate-600" />
                    Automation
                  </h3>
                  <p className="text-xs text-slate-500">Auto-route based on request complexity.</p>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => updateSettings({ auto: !settings.auto })}
                  className={`rounded-xl ${settings.auto
                    ? 'bg-slate-900 text-white border-slate-900 hover:bg-slate-800 hover:text-white'
                    : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-50'}`}
                >
                  {settings.auto ? 'Auto Routing Enabled' : 'Auto Routing Disabled'}
                </Button>
              </section>

              <section className="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm">
                <div className="mb-3">
                  <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                    <Bot className="h-4 w-4 text-slate-600" />
                    Model Preference
                  </h3>
                  <p className="text-xs text-slate-500">
                    Optional: provider or provider:model (example: `azure` or `azure:gpt-5.2-chat`).
                  </p>
                </div>
                <div className="flex gap-2">
                  <Input
                    value={modelPrefs}
                    onChange={(e) => setModelPrefs(e.target.value)}
                    placeholder="auto"
                    className="rounded-xl border-slate-300 focus-visible:ring-slate-400"
                  />
                  <Button
                    type="button"
                    onClick={saveModelPrefs}
                    size="sm"
                    className="rounded-xl bg-slate-900 text-white hover:bg-slate-800"
                  >
                    Apply
                  </Button>
                </div>
              </section>
            </>
          )}

          {activeTab === 'identity' && (
            <section className="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm space-y-3">
              <div>
                <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                  <IdCard className="h-4 w-4 text-slate-600" />
                  Global Identity (`IDENTITY.md`)
                </h3>
                <p className="text-xs text-slate-500">
                  Shared across all chats. Updated: {formatTimestamp(identityQuery.data?.updatedAt)}
                </p>
              </div>
              <Textarea
                value={identityDraft}
                onChange={(event) => setIdentityDraft(event.target.value)}
                placeholder="Describe assistant identity, style, and persistent global rules."
                className="min-h-[300px] rounded-xl border-slate-300 font-mono text-sm"
              />
              <div className="flex justify-end">
                <Button
                  type="button"
                  onClick={() => saveIdentityMutation.mutate(identityDraft)}
                  disabled={saveIdentityMutation.isPending || identityQuery.isLoading}
                  className="rounded-xl bg-slate-900 text-white hover:bg-slate-800"
                >
                  {saveIdentityMutation.isPending ? 'Saving...' : 'Save Identity'}
                </Button>
              </div>
            </section>
          )}

          {activeTab === 'chat' && (
            <section className="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm space-y-3">
              <div>
                <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                  <MessageSquareText className="h-4 w-4 text-slate-600" />
                  Chat Memory (`MEMORY.md`)
                </h3>
                <p className="text-xs text-slate-500">
                  {selectedChatId
                    ? `Applies only to this chat. Updated: ${formatTimestamp(chatMemoryQuery.data?.updatedAt)}`
                    : 'Select a chat to edit chat memory.'}
                </p>
              </div>

              {selectedChatId ? (
                <>
                  <Textarea
                    value={chatMemoryDraft}
                    onChange={(event) => setChatMemoryDraft(event.target.value)}
                    placeholder="Store thread-specific facts and decisions."
                    className="min-h-[300px] rounded-xl border-slate-300 font-mono text-sm"
                  />
                  <div className="flex justify-end">
                    <Button
                      type="button"
                      onClick={() => saveChatMemoryMutation.mutate(chatMemoryDraft)}
                      disabled={saveChatMemoryMutation.isPending || chatMemoryQuery.isLoading}
                      className="rounded-xl bg-slate-900 text-white hover:bg-slate-800"
                    >
                      {saveChatMemoryMutation.isPending ? 'Saving...' : 'Save Chat Memory'}
                    </Button>
                  </div>
                </>
              ) : (
                <div className="rounded-xl border border-dashed border-slate-300 bg-slate-50 px-4 py-6 text-center text-sm text-slate-500">
                  Create or select a chat, then reopen this tab to edit chat memory.
                </div>
              )}
            </section>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
