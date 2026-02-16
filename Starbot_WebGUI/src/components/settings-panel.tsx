import { useUIStore } from '@/store/ui-store';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { toast } from 'sonner';
import { Bot, Brain, Gauge, Sparkles, X } from 'lucide-react';

export function SettingsPanel() {
  const { isSettingsOpen, setSettingsOpen, settings, updateSettings } = useUIStore();
  const [modelPrefs, setModelPrefs] = useState(settings.model_prefs || '');

  const saveModelPrefs = () => {
    updateSettings({ model_prefs: modelPrefs.trim() || undefined });
    toast.success('Model preference saved');
  };

  return (
    <Dialog open={isSettingsOpen} onOpenChange={setSettingsOpen}>
      <DialogContent
        className="sm:max-w-3xl border-slate-200 bg-white p-0"
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
          <DialogTitle className="text-xl text-slate-900">Workspace Settings</DialogTitle>
          <DialogDescription className="text-slate-600">
            Configure routing behavior and model preferences.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-5 px-6 py-6 max-h-[78vh] overflow-y-auto">
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
                className={`flex-1 rounded-xl ${settings.speed === true
                  ? 'bg-slate-900 text-white border-slate-900 hover:bg-slate-800 hover:text-white'
                  : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-50'}`}
              >
                Fast
              </Button>
              <Button
                type="button"
                variant="outline"
                onClick={() => updateSettings({ speed: false })}
                className={`flex-1 rounded-xl ${settings.speed === false
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
              <p className="text-xs text-slate-500">Let the API auto-route based on request complexity.</p>
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
              <p className="text-xs text-slate-500">Optional: provider or provider:model (for example `azure` or `azure:gpt-5.2-chat`).</p>
            </div>
            <div className="flex gap-2">
              <Input
                value={modelPrefs}
                onChange={(e) => setModelPrefs(e.target.value)}
                placeholder="auto"
                className="rounded-xl border-slate-300 focus-visible:ring-slate-400"
              />
              <Button type="button" onClick={saveModelPrefs} size="sm" className="rounded-xl bg-slate-900 text-white hover:bg-slate-800">Apply</Button>
            </div>
          </section>
        </div>
      </DialogContent>
    </Dialog>
  );
}
