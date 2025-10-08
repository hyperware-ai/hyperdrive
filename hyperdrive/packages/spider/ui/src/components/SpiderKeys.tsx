import { useState } from 'react';
import { useSpiderStore } from '../store/spider';

export default function SpiderKeys() {
  const { spiderKeys, isLoading, error, createSpiderKey, revokeSpiderKey } = useSpiderStore();
  const [showAddForm, setShowAddForm] = useState(false);
  const [keyName, setKeyName] = useState('');
  const [permissions, setPermissions] = useState<string[]>(['read']);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!keyName.trim()) return;
    
    await createSpiderKey(keyName, permissions);
    setKeyName('');
    setPermissions(['read']);
    setShowAddForm(false);
  };

  const togglePermission = (perm: string) => {
    setPermissions(prev => 
      prev.includes(perm) 
        ? prev.filter(p => p !== perm)
        : [...prev, perm]
    );
  };

  return (
    <div className="component-container">
      <div className="component-header">
        <h2>Spider API Keys</h2>
        <button 
          className="btn btn-primary"
          onClick={() => setShowAddForm(!showAddForm)}
        >
          {showAddForm ? 'Cancel' : 'Generate New Key'}
        </button>
      </div>

      {error && (
        <div className="error-message">
          {error}
        </div>
      )}

      {showAddForm && (
        <form onSubmit={handleSubmit} className="spider-key-form">
          <div className="form-group">
            <label htmlFor="key-name">Key Name</label>
            <input
              id="key-name"
              type="text"
              value={keyName}
              onChange={(e) => setKeyName(e.target.value)}
              placeholder="My API Key"
              required
            />
          </div>
          
          <div className="form-group">
            <label>Permissions</label>
            <div className="permissions-grid">
              {['read', 'write', 'chat', 'admin'].map(perm => (
                <label key={perm} className="checkbox-label">
                  <input
                    type="checkbox"
                    checked={permissions.includes(perm)}
                    onChange={() => togglePermission(perm)}
                  />
                  {perm}
                </label>
              ))}
            </div>
          </div>
          
          <button type="submit" className="btn btn-primary" disabled={isLoading}>
            {isLoading ? 'Generating...' : 'Generate Key'}
          </button>
        </form>
      )}

      <div className="component-content">
        <div className="spider-keys-list">
          {spiderKeys.length === 0 ? (
            <p className="empty-state">No Spider API keys generated</p>
          ) : (
          spiderKeys.map((key) => (
            <div key={key.key} className="spider-key-item">
              <div className="spider-key-info">
                <h3>{key.name}</h3>
                <p className="key-value">Key: {key.key}</p>
                <p>Permissions: {key.permissions.join(', ')}</p>
                <p>Created: {new Date(key.createdAt * 1000).toLocaleDateString()}</p>
              </div>
              <button
                className="btn btn-danger"
                onClick={() => revokeSpiderKey(key.key)}
                disabled={isLoading}
              >
                Revoke
              </button>
            </div>
          ))
          )}
        </div>
      </div>
    </div>
  );
}