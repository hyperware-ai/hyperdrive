import { useState } from 'react';
import { useSpiderStore } from '../store/spider';
import { ClaudeLogin } from './ClaudeLogin';

export default function ApiKeys() {
  const { apiKeys, isLoading, error, setApiKey, removeApiKey } = useSpiderStore();
  const [showAddForm, setShowAddForm] = useState(false);
  const [showClaudeLogin, setShowClaudeLogin] = useState(false);
  const [provider, setProvider] = useState('anthropic');
  const [apiKeyValue, setApiKeyValue] = useState('');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!apiKeyValue.trim()) return;
    
    await setApiKey(provider, apiKeyValue);
    setApiKeyValue('');
    setShowAddForm(false);
  };

  return (
    <div className="component-container">
      <div className="component-header">
        <h2>API Keys</h2>
        <div className="flex gap-2">
          <button 
            className="btn btn-primary"
            onClick={() => {
              setShowAddForm(!showAddForm);
              setShowClaudeLogin(false);
            }}
          >
            {showAddForm ? 'Cancel' : 'Add API Key'}
          </button>
          <button 
            className="btn btn-secondary"
            onClick={() => {
              setShowClaudeLogin(!showClaudeLogin);
              setShowAddForm(false);
            }}
          >
            {showClaudeLogin ? 'Cancel' : 'Login with Claude'}
          </button>
        </div>
      </div>

      {error && (
        <div className="error-message">
          {error}
        </div>
      )}

      {showClaudeLogin && (
        <div className="claude-login-container">
          <ClaudeLogin 
            onSuccess={() => {
              setShowClaudeLogin(false);
              // Refresh API keys list
              useSpiderStore.getState().loadApiKeys();
            }}
            onCancel={() => setShowClaudeLogin(false)}
          />
        </div>
      )}

      {showAddForm && (
        <form onSubmit={handleSubmit} className="api-key-form">
          <div className="form-group">
            <label htmlFor="provider">Provider</label>
            <select
              id="provider"
              value={provider}
              onChange={(e) => setProvider(e.target.value)}
            >
              <option value="anthropic">Anthropic</option>
              <option value="openai">OpenAI</option>
              <option value="google">Google</option>
            </select>
          </div>
          
          <div className="form-group">
            <label htmlFor="api-key">API Key</label>
            <input
              id="api-key"
              type="password"
              value={apiKeyValue}
              onChange={(e) => setApiKeyValue(e.target.value)}
              placeholder="sk-..."
              required
            />
          </div>
          
          <button type="submit" className="btn btn-primary" disabled={isLoading}>
            {isLoading ? 'Adding...' : 'Add Key'}
          </button>
        </form>
      )}

      <div className="component-content">
        <div className="api-keys-list">
          {apiKeys.length === 0 ? (
            <p className="empty-state">No API keys configured</p>
          ) : (
          apiKeys.map((key) => (
            <div key={key.provider} className="api-key-item">
              <div className="api-key-info">
                <h3>{key.provider}</h3>
                <p>Key: {key.keyPreview}</p>
                <p>Created: {new Date(key.createdAt * 1000).toLocaleDateString()}</p>
                {key.lastUsed && (
                  <p>Last used: {new Date(key.lastUsed * 1000).toLocaleDateString()}</p>
                )}
              </div>
              <button
                className="btn btn-danger"
                onClick={() => removeApiKey(key.provider)}
                disabled={isLoading}
              >
                Remove
              </button>
            </div>
          ))
          )}
        </div>
      </div>
    </div>
  );
}