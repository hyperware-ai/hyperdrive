import React, { useEffect, useRef, useState } from 'react';
import type { RunningApp } from '../../../types/app.types';

interface AppContainerProps {
  app: RunningApp;
  isVisible: boolean;
}

export const AppContainer: React.FC<AppContainerProps> = ({ app, isVisible }) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const [hasError, setHasError] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  // Ensure we have a valid path
  const appUrl = app.path || `/app:${app.process}:${app.publisher}.os/`;

  const handleError = () => {
    setHasError(true);
    setIsLoading(false);
    console.error(`Failed to load app: ${app.label}`);
  };

  const handleLoad = () => {
    setIsLoading(false);
  };

  return (
    <div
      className={`app-container fixed inset-0 dark:bg-black bg-white z-30 transition-transform duration-300
        ${isVisible ? 'translate-x-0' : 'translate-x-full'}`}
    >
      {hasError ? (
        <div className="w-full h-full flex flex-col items-center justify-center bg-gradient-to-b from-gray-100 to-gray-200 dark:from-gray-800 dark:to-gray-900">
          <div className="text-center">
            <div className="text-6xl mb-4">⚠️</div>
            <h2 className="text-xl font-semibold mb-2 text-gray-800 dark:text-gray-200">Failed to load {app.label}</h2>
            <p className="text-gray-600 dark:text-gray-400">The app could not be loaded.</p>
          </div>
        </div>
      ) : (
        <>
          {isLoading && (
            <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-b from-gray-100 to-gray-200 dark:from-gray-800 dark:to-gray-900 z-10">
              <div className="flex flex-col items-center justify-center gap-2">
                <div className="w-12 h-12 border-4 border-gray-300 dark:border-gray-600 border-t-blue-500 rounded-full animate-spin mb-4"></div>
                <p className="text-gray-600 dark:text-gray-400">Loading {app.label}...</p>
              </div>
            </div>
          )}
          <iframe
            ref={iframeRef}
            src={appUrl}
            className="w-full h-full border-0"
            title={app.label}
            onError={handleError}
            onLoad={handleLoad}
            // Allow all necessary permissions for subdomain redirects
            allow="accelerometer; camera; encrypted-media; geolocation; gyroscope; microphone; midi; payment; usb; xr-spatial-tracking"
            // Enhanced sandbox permissions
            sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-top-navigation allow-modals allow-downloads allow-presentation allow-storage-access-by-user-activation"
          />
        </>
      )}
    </div>
  );
};
