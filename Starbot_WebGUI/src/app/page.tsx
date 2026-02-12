'use client';

import { Sidebar } from '@/components/sidebar';
import { ChatView } from '@/components/chat/chat-view';
import { SettingsPanel } from '@/components/settings-panel';
import { LogsPanel } from '@/components/logs-panel';
import { useUIStore } from '@/store/ui-store';
import { Button } from '@/components/ui/button';
import { Menu, Terminal } from 'lucide-react';
import { cn } from '@/lib/utils';

export default function Page() {
  const { isSidebarOpen, toggleSidebar, toggleLogs } = useUIStore();

  return (
    <div className="flex h-screen w-full bg-white overflow-hidden">
      {/* Sidebar */}
      <div className={cn(
          "transition-all duration-300 ease-in-out",
          isSidebarOpen ? "w-64" : "w-0 overflow-hidden"
      )}>
        <Sidebar />
      </div>

      {/* Main Content */}
      <div className="flex-1 flex flex-col relative min-w-0">
        <header className="h-14 border-b flex items-center justify-between px-4">
            <div className="flex items-center gap-4">
                <Button variant="ghost" size="icon" onClick={toggleSidebar}>
                    <Menu className="h-4 w-4" />
                </Button>
                <h1 className="font-semibold text-lg">Starbot</h1>
            </div>
            <Button variant="ghost" size="icon" onClick={toggleLogs} title="Diagnostics">
                <Terminal className="h-4 w-4" />
            </Button>
        </header>

        <main className="flex-1 overflow-hidden relative">
            <ChatView />
            <SettingsPanel />
            <LogsPanel />
        </main>
      </div>
    </div>
  );
}
