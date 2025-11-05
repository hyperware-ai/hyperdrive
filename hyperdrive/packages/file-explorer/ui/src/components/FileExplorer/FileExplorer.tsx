import React, { useEffect, useState } from 'react';
import useFileExplorerStore from '../../store/fileExplorer';
import { FileExplorer as FileExplorerAPI } from '../../lib/api';
import FileList from './FileList';
import Breadcrumb from './Breadcrumb';
import Toolbar from './Toolbar';
import UploadZone from '../Upload/UploadZone';
import './FileExplorer.css';

const FileExplorer: React.FC = () => {
  const {
    currentPath,
    files,
    selectedFiles,
    loading,
    error,
    setCurrentPath,
    setFiles,
    setLoading,
    setError,
    clearSelection
  } = useFileExplorerStore();

  const [viewMode, setViewMode] = useState<'list' | 'grid'>('list');
  const [showNewMenu, setShowNewMenu] = useState(false);
  const newMenuRef = React.useRef<HTMLDivElement>(null);

  // Close new menu when clicking outside
  React.useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (newMenuRef.current && !newMenuRef.current.contains(e.target as Node)) {
        setShowNewMenu(false);
      }
    };

    if (showNewMenu) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [showNewMenu]);

  const loadDirectory = async (path: string) => {
    try {
      setLoading(true);
      setError(null);
      const fileList = await FileExplorerAPI.list_directory(path);
      
      // Backend returns files for the requested directory with 2 levels of depth
      // We need to include all files so the tree structure works, but we'll filter
      // what's shown at the top level in FileList
      const filteredFiles = fileList.filter(file => {
        if (file.path === path) return false; // Exclude the directory itself
        
        // For the tree to work properly, we need to include all files
        // The FileList component will handle showing only direct children at top level
        return true;
      });
      
      setFiles(filteredFiles);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load directory');
    } finally {
      setLoading(false);
    }
  };

  const loadSubdirectory = async (path: string): Promise<FileExplorerAPI.FileInfo[]> => {
    try {
      const fileList = await FileExplorerAPI.list_directory(path);
      
      // Filter out the directory itself and return only its contents
      const filteredFiles = fileList.filter(file => {
        if (file.path === path) return false; // Exclude the directory itself
        return true;
      });
      
      return filteredFiles;
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load subdirectory');
      return [];
    }
  };

  // Initialize with home directory on first load
  useEffect(() => {
    const initializeDirectory = async () => {
      try {
        const cwd = await FileExplorerAPI.get_current_directory();
        setCurrentPath(cwd);
      } catch (err) {
        // If getting cwd fails, fall back to root
        console.error('Failed to get current directory:', err);
        setCurrentPath('/');
      }
    };
    
    initializeDirectory();
  }, []);

  // Load directory whenever path changes
  useEffect(() => {
    if (currentPath) {
      loadDirectory(currentPath);
    }
  }, [currentPath]);

  const handleNavigate = (path: string) => {
    clearSelection();
    setCurrentPath(path);
  };

  const handleCreateFolder = async () => {
    setShowNewMenu(false);
    const folderName = prompt('Enter folder name:');
    if (!folderName) return;

    const newPath = currentPath === '/' 
      ? `/${folderName}`
      : `${currentPath}/${folderName}`;

    try {
      await FileExplorerAPI.create_directory(newPath);
      await loadDirectory(currentPath);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create folder');
    }
  };

  const handleCreateFile = async () => {
    setShowNewMenu(false);
    const fileName = prompt('Enter file name:');
    if (!fileName) return;

    const newPath = currentPath === '/' 
      ? `/${fileName}`
      : `${currentPath}/${fileName}`;

    try {
      // Create an empty file
      await FileExplorerAPI.create_file(newPath, []);
      await loadDirectory(currentPath);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create file');
    }
  };

  const handleDelete = async () => {
    if (selectedFiles.length === 0) return;
    
    if (!confirm(`Delete ${selectedFiles.length} item(s)?`)) return;

    try {
      for (const path of selectedFiles) {
        const file = files.find(f => f.path === path);
        if (file?.is_directory) {
          await FileExplorerAPI.delete_directory(path);
        } else {
          await FileExplorerAPI.delete_file(path);
        }
      }
      clearSelection();
      await loadDirectory(currentPath);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete items');
    }
  };

  const handleRefresh = () => {
    loadDirectory(currentPath);
  };

  const handleNewMenu = () => {
    setShowNewMenu(!showNewMenu);
  };

  const handleFileUpload = () => {
    const fileInput = document.createElement('input');
    fileInput.type = 'file';
    fileInput.multiple = true;
    
    fileInput.onchange = async (e) => {
      const target = e.target as HTMLInputElement;
      const files = target.files;
      if (!files || files.length === 0) return;
      
      // Trigger the upload process for each file
      const event = new CustomEvent('upload-files', { 
        detail: { files: Array.from(files) } 
      });
      window.dispatchEvent(event);
    };
    
    fileInput.click();
  };

  const handleFolderUpload = () => {
    const fileInput = document.createElement('input');
    fileInput.type = 'file';
    fileInput.multiple = true;
    // Enable folder selection
    fileInput.webkitdirectory = true;
    (fileInput as any).directory = true;
    
    fileInput.onchange = async (e) => {
      const target = e.target as HTMLInputElement;
      const files = target.files;
      if (!files || files.length === 0) return;
      
      // Trigger the upload process for each file
      const event = new CustomEvent('upload-files', { 
        detail: { files: Array.from(files) } 
      });
      window.dispatchEvent(event);
    };
    
    fileInput.click();
  };

  return (
    <div className="file-explorer">
      <Toolbar
        viewMode={viewMode}
        onViewModeChange={setViewMode}
        onNewMenu={handleNewMenu}
        onUploadFiles={handleFileUpload}
        onUploadFolder={handleFolderUpload}
        onRefresh={handleRefresh}
      />
      
      {showNewMenu && (
        <div 
          ref={newMenuRef}
          style={{
            position: 'absolute',
            top: '50px',
            left: '10px',
            background: 'var(--bg-secondary)',
            border: '1px solid var(--border-color)',
            borderRadius: '4px',
            boxShadow: '0 2px 8px rgba(0,0,0,0.15)',
            zIndex: 1000,
            minWidth: '150px'
          }}
        >
          <button 
            onClick={handleCreateFile}
            style={{
              display: 'block',
              width: '100%',
              padding: '8px 16px',
              border: 'none',
              background: 'transparent',
              textAlign: 'left',
              cursor: 'pointer',
              fontSize: '14px'
            }}
            onMouseEnter={(e) => e.currentTarget.style.background = 'var(--bg-hover)'}
            onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
          >
            📄 New File
          </button>
          <button 
            onClick={handleCreateFolder}
            style={{
              display: 'block',
              width: '100%',
              padding: '8px 16px',
              border: 'none',
              background: 'transparent',
              textAlign: 'left',
              cursor: 'pointer',
              fontSize: '14px'
            }}
            onMouseEnter={(e) => e.currentTarget.style.background = 'var(--bg-hover)'}
            onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
          >
            📁 New Folder
          </button>
        </div>
      )}
      
      <Breadcrumb 
        currentPath={currentPath}
        onNavigate={handleNavigate}
      />
      
      {error && (
        <div className="error-message">
          {error}
        </div>
      )}
      
      <UploadZone
        currentPath={currentPath}
        onUploadComplete={() => loadDirectory(currentPath)}
      >
        <FileList
          files={files}
          viewMode={viewMode}
          loading={loading}
          onNavigate={handleNavigate}
          currentPath={currentPath}
          onLoadSubdirectory={loadSubdirectory}
          onDelete={() => loadDirectory(currentPath)}
        />
      </UploadZone>
    </div>
  );
};

export default FileExplorer;