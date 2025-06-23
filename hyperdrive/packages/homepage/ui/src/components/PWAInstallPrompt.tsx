import React, { useEffect, useState } from 'react';

interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>;
}

const PWAInstallPrompt: React.FC = () => {
  const [installPrompt, setInstallPrompt] = useState<BeforeInstallPromptEvent | null>(null);
  const [showPrompt, setShowPrompt] = useState(false);

  useEffect(() => {
    const handleBeforeInstallPrompt = (e: Event) => {
      // Prevent the default browser install prompt
      e.preventDefault();
      // Store the event for later use
      setInstallPrompt(e as BeforeInstallPromptEvent);

      // Show our custom prompt after a delay
      setTimeout(() => {
        setShowPrompt(true);
      }, 2000);
    };

    window.addEventListener('beforeinstallprompt', handleBeforeInstallPrompt);

    // Check if app is already installed
    if (window.matchMedia('(display-mode: standalone)').matches) {
      console.log('App is already installed');
    }

    return () => {
      window.removeEventListener('beforeinstallprompt', handleBeforeInstallPrompt);
    };
  }, []);

  const handleInstall = async () => {
    if (!installPrompt) return;

    // Show the browser's install prompt
    await installPrompt.prompt();

    // Wait for the user's response
    const { outcome } = await installPrompt.userChoice;
    console.log(`User ${outcome} the install prompt`);

    // Clear the prompt
    setInstallPrompt(null);
    setShowPrompt(false);
  };

  const handleDismiss = () => {
    setShowPrompt(false);
    // Show again after 7 days
    const nextPromptTime = Date.now() + (7 * 24 * 60 * 60 * 1000);
    localStorage.setItem('pwa-prompt-dismissed', nextPromptTime.toString());
  };

  // Check if prompt was previously dismissed
  useEffect(() => {
    const dismissedUntil = localStorage.getItem('pwa-prompt-dismissed');
    if (dismissedUntil && Date.now() < parseInt(dismissedUntil)) {
      setShowPrompt(false);
    }
  }, []);

  if (!showPrompt || !installPrompt) return null;

  return (
    <div className="fixed top-4 left-4 right-4 bg-black/90 text-green-400 p-4 rounded-lg shadow-lg z-50 max-w-md mx-auto animate-slide-down">
      <div className="flex items-start gap-3">
        <div className="flex-1">
          <h3 className="font-semibold mb-1">Install Hyperdrive</h3>
          <p className="text-sm text-gray-300">
            Add to your home screen for the best experience
          </p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={handleDismiss}
            className="px-3 py-1 text-xs bg-gray-700 hover:bg-gray-600 rounded transition-colors"
          >
            Not now
          </button>
          <button
            onClick={handleInstall}
            className="px-3 py-1 text-xs bg-green-600 hover:bg-green-500 rounded transition-colors"
          >
            Install
          </button>
        </div>
      </div>
    </div>
  );
};

export default PWAInstallPrompt;
