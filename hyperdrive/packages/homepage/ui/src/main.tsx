import React from 'react'
import ReactDOM from 'react-dom/client'
import Home from './components/Home'
import './index.css'
import { useNotificationStore } from './stores/notificationStore'

// Helper function to convert base64 to Uint8Array
function urlBase64ToUint8Array(base64String: string) {
  const padding = '='.repeat((4 - base64String.length % 4) % 4);
  const base64 = (base64String + padding)
    .replace(/\-/g, '+')
    .replace(/_/g, '/');

  const rawData = window.atob(base64);
  const outputArray = new Uint8Array(rawData.length);

  for (let i = 0; i < rawData.length; ++i) {
    outputArray[i] = rawData.charCodeAt(i);
  }
  return outputArray;
}

// Initialize push notifications
async function initializePushNotifications(registration: ServiceWorkerRegistration) {
  try {
    // Check if push notifications are supported
    if (!('PushManager' in window)) {
      return;
    }

    // Check current permission status
    let permission = Notification.permission;

    if (permission === 'denied') {
      return;
    }

    // Request permission if not granted
    if (permission === 'default') {
      permission = await Notification.requestPermission();

      if (permission !== 'granted') {
        return;
      }
    }

    // Get VAPID public key from server
    const vapidResponse = await fetch('/api/notifications/vapid-key');

    if (!vapidResponse.ok) {
      const errorText = await vapidResponse.text();
      console.error('[Init Push] Failed to get VAPID public key:', errorText);
      return;
    }

    const responseData = await vapidResponse.json();
    const { publicKey } = responseData;

    if (!publicKey) {
      console.error('[Init Push] No VAPID public key available in response');
      return;
    }

    // Check if already subscribed
    let subscription = await registration.pushManager.getSubscription();

    if (!subscription && permission === 'granted') {
      // Subscribe if we have permission but no subscription
      try {
          // Convert the public key
          const applicationServerKey = urlBase64ToUint8Array(publicKey);


          // Subscribe to push notifications
          subscription = await registration.pushManager.subscribe({
            userVisibleOnly: true,
            applicationServerKey: applicationServerKey
          });

        } catch (subscribeError: any) {
          console.error('[Init Push] Subscribe error:', subscribeError);
          console.error('[Init Push] Error name:', subscribeError?.name);
          console.error('[Init Push] Error message:', subscribeError?.message);

          if (subscribeError?.name === 'AbortError') {
            console.error('[Init Push] Push service registration was aborted - this usually means the VAPID key is invalid or malformed');
          }

          // Return early if subscription failed
          return;
        }

      // Send subscription to server (only if we have a subscription)
      if (subscription) {
        const response = await fetch('/api/notifications/subscribe', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify(subscription.toJSON())
        });

        if (!response.ok) {
          console.error('Failed to save subscription on server');
        }
      }
    } else {

      // Optionally update subscription on server to ensure it's current
      if (subscription) {
        await fetch('/api/notifications/subscribe', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify(subscription.toJSON())
        });
      }
    }
  } catch (error) {
    console.error('Error initializing push notifications:', error);
  }
}

// Listen for push notification messages from service worker
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.addEventListener('message', (event) => {
    if (event.data && event.data.type === 'PUSH_NOTIFICATION_RECEIVED') {
      const notification = event.data.notification;

      // Add to notification store
      useNotificationStore.getState().addNotification({
        appId: notification.appId || 'system',
        appLabel: notification.appLabel || 'System',
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
