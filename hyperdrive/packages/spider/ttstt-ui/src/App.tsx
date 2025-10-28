import React, { useState, useEffect } from 'react';
import useTtsttStore from './store/ttstt';
import TestPage from './components/TestPage';
import SettingsPage from './components/SettingsPage';
import ApiKeysPage from './components/ApiKeysPage';

type Page = 'test' | 'settings' | 'keys';

function App() {
  const [activePage, setActivePage] = useState<Page>('test');
  const { getAdminKey, adminKey, providers, loadProviders, error, clearError } = useTtsttStore();
  const [hasCheckedProviders, setHasCheckedProviders] = useState(false);

  useEffect(() => {
    // Try to get admin key and providers on initial load
    getAdminKey();
    loadProviders();
  }, []);

  useEffect(() => {
    // Auto-navigate to settings only if no providers configured
    if (!hasCheckedProviders && providers !== null) {
      setHasCheckedProviders(true);
      if (providers.length === 0) {
        setActivePage('settings');
      } else {
        // Ensure Test tab is active when providers exist
        setActivePage('test');
      }
    }
  }, [providers, hasCheckedProviders]);

  // Admin key prompt
  const [keyInput, setKeyInput] = useState('');

  const handleKeySubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (keyInput.trim()) {
      useTtsttStore.getState().setAdminKey(keyInput.trim());
      setKeyInput('');
    }
  };

  return (
    <div className="app">
      <header className="app-header">
        <h1>🎙️ TTSTT</h1>
        <p className="text-muted">Text-to-Speech & Speech-to-Text Wrapper</p>
        <nav className="tabs">
          <button
            className={`tab-button ${activePage === 'test' ? 'active' : ''}`}
            onClick={() => setActivePage('test')}
          >
            Test
          </button>
          <button
            className={`tab-button ${activePage === 'settings' ? 'active' : ''}`}
            onClick={() => setActivePage('settings')}
          >
            Settings
          </button>
          <button
            className={`tab-button ${activePage === 'keys' ? 'active' : ''}`}
            onClick={() => setActivePage('keys')}
          >
            API Keys
          </button>
        </nav>
      </header>

      {error && (
        <div className="error">
          <div className="flex justify-between items-center">
            <span>{error}</span>
            <button onClick={clearError} style={{ padding: '0.25rem 0.5rem' }}>×</button>
          </div>
        </div>
      )}

      {!adminKey && activePage !== 'test' && (
        <div className="card">
          <h2>Admin Key Required</h2>
          <p className="text-muted mb-3">Enter your admin key to access settings and API key management:</p>
          <form onSubmit={handleKeySubmit} className="form-group">
            <input
              type="password"
              value={keyInput}
              onChange={(e) => setKeyInput(e.target.value)}
              placeholder="Enter admin key"
              autoFocus
            />
            <button type="submit" className="primary mt-3">Submit</button>
          </form>
        </div>
      )}

      <main className="app-main">
        {activePage === 'test' && (
          providers && providers.length === 0 ? (
            <div className="warning">
              <h3>No Providers Configured</h3>
              <p>Please configure at least one provider in the Settings tab before testing TTS/STT functionality.</p>
            </div>
          ) : (
            <TestPage />
          )
        )}
        {activePage === 'settings' && adminKey && <SettingsPage />}
        {activePage === 'keys' && adminKey && <ApiKeysPage />}
      </main>
    </div>
  );
}

export default App;