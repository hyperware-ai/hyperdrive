import React, { useEffect, useRef } from 'react';
import { BsBell, BsX, BsTrash } from 'react-icons/bs';
import { useNotificationStore } from '../../../stores/notificationStore';
import classNames from 'classnames';

export const NotificationMenu: React.FC = () => {
  const menuRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = React.useState<{ left: string; top: string; width: string }>({
    left: '0',
    top: '0',
    width: '320px'
  });
  const {
    notifications,
    menuOpen,
    setMenuOpen,
    markAsRead,
    markAllAsRead,
    clearNotifications,
    getUnseenNotifications,
  } = useNotificationStore();

  // Get notifications to display based on menu state
  const displayNotifications = menuOpen
    ? getUnseenNotifications().length > 0
      ? getUnseenNotifications()
      : notifications // Show all if no unseen
    : [];

  // Calculate position when menu opens
  useEffect(() => {
    if (menuOpen) {
      const calculatePosition = () => {
        const button = document.querySelector('[data-notification-button]');
        if (!button) return;

        const buttonRect = button.getBoundingClientRect();
        const viewportWidth = window.innerWidth;
        const desiredWidth = 320; // Desired menu width
        const margin = 16; // Minimum margin from screen edges

        // Calculate maximum possible width
        const maxWidth = Math.min(viewportWidth - (2 * margin), desiredWidth);
        const menuWidth = maxWidth;

        // For desktop: position menu to the left of the button, aligned with right edge
        // For mobile: center the menu or align it to avoid cutoff
        let leftPosition;

        if (viewportWidth > 640) {
          // Desktop: align menu's right edge with button's right edge
          leftPosition = buttonRect.right - menuWidth;

          // If menu would go off the left edge, shift it right
          if (leftPosition < margin) {
            leftPosition = margin;
          }
        } else {
          // Mobile: ensure the menu is fully visible
          // Try to center it, but ensure it doesn't go off screen
          leftPosition = (viewportWidth - menuWidth) / 2;

          // Ensure it doesn't go off left edge
          if (leftPosition < margin) {
            leftPosition = margin;
          }

          // Ensure it doesn't go off right edge
          if (leftPosition + menuWidth > viewportWidth - margin) {
            leftPosition = viewportWidth - margin - menuWidth;
          }
        }

        // Calculate top position to place menu below the button
        const topPosition = buttonRect.bottom + 8; // 8px gap below button

        setPosition({
          left: `${leftPosition}px`,
          top: `${topPosition}px`,
          width: `${menuWidth}px`
        });
      };

      calculatePosition();

      // Recalculate on resize and scroll to handle position changes
      window.addEventListener('resize', calculatePosition);
      window.addEventListener('scroll', calculatePosition);

      return () => {
        window.removeEventListener('resize', calculatePosition);
        window.removeEventListener('scroll', calculatePosition);
      };
    }
  }, [menuOpen]);

  // Handle click outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        const bellButton = document.querySelector('[data-notification-button]');
        if (bellButton && !bellButton.contains(event.target as Node)) {
          setMenuOpen(false);
        }
      }
    };

    if (menuOpen) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [menuOpen, setMenuOpen]);

  if (!menuOpen) return null;

  return (
    <div
      ref={menuRef}
      className="fixed max-h-96 bg-white dark:bg-gray-900 rounded-2xl shadow-2xl border border-gray-200 dark:border-gray-700 z-50 overflow-hidden animate-in slide-in-from-top-2 fade-in duration-200"
      style={{
        left: position.left,
        top: position.top,
        width: position.width,
        maxWidth: 'calc(100vw - 2rem)'
      }}
    >
      <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <h3 className="font-semibold text-gray-900 dark:text-white">
          Notifications
        </h3>
        <div className="flex items-center gap-2">
          {displayNotifications.length > 0 && (
            <>
              <button
                onClick={markAllAsRead}
                className="text-xs px-2 py-1 bg-iris text-neon rounded-lg hover:opacity-90 transition-opacity"
                title="Mark all as read"
              >
                Read all
              </button>
              <button
                onClick={clearNotifications}
                className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
                title="Clear all"
              >
                <BsTrash className="w-4 h-4" />
              </button>
            </>
          )}
        </div>
      </div>

      <div className="overflow-y-auto max-h-80">
        {displayNotifications.length === 0 ? (
          <div className="p-8 text-center text-gray-500 dark:text-gray-400">
            <BsBell className="w-12 h-12 mx-auto mb-3 opacity-30" />
            <p className="text-sm">No new notifications</p>
          </div>
        ) : (
          <div className="divide-y divide-gray-100 dark:divide-gray-800">
            {displayNotifications.map((notification) => (
              <div
                key={notification.id}
                className={classNames(
                  "p-4 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors cursor-pointer",
                  {
                    "bg-blue-50 dark:bg-blue-900/20": !notification.read,
                  }
                )}
                onClick={() => markAsRead(notification.id)}
              >
                <div className="flex items-start gap-3">
                  {notification.icon ? (
                    <img
                      src={notification.icon}
                      alt={notification.appLabel}
                      className="w-10 h-10 rounded-lg object-cover flex-shrink-0"
                    />
                  ) : (
                    <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-iris to-neon flex items-center justify-center flex-shrink-0">
                      <BsBell className="w-5 h-5 text-white" />
                    </div>
                  )}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-start justify-between gap-2">
                      <div className="flex-1">
                        <p className="text-sm font-medium text-gray-900 dark:text-white">
                          {notification.title}
                        </p>
                        <p className="text-sm text-gray-600 dark:text-gray-400 mt-1 wrap-anywhere">
                          {notification.body}
                        </p>
                        <p className="text-xs text-gray-400 dark:text-gray-500 mt-2">
                          {notification.appLabel} • {formatTimestamp(notification.timestamp)}
                        </p>
                      </div>
                      {!notification.read && (
                        <div className="w-2 h-2 bg-blue-500 rounded-full flex-shrink-0 mt-2" />
                      )}
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

function formatTimestamp(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;
  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (days > 0) return `${days}d ago`;
  if (hours > 0) return `${hours}h ago`;
  if (minutes > 0) return `${minutes}m ago`;
  return 'Just now';
}
