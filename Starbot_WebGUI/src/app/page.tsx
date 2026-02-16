'use client';

import { Sidebar } from '@/components/sidebar';
import { ChatView } from '@/components/chat/chat-view';
import { SettingsPanel } from '@/components/settings-panel';
import { LogsPanel } from '@/components/logs-panel';
import { AccountMenu } from '@/components/account-menu';
import { useUIStore } from '@/store/ui-store';
import { Button } from '@/components/ui/button';
import { Menu } from 'lucide-react';
import { cn } from '@/lib/utils';

export default function Page() {
  const { isSidebarOpen, toggleSidebar } = useUIStore();

  return (
    <div className="relative h-screen w-full overflow-hidden bg-background">
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(56,189,248,0.16),transparent_45%),radial-gradient(circle_at_bottom_right,rgba(14,165,233,0.12),transparent_45%)]" />

      <div className="relative h-full w-full p-3 md:p-5">
        <div className="flex h-full w-full gap-3 md:gap-4">
          {/* Sidebar overlay on mobile */}
          {isSidebarOpen && (
            <div
              className="fixed inset-0 z-40 bg-slate-950/40 md:hidden"
              onClick={toggleSidebar}
              aria-hidden="true"
            />
          )}

          {/* Sidebar */}
          <div className={cn(
              "transition-all duration-300 ease-in-out z-50",
              "md:relative md:z-0",
              isSidebarOpen
                ? "fixed md:relative w-[18.5rem] md:w-[18.5rem]"
                : "w-0 md:w-0 overflow-hidden"
          )}>
            <Sidebar />
          </div>

          {/* Main Content */}
          <div className="flex-1 flex flex-col relative min-w-0 rounded-3xl border border-slate-200/80 bg-white/80 shadow-[0_20px_45px_rgba(15,23,42,0.08)] backdrop-blur overflow-hidden">
            <header className="h-14 border-b border-slate-200/80 flex items-center justify-between px-4 bg-white/70">
                <div className="flex items-center gap-4">
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={toggleSidebar}
                      aria-label="Toggle sidebar"
                      className="rounded-xl text-slate-700 hover:bg-slate-100"
                    >
                        <Menu className="h-4 w-4" />
                    </Button>
                    <div>
                      <p className="text-[11px] uppercase tracking-[0.16em] text-slate-500">Workspace</p>
                      <h1 className="font-semibold text-slate-900 leading-tight">Starbot</h1>
                    </div>
                </div>
                <AccountMenu />
            </header>

            <main className="flex-1 overflow-hidden relative">
                <ChatView />
                <SettingsPanel />
                <LogsPanel />
            </main>
          </div>
        </div>
      </div>
    </div>
  );
}
