// Global type definitions for Hyperware environment

// The window.our object is provided by the /our.js script
// It contains the node and process identity
declare global {
  interface Window {
    our?: {
      node: string;      // e.g., "alice.os"
      process: string;   // e.g., "skeleton-app:skeleton-app:skeleton.os"
    };
  }
}

// Base URL for API calls
// In production, this is empty (same origin)
// In development, you might proxy to your local node
export const BASE_URL = '';

// Helper to check if we're in a Hyperware environment
export const isHyperwareEnvironment = (): boolean => {
  return typeof window !== 'undefined' && window.our !== undefined;
};

// Get the current node identity
export const getNodeId = (): string | null => {
  return window.our?.node || null;
};

// Get the current process identity  
export const getProcessId = (): string | null => {
  return window.our?.process || null;
};