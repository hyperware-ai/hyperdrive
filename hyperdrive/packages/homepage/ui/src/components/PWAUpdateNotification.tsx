import React, { useEffect, useState } from 'react';

const PWAUpdateNotification: React.FC = () => {
  const [showUpdate, setShowUpdate] = useState(false);
  const [registration, setRegistration] = useState<ServiceWorkerRegistration | null>(null);

  useEffect(() => {
    if ('serviceWorker' in navigator) {
      navigator.serviceWorker.ready.then((reg) => {
        setRegistration(reg);

        // Listen for updates
        reg.addEventListener('updatefound', () => {
          const newWorker = reg.installing;
          if (newWorker) {
            newWorker.addEventListener('statechange', () => {
              if (newWorker.state === 'installed' && navigator.serviceWorker.controller) {
                // New content is available
                setShowUpdate(true);
              }
            });
          }
        });
      });

      // Listen for controller change
      let refreshing = false;
      navigator.serviceWorker.addEventListener('controllerchange', () => {
        if (!refreshing) {
          refreshing = true;
          window.location.reload();
        }
      });
    }
  }, []);

  const handleUpdate = () => {
    if (registration?.waiting) {
      registration.waiting.postMessage({ type: 'SKIP_WAITING' });
    }
    setShowUpdate(false);
  };

  if (!showUpdate) return null;

  return (
    <div className="fixed bottom-4 left-4 right-4 bg-black/90 text-green-400 p-4 rounded-lg shadow-lg z-50 max-w-md mx-auto">
      <div className="flex items-center justify-between">
        <span className="text-sm">A new version is available!</span>
        <div className="flex gap-2">
          <button
            onClick={() => setShowUpdate(false)}
            className="px-3 py-1 text-xs bg-gray-700 hover:bg-gray-600 rounded transition-colors"
          >
            Later
          </button>
          <button
            onClick={handleUpdate}
            className="px-3 py-1 text-xs bg-green-600 hover:bg-green-500 rounded transition-colors"
          >
            Update
          </button>
        </div>
      </div>
    </div>
  );
};

export default PWAUpdateNotification;
