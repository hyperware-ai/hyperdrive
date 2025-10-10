import { create } from 'zustand';
import { FileExplorer } from '../lib/api';

interface FileExplorerStore {
  currentPath: string;
  files: FileExplorer.FileInfo[];
  selectedFiles: string[];
  expandedDirectories: Set<string>;
  uploadProgress: Map<string, number>;
  sharedLinks: Map<string, string>;
  loading: boolean;
  error: string | null;
  
  // Actions
  setCurrentPath: (path: string) => void;
  setFiles: (files: FileExplorer.FileInfo[]) => void;
  selectFile: (path: string) => void;
  deselectFile: (path: string) => void;
  toggleFileSelection: (path: string) => void;
  clearSelection: () => void;
  selectAll: () => void;
  toggleDirectoryExpanded: (path: string) => void;
  updateUploadProgress: (fileId: string, progress: number) => void;
  addSharedLink: (path: string, link: string) => void;
  removeSharedLink: (path: string) => void;
  isFileShared: (path: string) => boolean;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
}

const useFileExplorerStore = create<FileExplorerStore>((set, get) => ({
  currentPath: '', // Start with empty path to avoid loading root before home is set
  files: [],
  selectedFiles: [],
  expandedDirectories: new Set<string>(),
  uploadProgress: new Map(),
  sharedLinks: new Map(),
  loading: false,
  error: null,

  setCurrentPath: (path) => set({ currentPath: path }),
  
  setFiles: (files) => set({ files, error: null }),
  
  selectFile: (path) => set((state) => ({
    selectedFiles: [...state.selectedFiles, path]
  })),
  
  deselectFile: (path) => set((state) => ({
    selectedFiles: state.selectedFiles.filter(p => p !== path)
  })),
  
  toggleFileSelection: (path) => {
    const state = get();
    if (state.selectedFiles.includes(path)) {
      state.deselectFile(path);
    } else {
      state.selectFile(path);
    }
  },
  
  clearSelection: () => set({ selectedFiles: [] }),
  
  selectAll: () => set((state) => ({
    selectedFiles: state.files.map(f => f.path)
  })),
  
  toggleDirectoryExpanded: (path) => set((state) => {
    const newExpanded = new Set(state.expandedDirectories);
    if (newExpanded.has(path)) {
      newExpanded.delete(path);
    } else {
      newExpanded.add(path);
    }
    return { expandedDirectories: newExpanded };
  }),
  
  updateUploadProgress: (fileId, progress) => set((state) => {
    const newProgress = new Map(state.uploadProgress);
    if (progress >= 100) {
      newProgress.delete(fileId);
    } else {
      newProgress.set(fileId, progress);
    }
    return { uploadProgress: newProgress };
  }),
  
  addSharedLink: (path, link) => set((state) => {
    const newLinks = new Map(state.sharedLinks);
    newLinks.set(path, link);
    return { sharedLinks: newLinks };
  }),
  
  removeSharedLink: (path) => set((state) => {
    const newLinks = new Map(state.sharedLinks);
    newLinks.delete(path);
    return { sharedLinks: newLinks };
  }),
  
  isFileShared: (path) => {
    const state = get();
    return state.sharedLinks.has(path);
  },
  
  setLoading: (loading) => set({ loading }),
  
  setError: (error) => set({ error }),
}));

export default useFileExplorerStore;