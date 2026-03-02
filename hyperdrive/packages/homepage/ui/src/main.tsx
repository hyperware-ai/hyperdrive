// Entry point for the React application
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.tsx'
import './index.css'
import './homepage/homepage.css'
import '@rainbow-me/rainbowkit/styles.css'
// Auto-start hw:// protocol link handling
import '@hyperware-ai/hw-protocol-watcher'
import { useNotificationStore } from './homepage/stores/notificationStore'
import { initializePushNotifications } from './homepage/utils/pushNotifications'
import { getChatBasePath } from './utils/chatBase'
import { getDefaultConfig, RainbowKitProvider } from '@rainbow-me/rainbowkit'
import { WagmiProvider, http } from 'wagmi'
import { base } from 'wagmi/chains'
import { QueryClientProvider, QueryClient } from '@tanstack/react-query'

const walletConfig = getDefaultConfig({
  appName: 'Hyperware Homepage',
  projectId: '1307513d16d359871023f1f78ac03361',
  chains: [base],
  ssr: false,
  transports: {
    [base.id]: http(),
  },
})

const walletQueryClient = new QueryClient()

const patchChatFetch = () => {
  const originalFetch = window.fetch.bind(window);
  window.fetch = (input, init) => {
    if (typeof input === 'string') {
      const chatBase = getChatBasePath();
      if (input.startsWith('//api/')) {
        return originalFetch(`${chatBase}${input.slice(1)}`, init);
      }
      if (input.startsWith('/api/')) {
        return originalFetch(`${chatBase}${input}`, init);
      }
      if (input.startsWith('http://api/') || input.startsWith('https://api/')) {
        const pathStart = input.indexOf('/api/');
        if (pathStart !== -1) {
          return originalFetch(`${chatBase}${input.slice(pathStart)}`, init);
        }
      }
    }
    return originalFetch(input, init);
  };
};

patchChatFetch();

const loadOurScript = () =>
  new Promise<void>((resolve) => {
    if ((window as any).our) {
      resolve();
      return;
    }

    const script = document.createElement('script');
    script.src = '/our.js';
    script.async = true;
    script.onload = () => resolve();
    script.onerror = () => resolve();
    document.head.appendChild(script);
  });

// Listen for push notification messages from service worker
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.addEventListener('message', (event) => {
    if (event.data && event.data.type === 'PUSH_NOTIFICATION_RECEIVED') {
      const notification = event.data.notification;

      const appId = notification.data?.appId || notification.appId || 'system';
      const appLabel = notification.data?.appLabel || notification.appLabel || 'System';

      useNotificationStore.getState().addNotification({
        appId,
        appLabel,
        title: notification.title,
        body: notification.body,
        icon: notification.icon,
      });
    }
  });
}

// Register service worker for PWA
if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker
      .register('/sw.js')
      .then(async (registration) => {
        await initializePushNotifications(registration);

        if ('Notification' in window) {
          useNotificationStore.getState().setPermissionGranted(
            Notification.permission === 'granted'
          );
        }

        setInterval(() => {
          registration.update();
        }, 60 * 60 * 1000);
      })
      .catch((error) => {
        console.error('SW registration failed:', error);
      });
  });
}

const renderApp = () => {
  const nodeName = (window as any).our?.node;
  if (nodeName) {
    document.title = `${nodeName} - home`;
  }

  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <WagmiProvider config={walletConfig}>
        <QueryClientProvider client={walletQueryClient}>
          <RainbowKitProvider initialChain={base.id} modalSize="compact" showRecentTransactions={true}>
            <App />
          </RainbowKitProvider>
        </QueryClientProvider>
      </WagmiProvider>
    </React.StrictMode>,
  )
};

loadOurScript().then(renderApp);
