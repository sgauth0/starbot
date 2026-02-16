'use client';

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { chatsApi } from '@/lib/api/chats';
import { projectsApi } from '@/lib/api/projects';
import { useUIStore } from '@/store/ui-store';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Plus, MessageSquare } from 'lucide-react';
import { cn } from '@/lib/utils';
import { toast } from 'sonner';
import { ApiError } from '@/lib/api';

export function Sidebar() {
  const { 
    selectedChatId, 
    setSelectedChatId, 
    isSidebarOpen
  } = useUIStore();
  
  const queryClient = useQueryClient();

  const { data: projects } = useQuery({
    queryKey: ['projects'],
    queryFn: projectsApi.list,
  });

  // Use first project or create default
  const currentProjectId = projects?.[0]?.id;

  const { data: chats } = useQuery({
    queryKey: ['chats', currentProjectId],
    queryFn: () => currentProjectId ? chatsApi.list(currentProjectId) : Promise.resolve([]),
    enabled: !!currentProjectId,
  });

  const createChatMutation = useMutation({
    mutationFn: async (title: string) => {
      let projectId = currentProjectId;

      if (!projectId) {
        const project = await projectsApi.create({ name: 'My Project' });
        projectId = project.id;
      }

      return chatsApi.create(projectId, { title });
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

  const handleCreateChat = () => {
    createChatMutation.mutate('New Chat');
  };

  if (!isSidebarOpen) return null;

  return (
    <aside className="h-full w-full rounded-3xl border border-slate-200/80 bg-white/80 shadow-[0_16px_40px_rgba(15,23,42,0.08)] backdrop-blur supports-[backdrop-filter]:bg-white/65 flex flex-col overflow-hidden">
      <div className="px-4 py-4 border-b border-slate-200/80 bg-gradient-to-r from-slate-50 to-white">
        <div className="mb-3">
          <p className="text-xs font-semibold uppercase tracking-[0.16em] text-slate-500">Starbot</p>
          <h2 className="text-sm font-semibold text-slate-900">Conversations</h2>
        </div>
        <Button
          onClick={handleCreateChat}
          className="w-full justify-start bg-slate-900 text-slate-50 hover:bg-slate-800 border-0 shadow-sm"
          aria-label="Create new chat"
          disabled={createChatMutation.isPending}
        >
          <Plus className="mr-2 h-4 w-4" />
          {createChatMutation.isPending ? 'Creating...' : 'New Chat'}
        </Button>
      </div>

      <ScrollArea className="flex-1 px-2 py-3">
        <div className="space-y-1.5" role="list" aria-label="Chat list">
          {chats?.map((chat) => (
            <Button
              key={chat.id}
              variant="ghost"
              className={cn(
                "w-full justify-start font-normal h-10 rounded-xl px-3",
                selectedChatId === chat.id
                  ? "bg-slate-900 text-white hover:bg-slate-800 hover:text-white"
                  : "text-slate-700 hover:bg-slate-100"
              )}
              onClick={() => setSelectedChatId(chat.id)}
              aria-label={`Chat: ${chat.title}`}
              aria-current={selectedChatId === chat.id ? "page" : undefined}
            >
              <MessageSquare className="mr-2 h-4 w-4" />
              <span className="truncate">{chat.title}</span>
            </Button>
          ))}
          {!chats?.length && (
            <div className="mx-2 mt-4 rounded-xl border border-dashed border-slate-300 bg-slate-50/80 px-4 py-6 text-sm text-slate-500 text-center">
              No chats yet. Click New Chat to create one.
            </div>
          )}
        </div>
      </ScrollArea>

      <div className="p-4 border-t border-slate-200/80 mt-auto bg-white/70">
        <p className="text-[11px] uppercase tracking-[0.16em] text-slate-500">Use account menu for settings</p>
      </div>
    </aside>
  );
}
