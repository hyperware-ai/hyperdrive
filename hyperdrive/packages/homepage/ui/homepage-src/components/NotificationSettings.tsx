import React, { useEffect, useState } from 'react';

interface NotificationSettingsProps {
  onClose?: () => void;
}

export const NotificationSettings: React.FC<NotificationSettingsProps> = ({ onClose }) => {
  const [permission, setPermission] = useState<NotificationPermission>('default');
  const [isSubscribed, setIsSubscribed] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    checkNotificationStatus();
  }, []);

  const checkNotificationStatus = async () => {
    console.log('Checking notification status...');

    if ('Notification' in window) {
      console.log('Notification API available, permission:', Notification.permission);
      setPermission(Notification.permission);
    } else {
      console.error('Notification API not available in this browser');
    }

    if ('serviceWorker' in navigator && 'PushManager' in window) {
      console.log('Service Worker and Push API available');
      try {
        // Check if service worker is registered
        const registrations = await navigator.serviceWorker.getRegistrations();
        console.log('Service worker registrations:', registrations.length);

        if (registrations.length === 0) {
          console.warn('No service worker registered');
          return;
        }

        const registration = await navigator.serviceWorker.ready;
        console.log('Service worker is ready');

        // Check if push is supported
        if (!registration.pushManager) {
          console.error('Push manager not available on registration');
          return;
        }

        const subscription = await registration.pushManager.getSubscription();
        console.log('Current push subscription:', subscription);

        if (subscription) {
          // Check subscription age and renew if needed
          await checkAndRenewSubscription(subscription);
        } else if (Notification.permission === 'granted') {
          // No subscription exists but we have permission, create one
          console.log('No subscription found but permission granted, creating new subscription');
          await createNewSubscription();
        }

        // Re-check subscription status after potential creation/renewal
        const updatedSubscription = await registration.pushManager.getSubscription();
        setIsSubscribed(!!updatedSubscription);
      } catch (error) {
        console.error('Error checking subscription status:', error);
      }
    } else {
      console.error('Service Worker or Push API not available');
    }
  };

  const handleEnableNotifications = async () => {
    setIsLoading(true);
    try {
      console.log('Starting notification enable process...');

      // Request permission
      const result = await Notification.requestPermission();
      console.log('Permission result:', result);
      setPermission(result);

      if (result === 'granted') {
        // Get VAPID key and subscribe
        console.log('Fetching VAPID key from /api/notifications/vapid-key...');
        const vapidResponse = await fetch('/api/notifications/vapid-key');
        console.log('VAPID response status:', vapidResponse.status);

        if (!vapidResponse.ok) {
          const errorText = await vapidResponse.text();
          console.error('Failed to get VAPID key:', errorText);
          throw new Error(`Failed to get VAPID key: ${errorText}`);
        }

        const responseData = await vapidResponse.json();
        console.log('VAPID response data:', responseData);
        const { publicKey } = responseData;

        if (!publicKey) {
          console.error('No public key in response:', responseData);
          throw new Error('No public key received from server');
        }

        console.log('Got VAPID public key:', publicKey);
        console.log('Converting public key to Uint8Array...');
        const applicationServerKey = urlBase64ToUint8Array(publicKey);
        console.log('Converted key length:', applicationServerKey.length, 'bytes');

        console.log('Getting service worker registration...');
        const registration = await navigator.serviceWorker.ready;
        console.log('Service worker ready, attempting to subscribe...');

        try {
          const subscription = await registration.pushManager.subscribe({
            userVisibleOnly: true,
            applicationServerKey: applicationServerKey
          });
          console.log('Successfully subscribed:', subscription);
          console.log('Subscription endpoint:', subscription.endpoint);

          // Send subscription to server with timestamp
          console.log('Sending subscription to server...');
          const subscriptionData = subscription.toJSON();
          const response = await fetch('/api/notifications/subscribe', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
            },
            body: JSON.stringify({
              ...subscriptionData,
              created_at: Date.now()
            })
          });

          console.log('Server response status:', response.status);
          if (response.ok) {
            setIsSubscribed(true);
            console.log('Successfully enabled push notifications');
          } else {
            const errorText = await response.text();
            console.error('Server rejected subscription:', errorText);
            throw new Error(`Server rejected subscription: ${errorText}`);
          }
        } catch (subscribeError: any) {
          console.error('Push subscription failed:', subscribeError);
          console.error('Error name:', subscribeError?.name);
          console.error('Error message:', subscribeError?.message);

          // Check for specific error types
          if (subscribeError?.name === 'AbortError') {
            console.error('Push service registration was aborted - this usually means the VAPID key is invalid or the push service is unavailable');
          } else if (subscribeError?.name === 'NotAllowedError') {
            console.error('Push notifications are not allowed - check browser settings');
          } else if (subscribeError?.name === 'NotSupportedError') {
            console.error('Push notifications are not supported in this browser');
          }

          throw subscribeError;
        }
      }
    } catch (error: any) {
      console.error('Error enabling notifications:', error);
      console.error('Full error details:', {
        name: error?.name,
        message: error?.message,
        stack: error?.stack
      });
      alert(`Failed to enable notifications: ${error?.message || error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleDisableNotifications = async () => {
    setIsLoading(true);
    try {
      const registration = await navigator.serviceWorker.ready;
      const subscription = await registration.pushManager.getSubscription();

      if (subscription) {
        await subscription.unsubscribe();

        // Notify server
        await fetch('/api/notifications/unsubscribe', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          }
        });

        setIsSubscribed(false);
      }
    } catch (error) {
      console.error('Error disabling notifications:', error);
      alert('Failed to disable notifications. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  const handleTestNotification = () => {
    if (permission === 'granted') {
      new Notification('Test Notification', {
        body: 'This is a test notification from Hyperware!',
        icon: '/icon-180.png'
      });
    }
  };

  const checkAndRenewSubscription = async (subscription: PushSubscription) => {
    try {
      console.log('Checking subscription age for endpoint:', subscription.endpoint);

      // Get subscription info from server to check age
      const response = await fetch('/api/notifications/subscription-info', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ endpoint: subscription.endpoint })
      });

      if (!response.ok) {
        console.error('Failed to get subscription info');
        // If we can't get info, assume it needs renewal
        await renewSubscription(subscription);
        return;
      }

      const data = await response.json();

      if (!data.subscription || !data.subscription.created_at) {
        console.log('No subscription found on server, creating new one');
        await renewSubscription(subscription);
        return;
      }

      // Check age
      const now = Date.now();
      const createdAt = data.subscription.created_at;
      const ageMs = now - createdAt;
      const oneWeekMs = 7 * 24 * 60 * 60 * 1000;
      const oneMonthMs = 30 * 24 * 60 * 60 * 1000;

      console.log('Subscription age:', Math.floor(ageMs / 1000 / 60 / 60), 'hours');

      if (ageMs > oneMonthMs) {
        console.log('Subscription older than 1 month, removing');
        // Remove old subscription
        await subscription.unsubscribe();
        setIsSubscribed(false);
        // Server will auto-remove on next interaction
      } else if (ageMs > oneWeekMs) {
        console.log('Subscription older than 1 week, renewing');
        await renewSubscription(subscription);
      } else {
        console.log('Subscription is fresh, no renewal needed');
      }
    } catch (error) {
      console.error('Error checking subscription age:', error);
      // On error, try to renew to be safe
      await renewSubscription(subscription);
    }
  };

  const createNewSubscription = async () => {
    try {
      console.log('Creating new subscription...');

      // Get VAPID key
      const vapidResponse = await fetch('/api/notifications/vapid-key');
      if (!vapidResponse.ok) {
        throw new Error('Failed to get VAPID key');
      }

      const { publicKey } = await vapidResponse.json();
      const applicationServerKey = urlBase64ToUint8Array(publicKey);

      // Create new subscription
      const registration = await navigator.serviceWorker.ready;
      const newSubscription = await registration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: applicationServerKey
      });

      // Send to server with timestamp
      const subscriptionData = newSubscription.toJSON();
      const response = await fetch('/api/notifications/subscribe', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          ...subscriptionData,
          created_at: Date.now()
        })
      });

      if (response.ok) {
        console.log('Successfully created new subscription');
        setIsSubscribed(true);
        return true;
      } else {
        throw new Error('Server rejected subscription');
      }
    } catch (error) {
      console.error('Error creating new subscription:', error);
      setIsSubscribed(false);
      return false;
    }
  };

  const renewSubscription = async (oldSubscription: PushSubscription) => {
    try {
      console.log('Renewing subscription...');

      // Unsubscribe old
      await oldSubscription.unsubscribe();

      // Get VAPID key
      const vapidResponse = await fetch('/api/notifications/vapid-key');
      if (!vapidResponse.ok) {
        throw new Error('Failed to get VAPID key');
      }

      const { publicKey } = await vapidResponse.json();
      const applicationServerKey = urlBase64ToUint8Array(publicKey);

      // Create new subscription
      const registration = await navigator.serviceWorker.ready;
      const newSubscription = await registration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: applicationServerKey
      });

      // Send to server with timestamp
      const subscriptionData = newSubscription.toJSON();
      const response = await fetch('/api/notifications/subscribe', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          ...subscriptionData,
          created_at: Date.now()
        })
      });

      if (response.ok) {
        console.log('Successfully renewed subscription');
        setIsSubscribed(true);
      } else {
        throw new Error('Server rejected renewed subscription');
      }
    } catch (error) {
      console.error('Error renewing subscription:', error);
      setIsSubscribed(false);
    }
  };

  const urlBase64ToUint8Array = (base64String: string) => {
    console.log('Converting base64 string:', base64String);
    console.log('Base64 string length:', base64String.length);

    const padding = '='.repeat((4 - base64String.length % 4) % 4);
    const base64 = (base64String + padding)
      .replace(/\-/g, '+')
      .replace(/_/g, '/');

    console.log('After padding and replacements:', base64);

    const rawData = window.atob(base64);
    console.log('Raw data length after atob:', rawData.length);

    const outputArray = new Uint8Array(rawData.length);

    for (let i = 0; i < rawData.length; ++i) {
      outputArray[i] = rawData.charCodeAt(i);
    }

    console.log('Output array length:', outputArray.length);
    console.log('First byte:', outputArray[0]);

    return outputArray;
  };

  return (
    <div style={{
      padding: '20px',
      backgroundColor: 'var(--bg-color, white)',
      borderRadius: '8px',
      maxWidth: '400px',
      margin: '0 auto'
    }}>
      <h2 style={{ marginTop: 0 }}>Notification Settings</h2>

      <div style={{ marginBottom: '20px' }}>
        <p><strong>Permission Status:</strong> {permission}</p>
        <p><strong>Push Subscription:</strong> {isSubscribed ? 'Active' : 'Inactive'}</p>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
        {permission === 'default' && (
          <button
            onClick={handleEnableNotifications}
            disabled={isLoading}
            style={{
              padding: '10px 20px',
              backgroundColor: '#4CAF50',
              color: 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: isLoading ? 'not-allowed' : 'pointer',
              opacity: isLoading ? 0.5 : 1
            }}
          >
            {isLoading ? 'Enabling...' : 'Enable Notifications'}
          </button>
        )}

        {permission === 'granted' && !isSubscribed && (
          <button
            onClick={handleEnableNotifications}
            disabled={isLoading}
            style={{
              padding: '10px 20px',
              backgroundColor: '#4CAF50',
              color: 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: isLoading ? 'not-allowed' : 'pointer',
              opacity: isLoading ? 0.5 : 1
            }}
          >
            {isLoading ? 'Subscribing...' : 'Subscribe to Push Notifications'}
          </button>
        )}

        {permission === 'granted' && isSubscribed && (
          <>
            <button
              onClick={handleDisableNotifications}
              disabled={isLoading}
              style={{
                padding: '10px 20px',
                backgroundColor: '#f44336',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: isLoading ? 'not-allowed' : 'pointer',
                opacity: isLoading ? 0.5 : 1
              }}
            >
              {isLoading ? 'Disabling...' : 'Disable Push Notifications'}
            </button>

            <button
              onClick={handleTestNotification}
              style={{
                padding: '10px 20px',
                backgroundColor: '#2196F3',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer'
              }}
            >
              Send Test Notification
            </button>
          </>
        )}

        {permission === 'denied' && (
          <p style={{ color: 'red' }}>
            Notifications are blocked. Please enable them in your browser settings.
          </p>
        )}
      </div>

      {onClose && (
        <button
          onClick={onClose}
          style={{
            marginTop: '20px',
            padding: '10px 20px',
            backgroundColor: '#9E9E9E',
            color: 'white',
            border: 'none',
            borderRadius: '4px',
            cursor: 'pointer'
          }}
        >
          Close
        </button>
      )}
    </div>
  );
};

export default NotificationSettings;
