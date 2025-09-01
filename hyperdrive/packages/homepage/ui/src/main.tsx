import React from 'react'
import ReactDOM from 'react-dom/client'
import Home from './components/Home'
import './index.css'

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
    console.log('[Init Push] Starting push notification initialization');

    // Check if push notifications are supported
    if (!('PushManager' in window)) {
      console.log('[Init Push] Push notifications not supported');
      return;
    }

    // Check current permission status
    let permission = Notification.permission;
    console.log('[Init Push] Current permission status:', permission);

    if (permission === 'denied') {
      console.log('[Init Push] Push notifications permission denied');
      return;
    }

    // Request permission if not granted
    if (permission === 'default') {
      console.log('[Init Push] Requesting notification permission...');
      permission = await Notification.requestPermission();
      console.log('[Init Push] Permission result:', permission);

      if (permission !== 'granted') {
        console.log('[Init Push] Permission not granted');
        return;
      }
    }

    // Get VAPID public key from server
    console.log('[Init Push] Fetching VAPID public key...');
    const vapidResponse = await fetch('/api/notifications/vapid-key');
    console.log('[Init Push] VAPID response status:', vapidResponse.status);

    if (!vapidResponse.ok) {
      const errorText = await vapidResponse.text();
      console.error('[Init Push] Failed to get VAPID public key:', errorText);
      return;
    }

    const responseData = await vapidResponse.json();
    console.log('[Init Push] VAPID response data:', responseData);
    const { publicKey } = responseData;

    if (!publicKey) {
      console.error('[Init Push] No VAPID public key available in response');
      return;
    }

    console.log('[Init Push] Got VAPID public key:', publicKey);

    // Check if already subscribed
    console.log('[Init Push] Checking existing subscription...');
    let subscription = await registration.pushManager.getSubscription();
    console.log('[Init Push] Existing subscription:', subscription);

    if (!subscription && permission === 'granted') {
      // Subscribe if we have permission but no subscription
      console.log('[Init Push] Permission granted, attempting to subscribe...');

      try {
          // Convert the public key
          console.log('[Init Push] Converting public key to Uint8Array...');
          console.log('[Init Push] Public key string length:', publicKey.length);
          const applicationServerKey = urlBase64ToUint8Array(publicKey);
          console.log('[Init Push] Converted key length:', applicationServerKey.length, 'bytes');
          console.log('[Init Push] First byte should be 0x04 for uncompressed:', applicationServerKey[0]);

          // Validate the key format
          if (applicationServerKey.length !== 65) {
            console.error('[Init Push] Invalid key length! Expected 65 bytes for P-256, got:', applicationServerKey.length);
          }
          if (applicationServerKey[0] !== 0x04) {
            console.error('[Init Push] Invalid key format! First byte should be 0x04 for uncompressed, got:', applicationServerKey[0]);
          }

          // Subscribe to push notifications
          subscription = await registration.pushManager.subscribe({
            userVisibleOnly: true,
            applicationServerKey: applicationServerKey
          });

          console.log('[Init Push] Successfully subscribed:', subscription);
          console.log('[Init Push] Subscription endpoint:', subscription.endpoint);
        } catch (subscribeError: any) {
          console.error('[Init Push] Subscribe error:', subscribeError);
          console.error('[Init Push] Error name:', subscribeError?.name);
          console.error('[Init Push] Error message:', subscribeError?.message);

          if (subscribeError?.name === 'AbortError') {
            console.error('[Init Push] Push service registration was aborted - this usually means the VAPID key is invalid or malformed');
            console.error('[Init Push] VAPID key that failed:', publicKey);
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

        if (response.ok) {
          console.log('Push notification subscription successful');
        } else {
          console.error('Failed to save subscription on server');
        }
      }
    } else {
      console.log('Already subscribed to push notifications');

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

// Register service worker for PWA
if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/sw.js')
      .then(async (registration) => {
        console.log('SW registered:', registration);

        // Initialize push notifications
        await initializePushNotifications(registration);

        // Check for updates periodically
        setInterval(() => {
          registration.update();
        }, 60 * 60 * 1000); // Check every hour
      })
      .catch((error) => {
        console.log('SW registration failed:', error);
      });
  });
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Home />
  </React.StrictMode>,
)
