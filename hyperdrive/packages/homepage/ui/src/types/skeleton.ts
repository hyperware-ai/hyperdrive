// Type definitions for the Skeleton App
// These should match the types defined in your Rust backend

// API Response types
export interface StatusResponse {
  counter: number;
  message_count: number;
  node: string;
}


// Method names must match the Rust function names exactly
export type ApiMethods = 
  | 'GetStatus'
  | 'IncrementCounter' 
  | 'GetMessages';

// Type-safe API call wrapper
export interface ApiCall<T> {
  [method: string]: T;
}

// Example of how to structure API calls:
// For no parameters: { "GetStatus": "" }
// For single parameter: { "IncrementCounter": 5 }
// For multiple parameters: { "MethodName": [param1, param2] }

// Store state interface
export interface SkeletonState {
  // Connection state
  nodeId: string | null;
  isConnected: boolean;
  
  // App data (mirrors backend state)
  counter: number;
  messages: string[];
  
  // UI state
  isLoading: boolean;
  error: string | null;
}