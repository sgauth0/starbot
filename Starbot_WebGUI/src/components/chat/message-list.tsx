import { Message } from '@/lib/types';
import { cn } from '@/lib/utils';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useEffect, useRef } from 'react';

interface MessageListProps {
  messages: Message[];
  status?: string;
}

export function MessageList({ messages, status }: MessageListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, status]);

  return (
    <ScrollArea className="flex-1 p-4" ref={scrollRef}>
      <div className="space-y-4 max-w-3xl mx-auto">
        {messages.map((msg, idx) => (
          <div
            key={msg.id || idx}
            className={cn(
              "flex flex-col gap-1",
              msg.role === 'user' ? "items-end" : "items-start"
            )}
          >
            <div className={cn(
               "text-xs text-slate-500",
               msg.role === 'user' ? "text-right" : "text-left"
            )}>
              {msg.role === 'user' ? 'You' : 'Starbot'}
            </div>
            <div
              className={cn(
                "rounded-lg px-4 py-2 max-w-[85%] whitespace-pre-wrap",
                msg.role === 'user' 
                  ? "bg-slate-900 text-slate-50" 
                  : "bg-white border border-slate-200 text-slate-900"
              )}
            >
              {msg.content}
            </div>
            {msg.role === 'tool' && (
                <div className="text-xs text-slate-500 font-mono bg-slate-100 p-2 rounded">
                    Tool Output: {msg.content}
                </div>
            )}
          </div>
        ))}
        {status && (
          <div className="flex items-center gap-2 text-sm text-slate-500 italic">
            <span>{status}</span>
             <span className="animate-pulse">...</span>
          </div>
        )}
        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  );
}
