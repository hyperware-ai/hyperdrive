import React, { useState, useRef, useEffect } from 'react';
import { FileInfo, deleteFile, deleteDirectory } from '../../lib/api';
import useFileExplorerStore from '../../store/fileExplorer';
import ContextMenu from '../ContextMenu/ContextMenu';
import ShareDialog from '../ShareDialog/ShareDialog';
import './FileItem.css';

interface FileItemProps {
  file: FileInfo & { children?: FileInfo[] };
  viewMode: 'list' | 'grid';
  onNavigate: (path: string) => void;
  depth?: number;
  onLoadSubdirectory?: (path: string) => Promise<FileInfo[]>;
  onDelete?: () => void;
}

const FileItem: React.FC<FileItemProps> = ({ file, viewMode, onNavigate, depth = 0, onLoadSubdirectory, onDelete }) => {
  const { selectedFiles, toggleFileSelection, isFileShared } = useFileExplorerStore();
  const [contextMenuOpen, setContextMenuOpen] = useState(false);
  const [contextMenuPosition, setContextMenuPosition] = useState({ x: 0, y: 0 });
  const [shareDialogOpen, setShareDialogOpen] = useState(false);
  const [menuOpenedByTouch, setMenuOpenedByTouch] = useState(false);

  const isSelected = selectedFiles.includes(file.path);
  const isShared = !file.isDirectory && isFileShared(file.path);

  const [isExpanded, setIsExpanded] = useState(false);
  const [childrenLoaded, setChildrenLoaded] = useState(false);
  const [loadedChildren, setLoadedChildren] = useState<(FileInfo & { children?: FileInfo[] })[]>([]);

  // Touch handling for iOS long-press
  const touchTimerRef = useRef<number | null>(null);
  const touchStartPos = useRef<{ x: number; y: number } | null>(null);
  const longPressTriggered = useRef(false);

  const handleClick = (e: React.MouseEvent) => {
    if (e.ctrlKey || e.metaKey) {
      toggleFileSelection(file.path);
    } else if (file.isDirectory) {
      // Single click navigates into directories
      onNavigate(file.path);
    }
    // Remove file selection on single click - no action for files
  };

  const buildTreeFromFlatList = (flatList: FileInfo[], parentPath: string): (FileInfo & { children?: FileInfo[] })[] => {
    const fileMap = new Map<string, FileInfo & { children?: FileInfo[] }>();
    const topLevelFiles: (FileInfo & { children?: FileInfo[] })[] = [];

    // First pass: create map of all files
    flatList.forEach(file => {
      fileMap.set(file.path, { ...file, children: [] });
    });

    // Second pass: build parent-child relationships
    flatList.forEach(file => {
      const fileWithChildren = fileMap.get(file.path)!;
      const fileParentPath = file.path.substring(0, file.path.lastIndexOf('/'));

      if (fileMap.has(fileParentPath)) {
        // This file has a parent in our list
        const parent = fileMap.get(fileParentPath)!;
        if (!parent.children) parent.children = [];
        parent.children.push(fileWithChildren);
      } else if (fileParentPath === parentPath) {
        // This is a direct child of the parent directory
        topLevelFiles.push(fileWithChildren);
      }
    });

    // Sort files: directories first, then by name
    const sortFiles = (files: (FileInfo & { children?: FileInfo[] })[]) => {
      return [...files].sort((a, b) => {
        if (a.isDirectory && !b.isDirectory) return -1;
        if (!a.isDirectory && b.isDirectory) return 1;
        return a.name.localeCompare(b.name);
      });
    };

    // Recursively sort all children
    const sortRecursive = (files: (FileInfo & { children?: FileInfo[] })[]) => {
      const sorted = sortFiles(files);
      sorted.forEach(file => {
        if (file.children && file.children.length > 0) {
          file.children = sortRecursive(file.children);
        }
      });
      return sorted;
    };

    return sortRecursive(topLevelFiles);
  };

  const handleExpandToggle = async (e: React.MouseEvent) => {
    e.stopPropagation();

    // If expanding and we haven't loaded children yet, load them
    if (!isExpanded && file.isDirectory && !childrenLoaded && onLoadSubdirectory) {
      const flatChildren = await onLoadSubdirectory(file.path);
      const treeChildren = buildTreeFromFlatList(flatChildren, file.path);
      setLoadedChildren(treeChildren);
      setChildrenLoaded(true);
    }

    setIsExpanded(!isExpanded);
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenuPosition({ x: e.clientX, y: e.clientY });
    setMenuOpenedByTouch(false);
    setContextMenuOpen(true);
  };

  // Touch event handlers for iOS compatibility
  const handleTouchStart = (e: React.TouchEvent) => {
    const touch = e.touches[0];
    touchStartPos.current = { x: touch.clientX, y: touch.clientY };
    longPressTriggered.current = false;

    // Start long press timer (500ms)
    touchTimerRef.current = window.setTimeout(() => {
      if (touchStartPos.current) {
        longPressTriggered.current = true;
        // Trigger context menu
        setContextMenuPosition({ x: touchStartPos.current.x, y: touchStartPos.current.y });
        setMenuOpenedByTouch(true);
        setContextMenuOpen(true);
        // Prevent default touch behavior
        e.preventDefault();
      }
    }, 500);
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    // If the touch moves more than 10px, cancel the long press
    if (touchStartPos.current && touchTimerRef.current) {
      const touch = e.touches[0];
      const deltaX = Math.abs(touch.clientX - touchStartPos.current.x);
      const deltaY = Math.abs(touch.clientY - touchStartPos.current.y);

      if (deltaX > 10 || deltaY > 10) {
        if (touchTimerRef.current) {
          clearTimeout(touchTimerRef.current);
          touchTimerRef.current = null;
        }
      }
    }
  };

  const handleTouchEnd = (e: React.TouchEvent) => {
    // Clear the timer
    if (touchTimerRef.current) {
      clearTimeout(touchTimerRef.current);
      touchTimerRef.current = null;
    }

    // If long press was triggered, prevent default click behavior
    if (longPressTriggered.current) {
      e.preventDefault();
      e.stopPropagation(); // Stop propagation to prevent menu from closing
      longPressTriggered.current = false;
    } else if (!e.defaultPrevented) {
      // Normal tap - handle as click
      if (file.isDirectory) {
        onNavigate(file.path);
      }
    }

    touchStartPos.current = null;
  };

  // Clean up timer on unmount
  useEffect(() => {
    return () => {
      if (touchTimerRef.current) {
        clearTimeout(touchTimerRef.current);
      }
    };
  }, []);

  const handleDelete = async () => {
    if (!confirm(`Delete ${file.name}?`)) return;

    try {
      if (file.isDirectory) {
        await deleteDirectory(file.path);
      } else {
        await deleteFile(file.path);
      }
      // Call the parent's onDelete callback to refresh the list
      if (onDelete) {
        onDelete();
      }
    } catch (err) {
      console.error('Failed to delete:', err);
      alert(`Failed to delete ${file.name}: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  };

  const getFileIcon = () => {
    if (file.isDirectory) {
      return isExpanded ? '📂' : '📁';
    }
    const ext = file.name.split('.').pop()?.toLowerCase();
    switch (ext) {
      case 'txt': return '📄';
      case 'pdf': return '📕';
      case 'jpg':
      case 'jpeg':
      case 'png':
      case 'gif': return '🖼️';
      case 'mp3':
      case 'wav': return '🎵';
      case 'mp4':
      case 'avi': return '🎬';
      case 'zip':
      case 'rar': return '📦';
      default: return '📄';
    }
  };

  const formatFileSize = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  // Determine which children to use - loaded children take precedence
  const childrenToRender = childrenLoaded ? loadedChildren : (file.children || []);
  const hasChildren = file.isDirectory && (childrenToRender.length > 0 || !childrenLoaded);

  return (
    <>
      <div
        className={`file-item file-item-${viewMode} ${isSelected ? 'selected' : ''}`}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
        style={{ paddingLeft: `${depth * 20 + 10}px` }}
      >
        <span
          className={`file-icon ${file.isDirectory && viewMode === 'list' ? 'clickable-folder' : ''}`}
          onClick={file.isDirectory && viewMode === 'list' ? handleExpandToggle : undefined}
        >
          {getFileIcon()}
        </span>
        <span className="file-name">{file.name}</span>
        {isShared && (
          <span className="shared-indicator" title="This file is shared">
            🔗
          </span>
        )}
        {viewMode === 'list' && (
          <>
            <span className="file-size">
              {file.isDirectory ? `${file.size} items` : formatFileSize(file.size)}
            </span>
            <span className="file-modified">
              {file.modified ? new Date(file.modified * 1000).toLocaleDateString() : '-'}
            </span>
          </>
        )}
      </div>

      {/* Render children when expanded */}
      {isExpanded && viewMode === 'list' && childrenToRender.length > 0 && (
        <div className="file-children">
          {childrenToRender.map((child) => (
            <FileItem
              key={child.path}
              file={child as FileInfo & { children?: FileInfo[] }}
              viewMode={viewMode}
              onNavigate={onNavigate}
              depth={depth + 1}
              onLoadSubdirectory={onLoadSubdirectory}
              onDelete={onDelete}
            />
          ))}
        </div>
      )}

      {contextMenuOpen && (
        <ContextMenu
          position={contextMenuPosition}
          file={file}
          onClose={() => setContextMenuOpen(false)}
          onShare={() => {
            setShareDialogOpen(true);
            setContextMenuOpen(false);
          }}
          onDelete={handleDelete}
          openedByTouch={menuOpenedByTouch}
        />
      )}

      {shareDialogOpen && (
        <ShareDialog
          file={file}
          onClose={() => setShareDialogOpen(false)}
        />
      )}
    </>
  );
};

export default FileItem;
