import React, { useEffect, useRef, useState } from 'react';
import { FileExplorer } from '../../lib/api';
import useFileExplorerStore from '../../store/fileExplorer';
import './ContextMenu.css';

interface ContextMenuProps {
  position: { x: number; y: number };
  file: FileExplorer.FileInfo;
  onClose: () => void;
  onShare: () => void;
  onDelete: () => void;
  openedByTouch?: boolean;
}

const ContextMenu: React.FC<ContextMenuProps> = ({ position, file, onClose, onShare, onDelete, openedByTouch }) => {
  const menuRef = useRef<HTMLDivElement>(null);
  const { isFileShared, removeSharedLink } = useFileExplorerStore();
  const isShared = !file.is_directory && isFileShared(file.path);

  // Track if a new touch has started after menu opened
  const touchStartedRef = useRef(false);

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
      await FileExplorer.unshare_file(file.path);
      removeSharedLink(file.path);
      onClose();
    } catch (err) {
      console.error('Failed to unshare file:', err);
    }
  };


  return (
    <div
      ref={menuRef}
      className="context-menu"
      style={{ left: position.x, top: position.y }}
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
      {!file.is_directory && (
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
