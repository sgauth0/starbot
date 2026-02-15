import { useUIStore } from '@/store/ui-store';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { X } from 'lucide-react';
import { useEffect, useState } from 'react';

export function SettingsPanel() {
  const { isSettingsOpen, toggleSettings, settings, updateSettings } = useUIStore();
  const [token, setToken] = useState('');

  useEffect(() => {
    if (isSettingsOpen) {
        setToken(localStorage.getItem('starbot_api_token') || '');
    }
  }, [isSettingsOpen]);

  const saveToken = () => {
      localStorage.setItem('starbot_api_token', token);
  };

  if (!isSettingsOpen) return null;

  return (
    <div className="absolute inset-0 z-50 bg-black/50 flex justify-end">
      <div className="w-80 bg-white h-full shadow-lg border-l p-4 animate-in slide-in-from-right duration-200 overflow-y-auto">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-lg font-semibold">Settings</h2>
          <Button variant="ghost" size="icon" onClick={toggleSettings}>
            <X className="h-4 w-4" />
          </Button>
        </div>

        <div className="space-y-6">
          <div className="space-y-2">
            <label className="text-sm font-medium">API Token</label>
            <div className="flex gap-2">
                <Input 
                    type="password" 
                    value={token} 
                    onChange={(e) => setToken(e.target.value)} 
                    placeholder="Local API Token"
                />
                <Button onClick={saveToken} size="sm">Save</Button>
            </div>
            <p className="text-xs text-slate-500">Token is stored in your browser's local storage.</p>
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">Reasoning Mode</label>
            <div className="flex flex-col gap-2">
              {['standard', 'thorough', 'deep'].map((mode) => (
                <Button
                  key={mode}
                  variant={settings.mode === mode ? 'default' : 'outline'}
                  onClick={() => updateSettings({ mode: mode as any })}
                  className="justify-start"
                >
                  <span className="capitalize">{mode}</span>
                </Button>
              ))}
            </div>
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">Execution Speed</label>
            <div className="flex gap-2">
              <Button
                variant={settings.speed === true ? 'default' : 'outline'}
                onClick={() => updateSettings({ speed: true })}
                className="flex-1"
              >
                Fast
              </Button>
              <Button
                variant={settings.speed === false ? 'default' : 'outline'}
                onClick={() => updateSettings({ speed: false })}
                className="flex-1"
              >
                Quality
              </Button>
            </div>
          </div>

          <div className="space-y-2">
             <label className="flex items-center gap-2 cursor-pointer">
                <input
                    type="checkbox"
                    checked={settings.auto}
                    onChange={(e) => updateSettings({ auto: e.target.checked })}
                    className="rounded border-slate-300"
                />
                <span className="text-sm font-medium">Auto-run Tools</span>
             </label>
          </div>
        </div>
      </div>
    </div>
  );
}
