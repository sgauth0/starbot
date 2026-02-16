import { useUIStore } from '@/store/ui-store';
import { Terminal } from 'lucide-react';
import { useQueryClient } from '@tanstack/react-query';
import { Message } from '@/lib/types';
import { ScrollArea } from '@/components/ui/scroll-area';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';

export function LogsPanel() {
  const { isLogsOpen, setLogsOpen, selectedChatId } = useUIStore();
  const queryClient = useQueryClient();

  const messages = selectedChatId
    ? queryClient.getQueryData<Message[]>(['messages', selectedChatId])
    : [];

  const logs = messages?.filter(m => m.role === 'tool' || m.role === 'system') || [];

  return (
    <Dialog open={isLogsOpen} onOpenChange={setLogsOpen}>
      <DialogContent className="sm:max-w-2xl max-h-[80vh] flex flex-col p-0">
        <DialogHeader className="px-6 pt-6 pb-4 border-b">
          <div className="flex items-center gap-2">
            <Terminal className="h-4 w-4" />
            <DialogTitle>Diagnostics / Logs</DialogTitle>
          </div>
        </DialogHeader>

        <ScrollArea className="flex-1 px-6 py-4">
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
      </DialogContent>
    </Dialog>
  );
}
