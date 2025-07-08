import React, { useState, useMemo } from 'react';
import type { HomepageApp } from '../../../types/app.types';
import { useAppStore } from '../../../stores/appStore';
import { useNavigationStore } from '../../../stores/navigationStore';
import { usePersistenceStore } from '../../../stores/persistenceStore';
import { AppIcon } from './AppIcon';
import { BsSearch, BsX } from 'react-icons/bs';

export const AppDrawer: React.FC = () => {
  const { apps } = useAppStore();
  const { isAppDrawerOpen, toggleAppDrawer, openApp } = useNavigationStore();
  const { homeScreenApps, addToHomeScreen } = usePersistenceStore();
  const [searchQuery, setSearchQuery] = useState('');

  const filteredApps = useMemo(() => {
    return apps
      .filter(app => app.label.toLowerCase().includes(searchQuery.toLowerCase()))
      .sort((a, b) => a.label.localeCompare(b.label));
  }, [apps, searchQuery]);

  const handleAddToHome = (app: HomepageApp) => {
    // Ensure we don't duplicate
    if (!homeScreenApps.includes(app.id)) {
      addToHomeScreen(app.id);
    }
    toggleAppDrawer();
  };

  if (!isAppDrawerOpen) return null;

  return (
    <div className="app-drawer fixed inset-0 bg-gradient-to-b from-gray-100/98 to-white/98 dark:from-gray-900/98 dark:to-black/98 backdrop-blur-xl z-50 flex flex-col">
      <div className="p-4">
        <div className="flex items-center gap-2">
          <BsSearch className="opacity-50" />
          <input
            type="text"
            placeholder="Search apps..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="grow self-stretch px-4 py-3 pl-12 bg-black/10 dark:bg-white/10 backdrop-blur rounded-2xl text-black dark:text-white placeholder-black/50 dark:placeholder-white/50 border border-black/20 dark:border-white/20 focus:border-blue-400 focus:outline-none transition-all"
            autoFocus
          />
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        <div className="grid grid-cols-4 md:grid-cols-6 gap-6">
          {filteredApps.map(app => (
            <div key={app.id} className="relative group">
              <div onClick={() => openApp(app)}>
                <AppIcon app={app} isEditMode={false} />
              </div>
              {!homeScreenApps.includes(app.id) && (
                <button
                  onClick={() => handleAddToHome(app)}
                  className="absolute -top-3 -right-3 w-8 h-8 rounded-full thin"
                >
                  +
                </button>
              )}
            </div>
          ))}
        </div>
      </div>

      <button
        onClick={toggleAppDrawer}
        className="m-4 p-4 text-center rounded-xl md:ml-auto"
      >
        <BsX className="text-lg" />
        <span>Close</span>
      </button>
    </div>
  );
};