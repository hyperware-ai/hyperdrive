import { useNotificationStore } from '../stores/notificationStore';

// Helper function to convert base64 to Uint8Array
function urlBase64ToUint8Array(base64String: string) {
  const padding = '='.repeat((4 - (base64String.length % 4)) % 4);
  const base64 = (base64String + padding)
    .replace(/-/g, '+')
    .replace(/_/g, '/');

  const rawData = window.atob(base64);
  const outputArray = new Uint8Array(rawData.length);

  for (let i = 0; i < rawData.length; ++i) {
    outputArray[i] = rawData.charCodeAt(i);
  }
  return outputArray;
}

async function fetchVapidPublicKey(): Promise<string | null> {
  try {
    const vapidResponse = await fetch('/api/notifications/vapid-key');

    if (!vapidResponse.ok) {
      const errorText = await vapidResponse.text();
      console.error('[Push] Failed to get VAPID public key:', errorText);
      useNotificationStore.getState().setHasVapidKey(false);
      return null;
    }

    const responseData = await vapidResponse.json();
    const publicKey = responseData?.publicKey;

    if (typeof publicKey === 'string' && publicKey.length > 0) {
      useNotificationStore.getState().setHasVapidKey(true);
      return publicKey;
    }

    console.error('[Push] No VAPID public key available in response');
    useNotificationStore.getState().setHasVapidKey(false);
    return null;
  } catch (error) {
    console.error('[Push] Error fetching VAPID public key:', error);
    useNotificationStore.getState().setHasVapidKey(false);
    return null;
  }
}

export async function initializePushNotifications(registration: ServiceWorkerRegistration) {
  try {
    if (!('PushManager' in window) || !('Notification' in window)) {
      return;
    }

    const permission = Notification.permission;
    useNotificationStore.getState().setPermissionGranted(permission === 'granted');

    const publicKey = await fetchVapidPublicKey();

    if (!publicKey) {
      return;
    }

    if (permission !== 'granted') {
      return;
    }

    let subscription = await registration.pushManager.getSubscription();

    if (subscription) {
      // Check subscription age and renew if needed
      console.log('[Push] Checking existing subscription age');

      try {
        const response = await fetch('/api/notifications/subscription-info', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({ endpoint: subscription.endpoint }),
        });

        if (response.ok) {
          const data = await response.json();

          if (!data.subscription || !data.subscription.created_at) {
            console.log('[Push] No subscription found on server, will create new one');
            await subscription.unsubscribe();
            subscription = null;
          } else {
            const now = Date.now();
            const createdAt = data.subscription.created_at;
            const ageMs = now - createdAt;
            const oneWeekMs = 7 * 24 * 60 * 60 * 1000;
            const oneMonthMs = 30 * 24 * 60 * 60 * 1000;

            console.log('[Push] Subscription age:', Math.floor(ageMs / 1000 / 60 / 60), 'hours');

            if (ageMs > oneMonthMs) {
              console.log('[Push] Subscription older than 1 month, removing');
              await subscription.unsubscribe();
              subscription = null;
            } else if (ageMs > oneWeekMs) {
              console.log('[Push] Subscription older than 1 week, renewing');
              await subscription.unsubscribe();
              subscription = null;
            }
          }
        } else {
          console.log('[Push] Could not check subscription age, will create new one');
          await subscription.unsubscribe();
          subscription = null;
        }
      } catch (error) {
        console.error('[Push] Error checking subscription:', error);
        if (subscription) {
          await subscription.unsubscribe();
          subscription = null;
        }
      }
    }

    if (!subscription) {
      try {
        const applicationServerKey = urlBase64ToUint8Array(publicKey);

        subscription = await registration.pushManager.subscribe({
          userVisibleOnly: true,
          applicationServerKey,
        });

        console.log('[Push] Created new subscription');
      } catch (subscribeError: any) {
        console.error('[Push] Subscribe error:', subscribeError);
        console.error('[Push] Error name:', subscribeError?.name);
        console.error('[Push] Error message:', subscribeError?.message);

        if (subscribeError?.name === 'AbortError') {
          console.error('[Push] Push service registration was aborted - this usually means the VAPID key is invalid or malformed');
        }

        return;
      }
    }

    if (subscription) {
      const subscriptionData = subscription.toJSON();
      const response = await fetch('/api/notifications/subscribe', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          ...subscriptionData,
          created_at: Date.now(),
        }),
      });

      if (!response.ok) {
        console.error('[Push] Failed to save subscription on server');
      } else {
        console.log('[Push] Successfully saved subscription on server');
      }
    }
  } catch (error) {
    console.error('[Push] Error initializing push notifications:', error);
  }
}
