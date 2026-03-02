import React, { useState, useMemo } from 'react';
import type { HomepageApp } from '../types/app.types';
import { useAppStore } from '../stores/appStore';
import { useNavigationStore } from '../stores/navigationStore';
import { usePersistenceStore } from '../stores/persistenceStore';
import { AppIcon } from './AppIcon';
import ChatSearch from '../../components/Chats/ChatSearch';
import classNames from 'classnames';

interface AppDrawerProps {
  forceOpen?: boolean;
  disableBackdropClose?: boolean;
  zIndexClass?: string;
}

export const AppDrawer: React.FC<AppDrawerProps> = ({
  forceOpen = false,
  disableBackdropClose = false,
  zIndexClass = 'z-50',
}) => {
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
    if (!forceOpen) {
      toggleAppDrawer();
    }
  };

  const isVisible = forceOpen || isAppDrawerOpen;
  if (!isVisible) return null;

  const handleBackdropClick = () => {
    if (forceOpen || disableBackdropClose) return;
    toggleAppDrawer();
  };

  const dockedClass = forceOpen ? 'app-drawer--docked' : '';

  return (
    <div
      className={`app-drawer fixed inset-0 bg-gradient-to-b from-gray-100/20 to-white/20 dark:from-gray-900/20 dark:to-black/20 backdrop-blur-xl ${zIndexClass} ${dockedClass} flex flex-col animate-modal-backdrop`}
      onClick={handleBackdropClick}
    >
      <div
        className="flex-1 overflow-y-auto overflow-x-hidden px-3 sm:px-4"
        style={{ paddingTop: 'calc(var(--safe-area-top, 0px) + 1rem)' }}
      >
        <div className="mx-auto w-full max-w-md md:max-w-none">
          <div className={classNames(`
            grid
            gap-3 sm:gap-4 md:gap-6 lg:gap-8
            justify-items-center
            `, {
            'grid-cols-3 md:[grid-template-columns:repeat(auto-fit,minmax(7rem,1fr))]': filteredApps.length > 0,
            'grid-cols-2': filteredApps.length === 0,
          })}>
          {filteredApps.map((app, index) => (
            <div
              key={app.id}
              className="relative group animate-grid-enter min-w-0 mx-auto w-full max-w-[7rem] md:max-w-[8rem]"
              style={{ '--item-index': index } as React.CSSProperties}
              data-app-id={app.id}
            >
              <div className="w-full" onClick={(e) => {
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
                  className="absolute top-0 right-0 w-5 h-5 sm:w-6 sm:h-6 rounded-full thin text-xs p-0 leading-none flex items-center justify-center"
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

      <div className="px-3 sm:px-4 py-4 flex-shrink-0">
        <div className="mx-auto w-full max-w-md md:max-w-none">
          <ChatSearch
            value={searchQuery}
            onChange={setSearchQuery}
            placeholder="Search apps..."
          />
        </div>
      </div>
    </div>
  );
};
