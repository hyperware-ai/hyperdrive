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
    <div className="app-drawer fixed inset-0 bg-gradient-to-b from-gray-100/20 to-white/20 dark:from-gray-900/20 dark:to-black/20 backdrop-blur-xl z-50 flex flex-col">
      <div className="p-4 self-stretch flex items-center gap-2">
        <h2 className="prose">My Apps</h2>
        <div className="bg-black/10 dark:bg-white/10 flex items-center gap-2 ml-auto max-w-md grow self-stretch rounded-lg pl-2">
          <BsSearch className="opacity-50 text-lg" />
          <input
            type="text"
            placeholder="Search apps..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="grow self-stretch bg-transparent"
            autoFocus
          />
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        <div className="grid grid-cols-4 md:grid-cols-6 gap-6">
          {filteredApps.map(app => (
            <div key={app.id} className="relative group" data-app-id={app.id}>
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