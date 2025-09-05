import { useState, useEffect } from 'react';
import { useSpiderStore } from '../store/spider';

export default function Settings() {
  const { config, isLoading, error, updateConfig } = useSpiderStore();
  const [provider, setProvider] = useState(config.defaultLlmProvider);
  const [model, setModel] = useState('');
  const [maxTokens, setMaxTokens] = useState(config.maxTokens);
  const [temperature, setTemperature] = useState(config.temperature);

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
  }, [config]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    await updateConfig({
      defaultLlmProvider: provider,
      maxTokens: maxTokens,
      temperature: temperature,
    });
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
        
        <button type="submit" className="btn btn-primary" disabled={isLoading}>
          {isLoading ? 'Saving...' : 'Save Settings'}
        </button>
      </form>
    </div>
  );
}