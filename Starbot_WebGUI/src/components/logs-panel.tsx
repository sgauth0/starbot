import { useUIStore } from '@/store/ui-store';
import { Button } from '@/components/ui/button';
import { X, Terminal } from 'lucide-react';
import { useQueryClient } from '@tanstack/react-query';
import { Message } from '@/lib/types';
import { ScrollArea } from '@/components/ui/scroll-area';

export function LogsPanel() {
  const { isLogsOpen, toggleLogs, selectedChatId } = useUIStore();
  const queryClient = useQueryClient();

  const messages = selectedChatId 
    ? queryClient.getQueryData<Message[]>(['messages', selectedChatId]) 
    : [];

  const logs = messages?.filter(m => m.role === 'tool' || m.role === 'system') || [];

  if (!isLogsOpen) return null;

  return (
    <div className="absolute inset-y-0 right-0 z-40 bg-white w-96 shadow-xl border-l flex flex-col animate-in slide-in-from-right duration-200">
        <div className="flex items-center justify-between p-4 border-b bg-slate-50">
          <div className="flex items-center gap-2">
            <Terminal className="h-4 w-4" />
            <h2 className="text-sm font-semibold">Diagnostics / Logs</h2>
          </div>
          <Button variant="ghost" size="icon" onClick={toggleLogs}>
            <X className="h-4 w-4" />
          </Button>
        </div>

        <ScrollArea className="flex-1 p-4">
            {logs.length === 0 ? (
                <div className="text-sm text-slate-500 italic">No logs available for this chat.</div>
            ) : (
                <div className="space-y-4">
                    {logs.map((log, i) => (
                        <div key={i} className="text-xs font-mono space-y-1 border-b pb-2 last:border-0">
                            <div className="font-semibold text-slate-700 uppercase">{log.role}</div>
                            <div className="text-slate-600 whitespace-pre-wrap">{log.content}</div>
                            {log.metadata && (
                                <pre className="bg-slate-100 p-1 rounded text-[10px] overflow-auto">
                                    {JSON.stringify(log.metadata, null, 2)}
                                </pre>
                            )}
                        </div>
                    ))}
                </div>
            )}
        </ScrollArea>
    </div>
  );
}
