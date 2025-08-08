import { useState, useEffect } from 'react'
import { callApi } from './utils/api'
import './App.css'

interface ServiceStatus {
  service: string
  version: string
  status: string
  wallets_count: number
  permissions_count: number
  chains_count: number
  initialized_at: number
  timestamp: string
}

function App() {
  const [status, setStatus] = useState<ServiceStatus | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    fetchStatus()
  }, [])

  const fetchStatus = async () => {
    try {
      setLoading(true)
      setError(null)
      const response = await callApi('status', {})
      setStatus(response)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch status')
    } finally {
      setLoading(false)
    }
  }

  const testPermissions = async () => {
    try {
      setError(null)
      const response = await callApi('api', {
        operation: 'ListWallets',
        params: {},
        timestamp: Date.now() / 1000
      })
      console.log('ListWallets response:', response)
      alert('Check console for ListWallets response')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to test permissions')
    }
  }

  const testCreateWallet = async () => {
    try {
      setError(null)
      const response = await callApi('api', {
        operation: 'CreateWallet',
        params: {
          name: 'Test Wallet',
          chain_id: 8453
        },
        timestamp: Date.now() / 1000
      })
      console.log('CreateWallet response:', response)
      alert('Check console for CreateWallet response')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create wallet')
    }
  }

  if (loading) {
    return (
      <div className="container">
        <div className="loading">Loading Hyperwallet Service...</div>
      </div>
    )
  }

  return (
    <div className="container">
      <h1>🔐 Hyperwallet Service</h1>
      
      {error && (
        <div className="error">
          <strong>Error:</strong> {error}
        </div>
      )}

      {status && (
        <div className="card">
          <h2>Service Status</h2>
          <div className="status-grid">
            <div className="status-item">
              <h3>Service</h3>
              <p>{status.service}</p>
            </div>
            <div className="status-item">
              <h3>Version</h3>
              <p>{status.version}</p>
            </div>
            <div className="status-item">
              <h3>Status</h3>
              <p style={{ color: status.status === 'running' ? '#4CAF50' : '#FF6B6B' }}>
                {status.status}
              </p>
            </div>
            <div className="status-item">
              <h3>Wallets</h3>
              <p>{status.wallets_count}</p>
            </div>
            <div className="status-item">
              <h3>Permissions</h3>
              <p>{status.permissions_count}</p>
            </div>
            <div className="status-item">
              <h3>Chains</h3>
              <p>{status.chains_count}</p>
            </div>
          </div>
          
          <p><strong>Initialized:</strong> {new Date(status.initialized_at * 1000).toLocaleString()}</p>
          <p><strong>Last Updated:</strong> {new Date(status.timestamp).toLocaleString()}</p>
        </div>
      )}

      <div className="card">
        <h2>Test Operations</h2>
        <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
          <button onClick={fetchStatus}>
            Refresh Status
          </button>
          <button onClick={testPermissions}>
            Test List Wallets
          </button>
          <button onClick={testCreateWallet}>
            Test Create Wallet
          </button>
        </div>
      </div>

      <div className="card">
        <h2>About Hyperwallet</h2>
        <p>
          Hyperwallet is a system-level wallet service for Hyperware that provides secure, 
          permission-based wallet operations for all applications.
        </p>
        <h3>Features:</h3>
        <ul>
          <li>🔐 Secure wallet management with encryption</li>
          <li>🎛️ Fine-grained permission system</li>
          <li>⛓️ Multi-chain support (Ethereum, Base, etc.)</li>
          <li>🗺️ Hypermap integration for identity management</li>
          <li>🎯 Token Bound Account (TBA) operations</li>
          <li>🔄 Inter-process communication API</li>
        </ul>
        
        <h3>Permission Levels:</h3>
        <ul>
          <li><strong>Read:</strong> Query balances, transaction history</li>
          <li><strong>Transact:</strong> Send transactions (includes Read)</li>
          <li><strong>Manage:</strong> Create/import wallets (includes Transact)</li>
          <li><strong>Admin:</strong> Full control including wallet deletion</li>
        </ul>
      </div>
    </div>
  )
}

export default App