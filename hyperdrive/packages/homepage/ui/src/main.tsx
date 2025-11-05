import React from 'react'
import ReactDOM from 'react-dom/client'
import Home from './components/Home'
import './index.css'
import { useNotificationStore } from './stores/notificationStore'
import { initializePushNotifications } from './utils/pushNotifications'

// Listen for push notification messages from service worker
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.addEventListener('message', (event) => {
    if (event.data && event.data.type === 'PUSH_NOTIFICATION_RECEIVED') {
      const notification = event.data.notification;

      // Extract appId and appLabel from data if available
      const appId = notification.data?.appId || notification.appId || 'system';
      const appLabel = notification.data?.appLabel || notification.appLabel || 'System';

      // Add to notification store
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
    navigator.serviceWorker.register('/sw.js')
      .then(async (registration) => {

        // Initialize push notifications
        await initializePushNotifications(registration);

        // Update permission state in store
        if ('Notification' in window) {
          useNotificationStore.getState().setPermissionGranted(
            Notification.permission === 'granted'
          );
        }

        // Check for updates periodically
        setInterval(() => {
          registration.update();
        }, 60 * 60 * 1000); // Check every hour
      })
      .catch((error) => {
        console.error('SW registration failed:', error);
      });
  });
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Home />
  </React.StrictMode>,
)
