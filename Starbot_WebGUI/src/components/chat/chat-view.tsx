import { useChatStream } from '@/hooks/use-chat-stream';
import { useUIStore } from '@/store/ui-store';
import { MessageList } from './message-list';
import { ChatInput } from './chat-input';
import { messagesApi } from '@/lib/api/messages';
import { chatsApi } from '@/lib/api/chats';
import { projectsApi } from '@/lib/api/projects';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { MessageSquare, Plus } from 'lucide-react';
import { Message } from '@/lib/types';
import { toast } from 'sonner';
import { MessageSkeleton } from './message-skeleton';
import { Button } from '@/components/ui/button';
import { ApiError } from '@/lib/api';

export function ChatView() {
  const { selectedChatId, settings, setDraftInput, setSelectedChatId } = useUIStore();
  const { messages, isLoading, status, startStream } = useChatStream(selectedChatId);
  const queryClient = useQueryClient();

  const { data: projects } = useQuery({
    queryKey: ['projects'],
    queryFn: projectsApi.list,
  });
  const currentProjectId = projects?.[0]?.id;

  const createChatMutation = useMutation({
    mutationFn: async () => {
      let projectId = currentProjectId;

      if (!projectId) {
        const project = await projectsApi.create({ name: 'My Project' });
        projectId = project.id;
      }

      return chatsApi.create(projectId, { title: 'New Chat' });
    },
    onSuccess: (newChat) => {
      const projectIdForCache = newChat.projectId || currentProjectId;
      queryClient.invalidateQueries({ queryKey: ['projects'] });
      if (projectIdForCache) {
        queryClient.invalidateQueries({ queryKey: ['chats', projectIdForCache] });
      }
      setSelectedChatId(newChat.id);
    },
    onError: (error) => {
      const message = error instanceof ApiError
        ? error.message
        : error instanceof Error
          ? error.message
          : 'Failed to create chat';
      toast.error(message);
    },
  });

  const sendMutation = useMutation({
    mutationFn: (content: string) => {
      if (!selectedChatId) throw new Error('No chat selected');
      return messagesApi.send(selectedChatId, content, 'user');
    },
    onMutate: async (content) => {
        // Optimistic update: add user message to UI immediately
        if (!selectedChatId) return;

        await queryClient.cancelQueries({ queryKey: ['messages', selectedChatId] });

        const previousMessages = queryClient.getQueryData(['messages', selectedChatId]);

        queryClient.setQueryData(['messages', selectedChatId], (old: Message[] = []) => [
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
    onSuccess: () => {
        // After message is sent, trigger the assistant response
        if (selectedChatId) {
            startStream(settings);
        }
    },
    onError: (err, content, context) => {
        if (selectedChatId && context?.previousMessages) {
            queryClient.setQueryData(['messages', selectedChatId], context.previousMessages);
        }
    }
  });

  const deleteMutation = useMutation({
    mutationFn: (messageId: string) => messagesApi.delete(messageId),
    onMutate: async (messageId) => {
      if (!selectedChatId) return;

      await queryClient.cancelQueries({ queryKey: ['messages', selectedChatId] });
      const previousMessages = queryClient.getQueryData(['messages', selectedChatId]);

      queryClient.setQueryData(['messages', selectedChatId], (old: Message[] = []) =>
        old.filter((msg) => msg.id !== messageId)
      );

      return { previousMessages };
    },
    onError: (err, messageId, context) => {
      if (selectedChatId && context?.previousMessages) {
        queryClient.setQueryData(['messages', selectedChatId], context.previousMessages);
      }
      toast.error('Failed to delete message');
    },
    onSuccess: () => {
      toast.success('Message deleted');
    },
  });

  const handleSend = (content: string) => {
    if (selectedChatId) {
      sendMutation.mutate(content);
    }
  };

  const handleEditMessage = async (message: Message) => {
    if (!selectedChatId) return;

    try {
      // Set the draft input to the message content
      setDraftInput(message.content);

      // Delete this message and all subsequent messages
      await messagesApi.deleteAfter(selectedChatId, message.id);

      // Invalidate and refetch messages
      queryClient.invalidateQueries({ queryKey: ['messages', selectedChatId] });

      toast.success('Message ready to edit');
    } catch (error) {
      toast.error('Failed to edit message');
    }
  };

  const handleDeleteMessage = (messageId: string) => {
    deleteMutation.mutate(messageId);
  };

  const handleRegenerateMessage = async (messageId: string) => {
    if (!selectedChatId) return;

    try {
      // Delete this message and all subsequent messages
      await messagesApi.deleteAfter(selectedChatId, messageId);

      // Invalidate queries to refresh the message list
      await queryClient.invalidateQueries({ queryKey: ['messages', selectedChatId] });

      // Trigger a new stream for the assistant response
      startStream(settings);

      toast.success('Regenerating response');
    } catch (error) {
      toast.error('Failed to regenerate message');
    }
  };

  if (!selectedChatId) {
    return (
      <div className="flex-1 flex items-center justify-center p-6">
        <div className="w-full max-w-xl rounded-3xl border border-slate-200/80 bg-white/80 p-8 shadow-[0_20px_50px_rgba(15,23,42,0.08)] text-center space-y-5">
          <div className="flex justify-center">
            <div className="rounded-2xl bg-gradient-to-br from-slate-900 to-slate-700 p-5 shadow-lg">
              <MessageSquare className="h-10 w-10 text-slate-50" />
            </div>
          </div>
          <div className="space-y-2">
            <h3 className="text-xl font-semibold text-slate-900">Start A Conversation</h3>
            <p className="text-sm text-slate-600 max-w-sm mx-auto">
              Create a chat to begin. You can switch mode, speed, and model preferences in Settings.
            </p>
          </div>
          <Button
            onClick={() => createChatMutation.mutate()}
            disabled={createChatMutation.isPending}
            className="bg-slate-900 text-slate-50 hover:bg-slate-800"
          >
            <Plus className="h-4 w-4 mr-2" />
            {createChatMutation.isPending ? 'Creating...' : 'New Chat'}
          </Button>
        </div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex-1 flex flex-col h-full overflow-hidden p-3 md:p-4">
        <MessageSkeleton />
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col h-full overflow-hidden relative">
      <MessageList
        messages={messages || []}
        status={status}
        onEditMessage={handleEditMessage}
        onDeleteMessage={handleDeleteMessage}
        onRegenerateMessage={handleRegenerateMessage}
      />
      <ChatInput onSend={handleSend} disabled={!!status} />
    </div>
  );
}
