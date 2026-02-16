import { KeyboardEvent } from 'react';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { Send } from 'lucide-react';
import { useUIStore } from '@/store/ui-store';

interface ChatInputProps {
  onSend: (content: string) => void;
  disabled?: boolean;
}

export function ChatInput({ onSend, disabled }: ChatInputProps) {
  const { draftInput, setDraftInput } = useUIStore();
  
  const handleSend = () => {
    if (draftInput.trim() && !disabled) {
      onSend(draftInput);
      setDraftInput('');
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="border-t border-slate-200/80 bg-white/85 px-4 py-4 backdrop-blur">
      <div className="max-w-4xl mx-auto flex gap-2 items-end">
        <Textarea
          value={draftInput}
          onChange={(e) => setDraftInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Ask Starbot anything..."
          className="min-h-[56px] max-h-[220px] rounded-2xl border-slate-300 bg-white shadow-sm focus-visible:ring-slate-400"
          disabled={disabled}
          aria-label="Message input"
        />
        <Button
          onClick={handleSend}
          disabled={disabled || !draftInput.trim()}
          size="icon"
          className="h-11 w-11 rounded-2xl bg-slate-900 text-slate-50 hover:bg-slate-800"
          aria-label="Send message"
        >
          <Send className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
