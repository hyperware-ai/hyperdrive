// TTSTT Type Definitions
// Re-export the auto-generated types from WIT
import { Ttstt } from '../../../target/ui/caller-utils';

export type Provider = Ttstt.Provider;
export type ApiKeyRole = Ttstt.ApiKeyRole;
export type ProviderConfig = Ttstt.ProviderConfig;
export type ApiKeyInfo = Ttstt.ApiKeyInfo;
export type AudioTextPair = Ttstt.AudioTextPair;

// Additional types not in WIT
export interface TtsRequest {
  text: string;
  provider?: Provider;
  voice?: string;
  model?: string;
  format?: string;
}

export interface TtsResponse {
  audio_data: string; // Base64 encoded
  format: string;
  provider: Provider;
}

export interface SttRequest {
  audioData: string; // Base64 encoded
  provider?: Provider;
  model?: string;
  language?: string;
}

export interface SttResponse {
  text: string;
  provider: Provider;
}

export interface AdminKeyResponse {
  adminKey: string;
  message: string;
}