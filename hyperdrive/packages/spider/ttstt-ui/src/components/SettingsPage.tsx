import React, { useState, useEffect } from 'react';
import useTtsttStore from '../store/ttstt';
import { Ttstt } from '../../../target/ui/caller-utils';
type ProviderConfig = Ttstt.ProviderConfig;

// Available voices for OpenAI
const OPENAI_VOICES = [
  'alloy', 'ash', 'ballad', 'coral', 'echo',
  'fable', 'nova', 'onyx', 'sage', 'shimmer', 'verse'
];

// Available voices for ElevenLabs
const ELEVENLABS_VOICES = [
  'Rachel', 'Drew', 'Clyde', 'Paul', 'Aria',
  'Domi', 'Dave', 'Roger', 'Fin', 'Sarah'
];

function SettingsPage() {
  const {
    providers,
    loadProviders,
    addProvider,
    removeProvider,
    setDefaultProvider,
    isLoading,
  } = useTtsttStore();

  // Load providers on mount
  useEffect(() => {
    loadProviders();
  }, []);

  // Form state
  const [showAddForm, setShowAddForm] = useState(false);
  const [newProvider, setNewProvider] = useState<Partial<ProviderConfig>>({
    provider: 'OpenAI' as Ttstt.Provider,
    api_key: '',
    is_default_tts: false,
    is_default_stt: false,
    default_voice: 'nova',
    default_speed: 1.5,
  });

  // Get voices based on provider
  const getVoicesForProvider = (provider: Ttstt.Provider | string) => {
    if (provider === 'ElevenLabs') {
      return ELEVENLABS_VOICES;
    }
    return OPENAI_VOICES;
  };

  // Get default voice for provider
  const getDefaultVoiceForProvider = (provider: Ttstt.Provider | string) => {
    if (provider === 'ElevenLabs') {
      return 'Rachel';
    }
    return 'nova';
  };

  const handleAddProvider = async (e: React.FormEvent) => {
    e.preventDefault();
    if (newProvider.api_key && newProvider.provider) {
      await addProvider(newProvider as ProviderConfig);
      setNewProvider({
        provider: 'OpenAI' as Ttstt.Provider,
        api_key: '',
        is_default_tts: false,
        is_default_stt: false,
        default_voice: 'nova',
        default_speed: 1.5,
      });
      setShowAddForm(false);
    }
  };

  const handleRemoveProvider = async (provider: Ttstt.Provider) => {
    if (window.confirm(`Remove ${provider} provider?`)) {
      await removeProvider(provider);
    }
  };

  const handleSetDefault = async (provider: Ttstt.Provider, type: 'tts' | 'stt') => {
    await setDefaultProvider(provider, type);
  };

  const handleUpdateProviderSettings = async (provider: Ttstt.Provider, voice: string, speed: number) => {
    // Find the existing provider config
    const existingProvider = providers.find(p => p.provider === provider);
    if (existingProvider) {
      // Update the provider with new settings
      await removeProvider(provider);
      await addProvider({
        provider: provider,
        api_key: '',  // We don't have the original API key here
        is_default_tts: existingProvider.is_default_tts,
        is_default_stt: existingProvider.is_default_stt,
        default_voice: voice,
        default_speed: speed,
      });
    }
  };

  return (
    <div className="settings-page">
      <h2>Provider Settings</h2>

      <div className="card">
        <h3>Configured Providers</h3>
        {providers.length === 0 ? (
          <p className="text-muted">No providers configured. Click "Add Provider" below to get started.</p>
        ) : (
          <div className="flex flex-col gap-4">
            {providers.map((provider) => (
              <div key={provider.provider} className="card" style={{ background: 'var(--surface)' }}>
                <h4 style={{ marginBottom: '1rem' }}>{provider.provider}</h4>

                {/* Voice and Speed Settings */}
                <div className="form-row" style={{ marginBottom: '1rem' }}>
                  <div className="form-group" style={{ marginBottom: 0 }}>
                    <label>Default Voice</label>
                    <select
                      value={provider.default_voice || getDefaultVoiceForProvider(provider.provider)}
                      onChange={(e) => handleUpdateProviderSettings(
                        provider.provider,
                        e.target.value,
                        provider.default_speed || 1.5
                      )}
                      disabled={isLoading}
                    >
                      {getVoicesForProvider(provider.provider).map(voice => (
                        <option key={voice} value={voice}>
                          {voice.charAt(0).toUpperCase() + voice.slice(1)}
                        </option>
                      ))}
                    </select>
                  </div>

                  <div className="form-group" style={{ marginBottom: 0 }}>
                    <label>
                      Default Speed
                      {provider.provider === 'ElevenLabs' && (
                        <span style={{ fontSize: '0.85rem', color: 'var(--text-muted)', marginLeft: '0.5rem' }}>
                          (affects voice dynamics)
                        </span>
                      )}
                    </label>
                    <div className="flex items-center gap-2">
                      <input
                        type="range"
                        min="0.25"
                        max="4.0"
                        step="0.25"
                        value={provider.default_speed || 1.5}
                        onChange={(e) => handleUpdateProviderSettings(
                          provider.provider,
                          provider.default_voice || getDefaultVoiceForProvider(provider.provider),
                          parseFloat(e.target.value)
                        )}
                        disabled={isLoading}
                        style={{ flex: 1 }}
                      />
                      <span style={{ minWidth: '3rem', textAlign: 'right' }}>
                        {(provider.default_speed || 1.5).toFixed(2)}x
                      </span>
                    </div>
                  </div>
                </div>

                {/* Default Settings */}
                <div className="flex gap-4" style={{ marginBottom: '1rem' }}>
                  <label className="flex items-center gap-2" style={{ marginBottom: 0 }}>
                    <input
                      type="radio"
                      name="default-tts"
                      checked={provider.is_default_tts}
                      onChange={() => handleSetDefault(provider.provider, 'tts')}
                      disabled={isLoading}
                      style={{ width: 'auto' }}
                    />
                    <span>Default for TTS</span>
                  </label>

                  {provider.provider !== 'ElevenLabs' && (
                    <label className="flex items-center gap-2" style={{ marginBottom: 0 }}>
                      <input
                        type="radio"
                        name="default-stt"
                        checked={provider.is_default_stt}
                        onChange={() => handleSetDefault(provider.provider, 'stt')}
                        disabled={isLoading}
                        style={{ width: 'auto' }}
                      />
                      <span>Default for STT</span>
                    </label>
                  )}
                </div>

                {/* Remove Button */}
                <button
                  onClick={() => handleRemoveProvider(provider.provider)}
                  disabled={isLoading}
                  className="danger"
                  style={{ fontSize: '0.875rem' }}
                >
                  Remove Provider
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="mt-4">
        <button
          onClick={() => setShowAddForm(!showAddForm)}
          disabled={isLoading}
          className="primary"
        >
          {showAddForm ? 'Cancel' : '+ Add Provider'}
        </button>

        {showAddForm && (
          <form onSubmit={handleAddProvider} className="card mt-3">
            <h3>Add New Provider</h3>
            <div className="form-group">
              <label>Provider</label>
              <select
                value={newProvider.provider || 'OpenAI'}
                onChange={(e) => {
                  const selectedProvider = e.target.value as Ttstt.Provider;
                  setNewProvider({
                    ...newProvider,
                    provider: selectedProvider,
                    default_voice: getDefaultVoiceForProvider(selectedProvider)
                  });
                }}
                disabled={isLoading}
              >
                <option value="OpenAI">OpenAI</option>
                <option value="ElevenLabs">ElevenLabs</option>
              </select>
            </div>

            <div className="form-group">
              <label>API Key</label>
              <input
                type="password"
                value={newProvider.api_key}
                onChange={(e) => setNewProvider({ ...newProvider, api_key: e.target.value })}
                placeholder="Enter your provider API key"
                required
                disabled={isLoading}
              />
            </div>

            <div className="form-row">
              <div className="form-group">
                <label>Default Voice</label>
                <select
                  value={newProvider.default_voice || getDefaultVoiceForProvider(newProvider.provider || Ttstt.Provider.OpenAI)}
                  onChange={(e) => setNewProvider({ ...newProvider, default_voice: e.target.value })}
                  disabled={isLoading}
                >
                  {getVoicesForProvider(newProvider.provider || Ttstt.Provider.OpenAI).map(voice => (
                    <option key={voice} value={voice}>
                      {voice.charAt(0).toUpperCase() + voice.slice(1)}
                    </option>
                  ))}
                </select>
              </div>

              <div className="form-group">
                <label>
                  Default Speed
                  {newProvider.provider === 'ElevenLabs' && (
                    <span style={{ fontSize: '0.85rem', color: 'var(--text-muted)', marginLeft: '0.5rem' }}>
                      (affects voice dynamics)
                    </span>
                  )}
                </label>
                <div className="flex items-center gap-2">
                  <input
                    type="range"
                    min="0.25"
                    max="4.0"
                    step="0.25"
                    value={newProvider.default_speed || 1.5}
                    onChange={(e) => setNewProvider({ ...newProvider, default_speed: parseFloat(e.target.value) })}
                    disabled={isLoading}
                    style={{ flex: 1 }}
                  />
                  <span style={{ minWidth: '3rem', textAlign: 'right' }}>
                    {(newProvider.default_speed || 1.5).toFixed(2)}x
                  </span>
                </div>
              </div>
            </div>

            <div className="flex gap-4 mb-4">
              <label className="flex items-center gap-2" style={{ marginBottom: 0, cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={newProvider.is_default_tts}
                  onChange={(e) => setNewProvider({ ...newProvider, is_default_tts: e.target.checked })}
                  disabled={isLoading}
                  style={{ width: 'auto' }}
                />
                <span>Set as default TTS provider</span>
              </label>

              {newProvider.provider !== 'ElevenLabs' && (
                <label className="flex items-center gap-2" style={{ marginBottom: 0, cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={newProvider.is_default_stt}
                    onChange={(e) => setNewProvider({ ...newProvider, is_default_stt: e.target.checked })}
                    disabled={isLoading}
                    style={{ width: 'auto' }}
                  />
                  <span>Set as default STT provider</span>
                </label>
              )}
            </div>

            <button type="submit" disabled={isLoading || !newProvider.api_key} className="primary">
              Add Provider
            </button>
          </form>
        )}
      </div>
    </div>
  );
}

export default SettingsPage;
