import React, { useEffect, useRef } from 'react';
import { FileInfo, unshareFile, deleteFile, deleteDirectory } from '../../lib/api';
import useFileExplorerStore from '../../store/fileExplorer';
import './ContextMenu.css';

interface ContextMenuProps {
  position: { x: number; y: number };
  file: FileInfo;
  onClose: () => void;
  onShare: () => void;
  onDelete: () => void;
}

const ContextMenu: React.FC<ContextMenuProps> = ({ position, file, onClose, onShare, onDelete }) => {
  const menuRef = useRef<HTMLDivElement>(null);
  const { isFileShared, removeSharedLink } = useFileExplorerStore();
  const isShared = !file.isDirectory && isFileShared(file.path);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  const handleUnshare = async () => {
    try {
      await unshareFile(file.path);
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