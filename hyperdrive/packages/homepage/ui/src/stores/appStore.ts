import { create } from 'zustand';
import type { HomepageApp } from '../types/app.types';

interface AppStore {
  apps: HomepageApp[];
  setApps: (apps: HomepageApp[]) => void;
  isEditMode: boolean;
  setEditMode: (mode: boolean) => void;
}

export const useAppStore = create<AppStore>((set) => ({
  apps: [],
  setApps: (apps) => set({ apps }),
  isEditMode: false,
  setEditMode: (isEditMode) => set({ isEditMode }),
}));