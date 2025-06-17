import React, { useState, useMemo } from 'react';
import type { HomepageApp } from '../../../types/app.types';
import { useAppStore } from '../../../stores/appStore';
import { useNavigationStore } from '../../../stores/navigationStore';
import { usePersistenceStore } from '../../../stores/persistenceStore';
import { AppIcon } from './AppIcon';

export const AppDrawer: React.FC = () => {
  const { apps } = useAppStore();
  const { isAppDrawerOpen, toggleAppDrawer } = useNavigationStore();
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
    <div className="fixed inset-0 bg-gradient-to-b from-gray-900/98 to-black/98 backdrop-blur-xl z-50 flex flex-col">
      <div className="p-4">
        <div className="relative">
          <input
            type="text"
            placeholder="Search apps..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full px-4 py-3 pl-12 bg-white/10 backdrop-blur rounded-2xl text-white placeholder-white/50 border border-white/20 focus:border-blue-400 focus:outline-none transition-all"
            autoFocus
          />
          <div className="absolute left-4 top-1/2 transform -translate-y-1/2 text-white/50">
            🔍
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        <div className="grid grid-cols-4 md:grid-cols-6 gap-4">
          {filteredApps.map(app => (
            <div key={app.id} className="relative group">
              <AppIcon app={app} isEditMode={false} />
              {!homeScreenApps.includes(app.id) && (
                <button
                  onClick={() => handleAddToHome(app)}
                  className="absolute -top-2 -right-2 w-7 h-7 bg-green-500 text-white rounded-full flex items-center justify-center text-sm shadow-lg opacity-0 group-hover:opacity-100 transition-opacity"
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
        className="p-6 text-white/70 text-center hover:text-white transition-colors"
      >
        Close
      </button>
    </div>
  );
};