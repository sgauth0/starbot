import { Message } from '@/lib/types';
import { cn } from '@/lib/utils';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useEffect, useRef } from 'react';
import { MarkdownContent } from './markdown-content';
import { MessageActions } from './message-actions';

interface MessageListProps {
  messages: Message[];
  status?: string;
  onEditMessage?: (message: Message) => void;
  onDeleteMessage?: (messageId: string) => void;
  onRegenerateMessage?: (messageId: string) => void;
}

export function MessageList({
  messages,
  status,
  onEditMessage,
  onDeleteMessage,
  onRegenerateMessage
}: MessageListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, status]);

  return (
    <ScrollArea className="flex-1 px-4 py-5 md:px-6" ref={scrollRef}>
      <div className="space-y-4 max-w-4xl mx-auto" role="log" aria-live="polite">
        {messages.map((msg, idx) => (
          <div
            key={msg.id || idx}
            className={cn(
              "group flex flex-col gap-1.5",
              msg.role === 'user' ? "items-end" : "items-start"
            )}
          >
            <div className={cn(
               "flex items-center gap-2 text-xs text-slate-500",
               msg.role === 'user' ? "flex-row-reverse text-right" : "text-left"
            )}>
              <span>{msg.role === 'user' ? 'You' : 'Starbot'}</span>
              <MessageActions
                message={msg}
                onEdit={onEditMessage}
                onDelete={onDeleteMessage}
                onRegenerate={onRegenerateMessage}
              />
            </div>
            <div
              className={cn(
                "rounded-2xl px-4 py-3 max-w-[92%] sm:max-w-[85%] md:max-w-[78%] shadow-sm",
                msg.role === 'user'
                  ? "bg-slate-900 text-slate-50"
                  : "bg-white border border-slate-200 text-slate-900"
              )}
            >
              <MarkdownContent content={msg.content} />
            </div>
            {msg.role === 'tool' && (
                <div className="text-xs text-slate-600 font-mono bg-slate-100 p-2 rounded-lg">
                    Tool Output: {msg.content}
                </div>
            )}
          </div>
        ))}
        {status && (
          <div className="flex items-center gap-2 text-sm text-slate-500 italic pl-1">
            <span>{status}</span>
             <span className="animate-pulse">...</span>
          </div>
        )}
        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  );
}
