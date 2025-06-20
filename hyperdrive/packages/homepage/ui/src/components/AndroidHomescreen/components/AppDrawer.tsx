import React, { useState, useMemo } from 'react';
import type { HomepageApp } from '../../../types/app.types';
import { useAppStore } from '../../../stores/appStore';
import { useNavigationStore } from '../../../stores/navigationStore';
import { usePersistenceStore } from '../../../stores/persistenceStore';
import { AppIcon } from './AppIcon';

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
        <div className="grid grid-cols-4 md:grid-cols-6 gap-6">
          {filteredApps.map(app => (
            <div key={app.id} className="relative group">
              <div onClick={() => openApp(app)}>
                <AppIcon app={app} isEditMode={false} />
              </div>
              {!homeScreenApps.includes(app.id) && (
                <button
                  onClick={() => handleAddToHome(app)}
                  className="absolute -top-3 -right-3 w-8 h-8 bg-green-500 text-white rounded-full flex items-center justify-center text-lg font-bold shadow-lg hover:bg-green-600 transition-colors"
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
        className="m-4 p-4 bg-[#353534] text-white text-center rounded-xl hover:bg-[#454544] transition-colors"
      >
        Close
      </button>
    </div>
  );
};