import React, { useState, useMemo } from 'react';
import type { HomepageApp } from '../../../types/app.types';
import { useAppStore } from '../../../stores/appStore';
import { useNavigationStore } from '../../../stores/navigationStore';
import { usePersistenceStore } from '../../../stores/persistenceStore';
import { AppIcon } from './AppIcon';
import { BsSearch, BsX } from 'react-icons/bs';
import classNames from 'classnames';

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

  const isMobile = window.innerWidth < 768;

  return (
    <div
      className="app-drawer fixed inset-0 bg-gradient-to-b from-gray-100/20 to-white/20 dark:from-gray-900/20 dark:to-black/20 backdrop-blur-xl z-50 flex flex-col animate-modal-backdrop"
      onClick={toggleAppDrawer}
    >
      <div className="px-2 py-1 self-stretch flex items-center gap-2">
        <h2 className="prose">My Apps</h2>
        <div className="bg-black/10 dark:bg-white/10 flex items-center gap-2 ml-auto max-w-sm grow self-stretch rounded-lg pl-2">
          <BsSearch className="opacity-50" />
          <input
            type="text"
            placeholder="Search apps..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="grow self-stretch !bg-transparent !p-0"
            autoFocus={!isMobile}
          />
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        <div className={classNames(`
          grid
          gap-4 md:gap-6 lg:gap-8
          `, {
          'grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6': filteredApps.length > 0,
          'grid-cols-2': filteredApps.length === 0,
        })}>
          {filteredApps.map((app, index) => (
            <div
              key={app.id}
              className="relative group animate-grid-enter"
              style={{ '--item-index': index } as React.CSSProperties}
              data-app-id={app.id}
            >
              <div onClick={(e) => {
                e.stopPropagation();
                if (app.path === null) {
                  return;
                }
                openApp(app);
              }}>
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
          {filteredApps.length === 0 && (
            <div
              className={classNames('bg-neon text-black rounded-lg px-2 py-1 text-xs flex flex-wrap items-center justify-center col-span-full')}
            >
              <span>No installed apps found.</span>
              <span
                // href={`/main:app-store:sys/?search=${searchQuery}`}
                className="underline text-iris font-bold cursor-pointer"
                onClick={(e) => {
                  e.stopPropagation();
                  setSearchQuery('')
                  openApp(apps.find(a => a.id === 'main:app-store:sys')!, `?search=${searchQuery}`)
                }}
              >
                Search the app store
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};