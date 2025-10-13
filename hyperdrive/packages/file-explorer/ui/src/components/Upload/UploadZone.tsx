import React, { useState, useRef } from 'react';
import { FileExplorer } from '../../lib/api';
import useFileExplorerStore from '../../store/fileExplorer';
import './UploadZone.css';

interface UploadZoneProps {
  currentPath: string;
  onUploadComplete: () => void;
  children: React.ReactNode;
}

const UploadZone: React.FC<UploadZoneProps> = ({ currentPath, onUploadComplete, children }) => {
  const [isDragging, setIsDragging] = useState(false);
  const { updateUploadProgress, setError } = useFileExplorerStore();
  const dragCounter = useRef(0);

  // Listen for upload events from the toolbar
  React.useEffect(() => {
    const handleUploadEvent = async (e: CustomEvent) => {
      const files = e.detail.files as File[];
      for (const file of files) {
        await handleFileUpload(file);
      }
    };

    window.addEventListener('upload-files', handleUploadEvent as EventListener);
    return () => {
      window.removeEventListener('upload-files', handleUploadEvent as EventListener);
    };
  }, [currentPath, onUploadComplete]);

  const handleDrag = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  };

  const handleDragIn = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounter.current++;
    if (e.dataTransfer.items && e.dataTransfer.items.length > 0) {
      setIsDragging(true);
    }
  };

  const handleDragOut = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounter.current--;
    if (dragCounter.current === 0) {
      setIsDragging(false);
    }
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
    dragCounter.current = 0;

    // Handle both files and folders
    const items = e.dataTransfer.items;
    if (items) {
      // Use DataTransferItemList interface when available
      const files: File[] = [];
      
      for (let i = 0; i < items.length; i++) {
        const item = items[i];
        if (item.kind === 'file') {
          const entry = item.webkitGetAsEntry?.();
          if (entry) {
            await processEntry(entry, '', files);
          } else {
            // Fallback to getAsFile for browsers that don't support webkitGetAsEntry
            const file = item.getAsFile();
            if (file) files.push(file);
          }
        }
      }
      
      // Upload all collected files
      for (const file of files) {
        await handleFileUpload(file);
      }
    } else {
      // Fallback for browsers that don't support DataTransferItemList
      const files = Array.from(e.dataTransfer.files);
      for (const file of files) {
        await handleFileUpload(file);
      }
    }
  };

  // Recursively process file system entries (for folder support)
  const processEntry = async (entry: any, path: string, files: File[]): Promise<void> => {
    if (entry.isFile) {
      // Handle file
      return new Promise((resolve) => {
        entry.file((file: File) => {
          // Add the relative path to the file object
          Object.defineProperty(file, 'webkitRelativePath', {
            value: path + file.name,
            writable: false
          });
          files.push(file);
          resolve();
        });
      });
    } else if (entry.isDirectory) {
      // Handle directory
      const dirReader = entry.createReader();
      return new Promise((resolve) => {
        dirReader.readEntries(async (entries: any[]) => {
          for (const childEntry of entries) {
            await processEntry(childEntry, path + entry.name + '/', files);
          }
          resolve();
        });
      });
    }
  };

  const handleFileUpload = async (file: File) => {
    const fileId = `${file.name}-${Date.now()}`;
    
    try {
      updateUploadProgress(fileId, 0);
      
      const content = await file.arrayBuffer();
      const contentArray = Array.from(new Uint8Array(content));
      
      // Check if file is part of a folder upload
      const relativePath = (file as any).webkitRelativePath || '';
      let uploadPath = currentPath;
      let fileName = file.name;
      
      if (relativePath) {
        // Handle folder structure
        const pathParts = relativePath.split('/');
        fileName = pathParts.pop() || file.name;
        
        // Create nested folder structure
        for (const folderName of pathParts) {
          uploadPath = uploadPath === '/' ? `/${folderName}` : `${uploadPath}/${folderName}`;
          try {
            await FileExplorer.create_directory(uploadPath);
          } catch (err) {
            // Ignore if directory already exists
            console.log(`Directory ${uploadPath} might already exist`);
          }
        }
      }
      
      // Simulate upload progress
      updateUploadProgress(fileId, 50);
      
      await FileExplorer.upload_file(uploadPath, fileName, contentArray);
      
      updateUploadProgress(fileId, 100);
      onUploadComplete();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Upload failed');
      updateUploadProgress(fileId, 100); // Remove from progress
    }
  };

  return (
    <div
      className={`upload-zone ${isDragging ? 'dragging' : ''}`}
      onDrop={handleDrop}
      onDragOver={handleDrag}
      onDragEnter={handleDragIn}
      onDragLeave={handleDragOut}
    >
      {children}
      {isDragging && (
        <div className="upload-overlay">
          <div className="upload-message">
            Drop files or folders here to upload
          </div>
        </div>
      )}
    </div>
  );
};

export default UploadZone;