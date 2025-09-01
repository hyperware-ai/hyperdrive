// Service worker for PWA installability and push notifications
// This is an online-only PWA, so we only cache the app shell

const CACHE_NAME = 'hyperdrive-v1';
const urlsToCache = [
  '/',
  '/index.html',
  '/manifest.json'
];

// Install event - cache app shell
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then((cache) => {
        console.log('Opened cache');
        return cache.addAll(urlsToCache);
      })
      .then(() => self.skipWaiting())
  );
});

// Activate event - clean up old caches
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((cacheNames) => {
      return Promise.all(
        cacheNames.map((cacheName) => {
          if (cacheName !== CACHE_NAME) {
            console.log('Deleting old cache:', cacheName);
            return caches.delete(cacheName);
          }
        })
      );
    }).then(() => self.clients.claim())
  );
});

// Fetch event - network first strategy (online-only app)
self.addEventListener('fetch', (event) => {
  // Only handle same-origin, GET requests for the app shell. Let the browser
  // handle everything else (prevents returning undefined Responses).
  if (event.request.method !== 'GET') return;
  if (!event.request.url.startsWith(self.location.origin)) return;

  const url = new URL(event.request.url);
  const isAppShellRequest = urlsToCache.includes(url.pathname);

  if (!isAppShellRequest) return; // don't intercept non-app-shell routes

  event.respondWith(
    fetch(event.request)
      .then((response) => {
        const responseToCache = response.clone();
        caches.open(CACHE_NAME).then((cache) => {
          cache.put(event.request, responseToCache);
        });
        return response;
      })
      .catch(async () => {
        const cached = await caches.match(event.request);
        return cached || new Response('', { status: 504, statusText: 'Gateway Timeout' });
      })
  );
});

// Listen for messages from the app
self.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'SKIP_WAITING') {
    self.skipWaiting();
  }
});

// Push notification event handler
self.addEventListener('push', (event) => {
  if (!event.data) {
    console.log('Push event but no data');
    return;
  }

  let notificationData;
  try {
    notificationData = event.data.json();
  } catch (e) {
    console.error('Failed to parse push notification data:', e);
    return;
  }

  const title = notificationData.title || 'New Notification';
  const options = {
    body: notificationData.body || '',
    icon: notificationData.icon || '/icon-180.png',
    badge: '/icon-180.png',
    vibrate: [100, 50, 100],
    data: notificationData.data || {},
    actions: notificationData.actions || [],
    requireInteraction: false,
    tag: notificationData.tag || 'default',
    renotify: true
  };

  event.waitUntil(
    self.registration.showNotification(title, options)
  );
});

// Notification click event handler
self.addEventListener('notificationclick', (event) => {
  event.notification.close();

  // Get the notification data
  const data = event.notification.data || {};

  // Default URL is the root, but can be overridden by notification data
  let urlToOpen = new URL('/', self.location.origin);

  if (data.url) {
    try {
      urlToOpen = new URL(data.url, self.location.origin);
    } catch (e) {
      console.error('Invalid URL in notification data:', data.url);
    }
  }

  // Handle notification action clicks
  if (event.action) {
    // Action-specific handling can be added here
    if (data.actions && data.actions[event.action]) {
      try {
        urlToOpen = new URL(data.actions[event.action], self.location.origin);
      } catch (e) {
        console.error('Invalid action URL:', data.actions[event.action]);
      }
    }
  }

  event.waitUntil(
    clients.matchAll({
      type: 'window',
      includeUncontrolled: true
    }).then((windowClients) => {
      // Check if there's already a window open
      for (let client of windowClients) {
        if (client.url === urlToOpen.href && 'focus' in client) {
          return client.focus();
        }
      }
      // If no window is open, open a new one
      if (clients.openWindow) {
        return clients.openWindow(urlToOpen.href);
      }
    })
  );
});

// Handle push subscription change (e.g., when subscription expires)
self.addEventListener('pushsubscriptionchange', (event) => {
  event.waitUntil(
    self.registration.pushManager.subscribe(event.oldSubscription.options)
      .then((subscription) => {
        // Send new subscription to server
        return fetch('/api/notifications/subscribe', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify(subscription.toJSON())
        });
      })
  );
});
