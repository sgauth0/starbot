'use client';

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { chatsApi } from '@/lib/api/chats';
import { projectsApi } from '@/lib/api/projects';
import { useUIStore } from '@/store/ui-store';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Plus, MessageSquare, Settings, Folder } from 'lucide-react';
import { cn } from '@/lib/utils';
import { useRouter } from 'next/navigation';

export function Sidebar() {
  const { 
    selectedChatId, 
    setSelectedChatId, 
    isSidebarOpen, 
    toggleSettings 
  } = useUIStore();
  
  const queryClient = useQueryClient();
  const router = useRouter();

  const { data: projects } = useQuery({
    queryKey: ['projects'],
    queryFn: projectsApi.list,
  });

  const { data: chats } = useQuery({
    queryKey: ['chats'],
    queryFn: () => chatsApi.list(),
  });

  const createChatMutation = useMutation({
    mutationFn: chatsApi.create,
    onSuccess: (newChat) => {
      queryClient.invalidateQueries({ queryKey: ['chats'] });
      setSelectedChatId(newChat.id);
    },
  });

  const handleCreateChat = () => {
    createChatMutation.mutate({ title: 'New Chat' });
  };

  if (!isSidebarOpen) return null;

  return (
    <div className="w-64 border-r bg-slate-50 flex flex-col h-full">
      <div className="p-4 border-b">
        <Button onClick={handleCreateChat} className="w-full justify-start" variant="outline">
          <Plus className="mr-2 h-4 w-4" />
          New Chat
        </Button>
      </div>

      <ScrollArea className="flex-1">
        <div className="p-2 space-y-2">
          {chats?.map((chat) => (
            <Button
              key={chat.id}
              variant={selectedChatId === chat.id ? 'secondary' : 'ghost'}
              className={cn("w-full justify-start font-normal", selectedChatId === chat.id && "bg-slate-200")}
              onClick={() => setSelectedChatId(chat.id)}
            >
              <MessageSquare className="mr-2 h-4 w-4" />
              <span className="truncate">{chat.title}</span>
            </Button>
          ))}
        </div>
      </ScrollArea>

      <div className="p-4 border-t mt-auto">
        <Button variant="ghost" className="w-full justify-start" onClick={toggleSettings}>
          <Settings className="mr-2 h-4 w-4" />
          Settings
        </Button>
      </div>
    </div>
  );
}
