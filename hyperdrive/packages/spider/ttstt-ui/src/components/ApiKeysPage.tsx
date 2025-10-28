import React, { useState, useEffect } from 'react';
import useTtsttStore from '../store/ttstt';
import { Ttstt } from '../../../target/ui/caller-utils';
const ApiKeyRole = Ttstt.ApiKeyRole;

function ApiKeysPage() {
  const {
    apiKeys,
    loadApiKeys,
    generateApiKey,
    isLoading,
  } = useTtsttStore();

  // Load API keys on mount
  useEffect(() => {
    loadApiKeys();
  }, []);

  // Form state
  const [showGenerateForm, setShowGenerateForm] = useState(false);
  const [newKeyName, setNewKeyName] = useState('');
  const [newKeyRole, setNewKeyRole] = useState<Ttstt.ApiKeyRole>(ApiKeyRole.Requestor);
  const [generatedKey, setGeneratedKey] = useState<string | null>(null);

  const handleGenerateKey = async (e: React.FormEvent) => {
    e.preventDefault();
    if (newKeyName.trim()) {
      try {
        const key = await generateApiKey(newKeyName, newKeyRole);
        setGeneratedKey(key);
        setNewKeyName('');
        setShowGenerateForm(false);
      } catch (error) {
        console.error('Failed to generate key:', error);
      }
    }
  };

  const handleRevokeKey = async () => {
    // Note: In a real app, you'd need the full key, not just the preview
    // This is a limitation of the current implementation
    if (window.confirm(`Revoke this API key?`)) {
      // For now, we can't revoke without the full key
      alert('Revocation requires the full API key');
    }
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    alert('Copied to clipboard!');
  };

  return (
    <div className="api-keys-page">
      <h2>API Key Management</h2>

      {generatedKey && (
        <div className="success">
          <h3>New API Key Generated!</h3>
          <p className="text-muted mb-3">Save this key now - it won't be shown again:</p>
          <div className="flex items-center gap-2 mb-3">
            <code style={{ flex: 1, padding: '0.5rem', background: 'var(--surface)', borderRadius: '4px', wordBreak: 'break-all' }}>
              {generatedKey}
            </code>
            <button onClick={() => copyToClipboard(generatedKey)} className="primary">
              Copy
            </button>
          </div>
          <button onClick={() => setGeneratedKey(null)}>Close</button>
        </div>
      )}

      <div className="card">
        <h3>Existing API Keys</h3>
        {apiKeys.length === 0 ? (
          <p className="text-muted">No API keys generated yet.</p>
        ) : (
          <div style={{ overflowX: 'auto' }}>
            <table style={{ width: '100%', borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ borderBottom: '2px solid var(--border-color)' }}>
                  <th style={{ padding: '0.75rem', textAlign: 'left', color: 'var(--text-primary)' }}>Name</th>
                  <th style={{ padding: '0.75rem', textAlign: 'left', color: 'var(--text-primary)' }}>Role</th>
                  <th style={{ padding: '0.75rem', textAlign: 'left', color: 'var(--text-primary)' }}>Created</th>
                  <th style={{ padding: '0.75rem', textAlign: 'left', color: 'var(--text-primary)' }}>Key Preview</th>
                  <th style={{ padding: '0.75rem', textAlign: 'right', color: 'var(--text-primary)' }}>Actions</th>
                </tr>
              </thead>
              <tbody>
                {apiKeys.map((key, index) => (
                  <tr key={index} style={{ borderBottom: '1px solid var(--border-color)' }}>
                    <td style={{ padding: '0.75rem', color: 'var(--text-primary)' }}>{key.name}</td>
                    <td style={{ padding: '0.75rem' }}>
                      <span style={{
                        padding: '0.25rem 0.5rem',
                        background: key.role === ApiKeyRole.Admin ? 'var(--warning-color)' : 'var(--primary-color)',
                        color: 'white',
                        borderRadius: '4px',
                        fontSize: '0.75rem'
                      }}>
                        {key.role}
                      </span>
                    </td>
                    <td style={{ padding: '0.75rem', color: 'var(--text-muted)', fontSize: '0.875rem' }}>
                      {new Date(key.created_at).toLocaleDateString()}
                    </td>
                    <td style={{ padding: '0.75rem' }}>
                      <code style={{ fontSize: '0.875rem' }}>{key.key_preview}</code>
                    </td>
                    <td style={{ padding: '0.75rem', textAlign: 'right' }}>
                      <button
                        onClick={() => handleRevokeKey()}
                        disabled={isLoading}
                        className="danger"
                        style={{ fontSize: '0.875rem' }}
                      >
                        Revoke
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <div className="mt-4">
        <button
          onClick={() => setShowGenerateForm(!showGenerateForm)}
          disabled={isLoading}
          className="primary"
        >
          {showGenerateForm ? 'Cancel' : '+ Generate New Key'}
        </button>

        {showGenerateForm && (
          <form onSubmit={handleGenerateKey} className="card mt-3">
            <h3>Generate New API Key</h3>
            <div className="form-group">
              <label>Key Name</label>
              <input
                type="text"
                value={newKeyName}
                onChange={(e) => setNewKeyName(e.target.value)}
                placeholder="e.g., Production App, Development"
                required
                disabled={isLoading}
              />
            </div>

            <div className="form-group">
              <label>Role</label>
              <select
                value={newKeyRole}
                onChange={(e) => setNewKeyRole(e.target.value as Ttstt.ApiKeyRole)}
                disabled={isLoading}
              >
                <option value={ApiKeyRole.Requestor}>Requestor (Can use TTS/STT)</option>
                <option value={ApiKeyRole.Admin}>Admin (Full access)</option>
              </select>
            </div>

            <button type="submit" disabled={isLoading || !newKeyName.trim()} className="primary">
              Generate Key
            </button>
          </form>
        )}
      </div>

      <div className="warning mt-4">
        <h4>\u26a0\ufe0f Important Notes</h4>
        <ul className="text-small" style={{ paddingLeft: '1.5rem', marginTop: '0.5rem' }}>
          <li>API keys are shown only once when generated. Save them securely.</li>
          <li>Admin keys can manage providers and generate/revoke other keys.</li>
          <li>Requestor keys can only access TTS and STT endpoints.</li>
          <li>Revoking a key requires the full key (not just the preview).</li>
        </ul>
      </div>
    </div>
  );
}

export default ApiKeysPage;
