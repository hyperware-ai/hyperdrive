import React from 'react';
import { FileExplorer } from '../../lib/api';
import FileItem from './FileItem';
import './FileList.css';

interface FileListProps {
  files: FileExplorer.FileInfo[];
  viewMode: 'list' | 'grid';
  loading: boolean;
  onNavigate: (path: string) => void;
  currentPath: string;
  onLoadSubdirectory?: (path: string) => Promise<FileExplorer.FileInfo[]>;
  onDelete?: () => void;
}

const FileList: React.FC<FileListProps> = ({ files, viewMode, loading, onNavigate, currentPath, onLoadSubdirectory, onDelete }) => {
  if (loading) {
    return <div className="file-list-loading">Loading...</div>;
  }

  if (files.length === 0) {
    return <div className="file-list-empty">No files in this directory</div>;
  }

  // Build tree structure from flat list
  const fileMap = new Map<string, FileExplorer.FileInfo & { children?: FileExplorer.FileInfo[] }>();
  const topLevelFiles: (FileExplorer.FileInfo & { children?: FileExplorer.FileInfo[] })[] = [];
  
  // First pass: create map of all files
  files.forEach(file => {
    fileMap.set(file.path, { ...file, children: [] });
  });
  
  // Second pass: build ALL parent-child relationships first
  files.forEach(file => {
    const fileWithChildren = fileMap.get(file.path)!;
    const parentPath = file.path.substring(0, file.path.lastIndexOf('/'));
    
    
    if (fileMap.has(parentPath)) {
      // This file has a parent in our list
      const parent = fileMap.get(parentPath)!;
      if (!parent.children) parent.children = [];
      parent.children.push(fileWithChildren);
    }
  });
  
  // Third pass: determine what goes at the top level
  // Handle potential leading slash mismatch between breadcrumb and double-click navigation
  const normalizedCurrentPath = currentPath.startsWith('/') && currentPath !== '/' 
    ? currentPath.substring(1) 
    : currentPath;
  const expectedParent = normalizedCurrentPath === '/' ? '' : normalizedCurrentPath;
  
  files.forEach(file => {
    const fileWithChildren = fileMap.get(file.path)!;
    const parentPath = file.path.substring(0, file.path.lastIndexOf('/'));
    
    // Only add to top level if it's a direct child of current directory and has no parent in the list
    if (!fileMap.has(parentPath) && parentPath === expectedParent) {
      topLevelFiles.push(fileWithChildren);
    }
  });

  // Sort files: directories first, then by name
  const sortFiles = (files: (FileExplorer.FileInfo & { children?: FileExplorer.FileInfo[] })[]) => {
    return [...files].sort((a, b) => {
      if (a.is_directory && !b.is_directory) return -1;
      if (!a.is_directory && b.is_directory) return 1;
      return a.name.localeCompare(b.name);
    });
  };

  // Recursively sort all children
  const sortRecursive = (files: (FileExplorer.FileInfo & { children?: FileExplorer.FileInfo[] })[]) => {
    const sorted = sortFiles(files);
    sorted.forEach(file => {
      if (file.children && file.children.length > 0) {
        file.children = sortRecursive(file.children);
      }
    });
    return sorted;
  };

  const sortedFiles = sortRecursive(topLevelFiles);

  return (
    <div className={`file-list file-list-${viewMode}`}>
      {sortedFiles.map((file) => (
        <FileItem
          key={file.path}
          file={file}
          viewMode={viewMode}
          onNavigate={onNavigate}
          depth={0}
          onLoadSubdirectory={onLoadSubdirectory}
          onDelete={onDelete}
        />
      ))}
    </div>
  );
};

export default FileList;