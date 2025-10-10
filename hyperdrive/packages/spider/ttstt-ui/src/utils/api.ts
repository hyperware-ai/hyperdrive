// API utilities for TTSTT

import { Ttstt } from '../../../target/ui/caller-utils';

// Re-export all the generated API functions with snake_case names
export const addProvider = Ttstt.add_provider;
export const removeProvider = Ttstt.remove_provider;
export const getProviders = Ttstt.get_providers;
export const setDefaultProvider = Ttstt.set_default_provider;
export const generateApiKey = Ttstt.generate_api_key;
export const revokeApiKey = Ttstt.revoke_api_key;
export const listApiKeys = Ttstt.list_api_keys;
export const testTts = Ttstt.test_tts;
export const testStt = Ttstt.test_stt;
export const getHistory = Ttstt.get_history;
export const getAudioTextPair = Ttstt.get_audio_text_pair;
export const getAdminKey = Ttstt.get_admin_key;

// Re-export the error class for convenience
export { ApiError } from '../../../target/ui/caller-utils';