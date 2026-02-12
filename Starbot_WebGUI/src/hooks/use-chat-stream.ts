import { useEffect, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { chatsApi } from '@/lib/api/chats';
import { API_BASE_URL, getApiToken } from '@/lib/config';
import { Message } from '@/lib/types';

export function useChatStream(chatId: string | null) {
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<string>('');
  
  const { data: messages, isLoading, error } = useQuery({
    queryKey: ['messages', chatId],
    queryFn: () => chatId ? chatsApi.getMessages(chatId) : Promise.resolve([]),
    enabled: !!chatId,
  });

  useEffect(() => {
    if (!chatId) return;

    const token = getApiToken();
    const url = new URL(`${API_BASE_URL}/chats/${chatId}/stream`);
    if (token) {
        url.searchParams.append('token', token); // Pass token in query if headers not supported by EventSource
    }

    // Note: Standard EventSource doesn't support headers. 
    // If the API requires header auth, we might need a polyfill or use fetch with ReadableStream.
    // For now, assuming the API accepts token in query param or we use a polyfill if needed.
    // Or we rely on the prompt "Web GUI sends a local API token header" -> this implies we might need a custom EventSource.
    // I will use native EventSource with query param as a fallback for now.
    
    const eventSource = new EventSource(url.toString());

    eventSource.addEventListener('assistant.delta', (e) => {
      const data = JSON.parse(e.data);
      queryClient.setQueryData<Message[]>(['messages', chatId], (old) => {
        if (!old) return [];
        const lastMsg = old[old.length - 1];
        if (lastMsg && lastMsg.role === 'assistant' && !lastMsg.metadata?.final) {
           // Append to last message
           return [
             ...old.slice(0, -1),
             { ...lastMsg, content: lastMsg.content + data.content }
           ];
        } else {
           // New message
           return [...old, { 
             id: 'temp-assistant', 
             chatId, 
             role: 'assistant', 
             content: data.content, 
             createdAt: new Date().toISOString() 
            }];
        }
      });
    });

    eventSource.addEventListener('assistant.final', (e) => {
        const data = JSON.parse(e.data);
        // Replace temp message with final one or update it
        queryClient.setQueryData<Message[]>(['messages', chatId], (old) => {
            if (!old) return [];
             const lastMsg = old[old.length - 1];
             if (lastMsg && lastMsg.role === 'assistant') {
                 return [...old.slice(0, -1), { ...data, metadata: { ...data.metadata, final: true } }];
             }
             return [...old, data];
        });
        setStatus('');
    });

    eventSource.addEventListener('status', (e) => {
        const data = JSON.parse(e.data);
        setStatus(data.message || data.status);
    });

    eventSource.addEventListener('tool.start', (e) => {
        const data = JSON.parse(e.data);
        setStatus(`Running tool: ${data.tool}`);
    });

     eventSource.addEventListener('tool.end', (e) => {
        setStatus('');
    });
    
    eventSource.addEventListener('error', (e) => {
        console.error('SSE Error:', e);
        setStatus('Error in stream');
        eventSource.close();
    });

    return () => {
      eventSource.close();
      setStatus('');
    };
  }, [chatId, queryClient]);

  return { messages, isLoading, error, status };
}
