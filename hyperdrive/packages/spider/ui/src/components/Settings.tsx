import { useState, useEffect } from 'react';
import { useSpiderStore } from '../store/spider';

export default function Settings() {
  const { config, isLoading, error, updateConfig, loadMcpServers } = useSpiderStore();
  const [provider, setProvider] = useState(config.defaultLlmProvider);
  const [model, setModel] = useState('');
  const [maxTokens, setMaxTokens] = useState(config.maxTokens);
  const [temperature, setTemperature] = useState(config.temperature);
  const [buildContainerWsUri, setBuildContainerWsUri] = useState(config.buildContainerWsUri || '');
  const [buildContainerApiKey, setBuildContainerApiKey] = useState(config.buildContainerApiKey || '');
  const [showSelfHosting, setShowSelfHosting] = useState(false);

  // Model options based on provider
  const modelOptions = {
    anthropic: [
      { value: 'claude-opus-4-1-20250805', label: 'Claude 4.1 Opus' },
      { value: 'claude-sonnet-4-20250514', label: 'Claude 4 Sonnet' }
    ],
    openai: [
      { value: 'gpt-4o', label: 'GPT-4o' },
      { value: 'gpt-4o-mini', label: 'GPT-4o Mini' },
      { value: 'gpt-4-turbo', label: 'GPT-4 Turbo' },
      { value: 'gpt-4', label: 'GPT-4' },
      { value: 'gpt-3.5-turbo', label: 'GPT-3.5 Turbo' }
    ],
    google: [
      { value: 'gemini-2.0-flash-exp', label: 'Gemini 2.0 Flash (Experimental)' },
      { value: 'gemini-1.5-pro', label: 'Gemini 1.5 Pro' },
      { value: 'gemini-1.5-flash', label: 'Gemini 1.5 Flash' },
      { value: 'gemini-pro', label: 'Gemini Pro' }
    ]
  };

  useEffect(() => {
    setProvider(config.defaultLlmProvider);
    setMaxTokens(config.maxTokens);
    setTemperature(config.temperature);
    setBuildContainerWsUri(config.buildContainerWsUri || '');
    setBuildContainerApiKey(config.buildContainerApiKey || '');
  }, [config]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    await updateConfig({
      defaultLlmProvider: provider,
      maxTokens: maxTokens,
      temperature: temperature,
      buildContainerWsUri: buildContainerWsUri,
      buildContainerApiKey: buildContainerApiKey,
    });
    // Reload MCP servers to refresh tools list
    await loadMcpServers();
  };

  return (
    <div className="component-container">
      <div className="component-header">
        <h2>Settings</h2>
      </div>

      {error && (
        <div className="error-message">
          {error}
        </div>
      )}

      <form onSubmit={handleSubmit} className="settings-form">
        <div className="model-select-group">
          <div className="form-group">
            <label htmlFor="provider">LLM Provider</label>
            <select
              id="provider"
              value={provider}
              onChange={(e) => {
                setProvider(e.target.value);
                setModel(''); // Reset model when provider changes
              }}
            >
              <option value="anthropic">Anthropic</option>
              <option value="openai">OpenAI</option>
              <option value="google">Google</option>
            </select>
          </div>

          <div className="form-group">
            <label htmlFor="model">Model</label>
            <select
              id="model"
              value={model}
              onChange={(e) => setModel(e.target.value)}
            >
              <option value="">Default</option>
              {(modelOptions[provider as keyof typeof modelOptions] || []).map(opt => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="form-group">
          <label htmlFor="max-tokens">Max Tokens</label>
          <input
            id="max-tokens"
            type="number"
            value={maxTokens}
            onChange={(e) => setMaxTokens(Number(e.target.value))}
            min="1"
            max="100000"
          />
        </div>

        <div className="form-group">
          <label htmlFor="temperature">Temperature</label>
          <input
            id="temperature"
            type="number"
            value={temperature}
            onChange={(e) => setTemperature(Number(e.target.value))}
            min="0"
            max="2"
            step="0.1"
          />
        </div>

        <div className="form-group">
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => setShowSelfHosting(!showSelfHosting)}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '0.5rem',
              marginBottom: '1rem'
            }}
          >
            <span style={{
              transform: showSelfHosting ? 'rotate(90deg)' : 'rotate(0deg)',
              transition: 'transform 0.2s',
              display: 'inline-block'
            }}>▶</span>
            Self Hosting?
          </button>

          {showSelfHosting && (
            <div style={{
              padding: '1rem',
              backgroundColor: 'rgba(255, 255, 255, 0.02)',
              borderRadius: '0.5rem',
              marginTop: '0.5rem'
            }}>
              <div className="form-group">
                <label htmlFor="build-container-ws-uri">Build Container WebSocket URI</label>
                <input
                  id="build-container-ws-uri"
                  type="text"
                  value={buildContainerWsUri}
                  onChange={(e) => setBuildContainerWsUri(e.target.value)}
                  placeholder="ws://localhost:8091"
                />
                <small className="form-help-text">
                  WebSocket URI for your self-hosted build container
                </small>
              </div>

              <div className="form-group">
                <label htmlFor="build-container-api-key">Build Container API Key</label>
                <input
                  id="build-container-api-key"
                  type="password"
                  value={buildContainerApiKey}
                  onChange={(e) => setBuildContainerApiKey(e.target.value)}
                  placeholder="Enter API key"
                />
                <small className="form-help-text">
                  API key for authenticating with your self-hosted build container
                </small>
              </div>
            </div>
          )}
        </div>

        <button type="submit" className="btn btn-primary" disabled={isLoading}>
          {isLoading ? 'Saving...' : 'Save Settings'}
        </button>
      </form>
    </div>
  );
}
