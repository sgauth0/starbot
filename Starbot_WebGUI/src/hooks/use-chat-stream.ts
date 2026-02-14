import { useEffect, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { chatsApi } from '@/lib/api/chats';
import { API_BASE_URL, getApiToken } from '@/lib/config';
import { Message } from '@/lib/types';

export function useChatStream(chatId: string | null) {
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<string>('');
  const abortControllerRef = useRef<AbortController | null>(null);

  const { data: messages, isLoading, error } = useQuery({
    queryKey: ['messages', chatId],
    queryFn: () => chatId ? chatsApi.getMessages(chatId) : Promise.resolve([]),
    enabled: !!chatId,
  });

  const startStream = async (mode: 'quick' | 'standard' | 'deep' = 'standard') => {
    if (!chatId) return;

    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }

    const controller = new AbortController();
    abortControllerRef.current = controller;

    try {
      const token = getApiToken();
      const response = await fetch(`${API_BASE_URL}/chats/${chatId}/run`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Accept': 'text/event-stream',
          ...(token ? { 'X-API-Token': token } : {}),
        },
        body: JSON.stringify({ mode }),
        signal: controller.signal,
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const reader = response.body?.getReader();
      if (!reader) throw new Error('No response body');

      const decoder = new TextDecoder();
      let buffer = '';
      let currentEvent = 'message';

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';

        for (const line of lines) {
          if (!line.trim()) {
            currentEvent = 'message'; // Reset on blank line
            continue;
          }

          if (line.startsWith('event:')) {
            currentEvent = line.slice(6).trim();
          }

          if (line.startsWith('data:')) {
            const data = JSON.parse(line.slice(5).trim());
            handleSSEEvent(currentEvent, data);
          }
        }
      }
    } catch (err) {
      if (err instanceof Error && err.name !== 'AbortError') {
        console.error('Stream error:', err);
        setStatus(`Error: ${err.message}`);
      }
    }
  };

  const handleSSEEvent = (eventType: string, data: any) => {
    switch (eventType) {
      case 'status':
        setStatus(data.message || '');
        break;

      case 'token.delta':
        queryClient.setQueryData<Message[]>(['messages', chatId], (old) => {
          if (!old) return [];
          const lastMsg = old[old.length - 1];

          if (lastMsg?.role === 'assistant' && !lastMsg.metadata?.final) {
            return [
              ...old.slice(0, -1),
              { ...lastMsg, content: lastMsg.content + (data.text || data.delta) }
            ];
          } else {
            return [...old, {
              id: data.message_id || 'temp-assistant',
              chatId: chatId!,
              role: 'assistant',
              content: data.text || data.delta || '',
              createdAt: new Date().toISOString(),
            }];
          }
        });
        break;

      case 'message.final':
        queryClient.setQueryData<Message[]>(['messages', chatId], (old) => {
          if (!old) return [];
          const lastMsg = old[old.length - 1];

          if (lastMsg?.role === 'assistant') {
            return [...old.slice(0, -1), {
              id: data.message_id || data.id,
              chatId: chatId!,
              role: 'assistant',
              content: data.content,
              createdAt: new Date().toISOString(),
              metadata: { final: true, ...data.usage },
            }];
          }
          return old;
        });
        setStatus('');
        break;

      case 'chat.updated':
        queryClient.invalidateQueries({ queryKey: ['chat', chatId] });
        break;

      case 'error':
      case 'run.error':
        setStatus(`Error: ${data.message}`);
        break;
    }
  };

  useEffect(() => {
    return () => {
      if (abortControllerRef.current) {
        abortControllerRef.current.abort();
      }
    };
  }, [chatId]);

  return { messages, isLoading, error, status, startStream };
}
