import { create } from 'zustand';
import { Settings } from '@/lib/types';

interface UIState {
  selectedChatId: string | null;
  setSelectedChatId: (id: string | null) => void;

  isSettingsOpen: boolean;
  toggleSettings: () => void;

  isLogsOpen: boolean;
  toggleLogs: () => void;
  
  isSidebarOpen: boolean;
  toggleSidebar: () => void;

  settings: Settings;
  updateSettings: (settings: Partial<Settings>) => void;

  draftInput: string;
  setDraftInput: (input: string) => void;
}

export const useUIStore = create<UIState>((set) => ({
  selectedChatId: null,
  setSelectedChatId: (id) => set({ selectedChatId: id }),

  isSettingsOpen: false,
  toggleSettings: () => set((state) => ({ isSettingsOpen: !state.isSettingsOpen })),

  isLogsOpen: false,
  toggleLogs: () => set((state) => ({ isLogsOpen: !state.isLogsOpen })),

  isSidebarOpen: true,
  toggleSidebar: () => set((state) => ({ isSidebarOpen: !state.isSidebarOpen })),

  settings: {
    mode: 'standard',
    autoRun: true,
    speed: 'quality',
  },
  updateSettings: (newSettings) => 
    set((state) => ({ settings: { ...state.settings, ...newSettings } })),

  draftInput: '',
  setDraftInput: (input) => set({ draftInput: input }),
}));
