import { useChatStream } from '@/hooks/use-chat-stream';
import { useUIStore } from '@/store/ui-store';
import { MessageList } from './message-list';
import { ChatInput } from './chat-input';
import { messagesApi } from '@/lib/api/messages';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Loader2 } from 'lucide-react';

export function ChatView() {
  const { selectedChatId, settings } = useUIStore();
  const { messages, isLoading, status } = useChatStream(selectedChatId);
  const queryClient = useQueryClient();

  const sendMutation = useMutation({
    mutationFn: (content: string) => {
      if (!selectedChatId) throw new Error('No chat selected');
      return messagesApi.send(selectedChatId, content, 'user');
    },
    onMutate: async (content) => {
        // Optimistic update if needed, but the stream might handle it.
        // For now, we rely on the backend to append the user message or the stream to send it back?
        // Usually, we should optimistically add the user message.
        if (!selectedChatId) return;

        await queryClient.cancelQueries({ queryKey: ['messages', selectedChatId] });

        const previousMessages = queryClient.getQueryData(['messages', selectedChatId]);

        queryClient.setQueryData(['messages', selectedChatId], (old: any[] = []) => [
            ...old,
            {
                id: 'temp-user',
                chatId: selectedChatId,
                role: 'user',
                content,
                createdAt: new Date().toISOString(),
            },
        ]);

        return { previousMessages };
    },
    onError: (err, content, context) => {
        if (selectedChatId && context?.previousMessages) {
            queryClient.setQueryData(['messages', selectedChatId], context.previousMessages);
        }
    }
  });

  const handleSend = (content: string) => {
    if (selectedChatId) {
      sendMutation.mutate(content);
    }
  };

  if (!selectedChatId) {
    return (
      <div className="flex-1 flex items-center justify-center text-slate-500">
        Select a chat or create a new one to start.
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex-1 flex items-center justify-center text-slate-500">
        <Loader2 className="h-6 w-6 animate-spin mr-2" />
        Loading messages...
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col h-full overflow-hidden relative">
      <MessageList messages={messages || []} status={status} />
      <ChatInput onSend={handleSend} disabled={!!status} />
    </div>
  );
}
