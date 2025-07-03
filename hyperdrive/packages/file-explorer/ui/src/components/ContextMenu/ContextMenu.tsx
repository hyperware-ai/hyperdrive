import React, { useEffect, useRef, useState } from 'react';
import { FileInfo, unshareFile, deleteFile, deleteDirectory } from '../../lib/api';
import useFileExplorerStore from '../../store/fileExplorer';
import './ContextMenu.css';

interface ContextMenuProps {
  position: { x: number; y: number };
  file: FileInfo;
  onClose: () => void;
  onShare: () => void;
  onDelete: () => void;
  openedByTouch?: boolean;
}

const ContextMenu: React.FC<ContextMenuProps> = ({ position, file, onClose, onShare, onDelete, openedByTouch }) => {
  const menuRef = useRef<HTMLDivElement>(null);
  const { isFileShared, removeSharedLink } = useFileExplorerStore();
  const isShared = !file.isDirectory && isFileShared(file.path);

  // Touch drag-to-select state
  const [hoveredButton, setHoveredButton] = useState<HTMLElement | null>(null);
  const touchStartedRef = useRef(false);
  const touchMovedRef = useRef(false);
  const isDraggingRef = useRef(false);

  useEffect(() => {
    // For touch-opened menus, delay adding the outside click handlers
    // This prevents the menu from immediately closing on iOS
    let timeoutId: number | undefined;

    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    const handleTouchOutside = (e: TouchEvent) => {
      // If the touch is inside the menu, don't close
      if (menuRef.current && menuRef.current.contains(e.target as Node)) {
        return;
      }

      // For touch-opened menus, only close on deliberate outside tap
      if (openedByTouch) {
        // Check if this is a new touch interaction (not the same one that opened the menu)
        const touch = e.touches[0];
        if (touch) {
          // Store this touch interaction
          touchStartedRef.current = true;
        }
      } else {
        // For non-touch opened menus, close immediately
        onClose();
      }
    };

    const handleTouchEndOutside = (e: TouchEvent) => {
      // Only close if menu was opened by touch and user tapped outside
      if (openedByTouch && touchStartedRef.current) {
        if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
          onClose();
        }
      }
      touchStartedRef.current = false;
    };

    if (openedByTouch) {
      // Delay adding touch handlers for touch-opened menus
      timeoutId = window.setTimeout(() => {
        document.addEventListener('touchstart', handleTouchOutside, { passive: false });
        document.addEventListener('touchend', handleTouchEndOutside, { passive: false });
      }, 100); // Small delay to let the opening touch complete
    } else {
      // Add handlers immediately for mouse-opened menus
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('touchstart', handleTouchOutside);
      document.removeEventListener('touchend', handleTouchEndOutside);
    };
  }, [onClose, openedByTouch]);

  const handleUnshare = async () => {
    try {
      await unshareFile(file.path);
      removeSharedLink(file.path);
      onClose();
    } catch (err) {
      console.error('Failed to unshare file:', err);
    }
  };

  // Touch event handlers for drag-to-select
  const handleTouchStart = (e: React.TouchEvent) => {
    touchStartedRef.current = true;
    touchMovedRef.current = false;
    isDraggingRef.current = false;

    const touch = e.touches[0];
    const element = document.elementFromPoint(touch.clientX, touch.clientY);

    if (element && element.tagName === 'BUTTON' && menuRef.current?.contains(element)) {
      setHoveredButton(element as HTMLElement);
      (element as HTMLElement).classList.add('touch-hover');
    }
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    if (!touchStartedRef.current) return;

    touchMovedRef.current = true;
    isDraggingRef.current = true;

    const touch = e.touches[0];
    const element = document.elementFromPoint(touch.clientX, touch.clientY);

    // Remove hover from previous button
    if (hoveredButton) {
      hoveredButton.classList.remove('touch-hover');
    }

    // Add hover to new button if it's within the menu
    if (element && element.tagName === 'BUTTON' && menuRef.current?.contains(element)) {
      setHoveredButton(element as HTMLElement);
      (element as HTMLElement).classList.add('touch-hover');
    } else {
      setHoveredButton(null);
    }
  };

  const handleTouchEnd = (e: React.TouchEvent) => {
    const wasDragging = isDraggingRef.current;

    // Clean up hover state
    if (hoveredButton) {
      hoveredButton.classList.remove('touch-hover');

      // If user dragged to a button and released, trigger its action
      if (wasDragging) {
        e.preventDefault();
        hoveredButton.click();
      }
    }

    // Reset state
    touchStartedRef.current = false;
    touchMovedRef.current = false;
    isDraggingRef.current = false;
    setHoveredButton(null);
  };

  // Clean up on unmount
  useEffect(() => {
    return () => {
      if (hoveredButton) {
        hoveredButton.classList.remove('touch-hover');
      }
    };
  }, [hoveredButton]);

  return (
    <div
      ref={menuRef}
      className="context-menu"
      style={{ left: position.x, top: position.y }}
      onTouchStart={handleTouchStart}
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
    >
      <button onClick={() => { /* TODO */ onClose(); }}>
        📋 Copy
      </button>
      <button onClick={() => { /* TODO */ onClose(); }}>
        ✂️ Cut
      </button>
      <button onClick={() => { /* TODO */ onClose(); }}>
        📄 Rename
      </button>
      {!file.isDirectory && (
        isShared ? (
          <button onClick={handleUnshare}>
            🔓 Unshare
          </button>
        ) : (
          <button onClick={onShare}>
            🔗 Share
          </button>
        )
      )}
      <hr />
      <button onClick={() => { onDelete(); onClose(); }}>
        🗑️ Delete
      </button>
    </div>
  );
};

export default ContextMenu;
