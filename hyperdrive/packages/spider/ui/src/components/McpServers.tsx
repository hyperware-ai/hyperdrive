import { useState, useEffect } from 'react';
import { useSpiderStore } from '../store/spider';

export default function McpServers() {
  const { 
    mcpServers, 
    isLoading, 
    error, 
    addMcpServer, 
    connectMcpServer, 
    disconnectMcpServer,
    removeMcpServer,
    loadMcpServers 
  } = useSpiderStore();
  const [showAddForm, setShowAddForm] = useState(false);
  const [serverName, setServerName] = useState('');
  const [transportType, setTransportType] = useState('websocket');
  const [url, setUrl] = useState('ws://localhost:10125');
  const [hypergridUrl, setHypergridUrl] = useState('http://localhost:8080/operator:hypergrid:ware.hypr/shim/mcp');
  const [hypergridToken, setHypergridToken] = useState('');
  const [hypergridClientId, setHypergridClientId] = useState('');
  const [hypergridNode, setHypergridNode] = useState('');
  const [connectingServers, setConnectingServers] = useState<Set<string>>(new Set());

  // Periodically refresh server status
  useEffect(() => {
    const interval = setInterval(() => {
      loadMcpServers();
    }, 5000); // Refresh every 5 seconds

    return () => clearInterval(interval);
  }, [loadMcpServers]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    let transport: any = {
      transportType: transportType,
      command: null,
      args: null,
      url: null,
      hypergridToken: null,
      hypergridClientId: null,
      hypergridNode: null
    };
    
    if (transportType === 'websocket') {
      transport.url = url;
    } else if (transportType === 'hypergrid') {
      transport.url = hypergridUrl;
      transport.hypergridToken = hypergridToken;
      transport.hypergridClientId = hypergridClientId;
      transport.hypergridNode = hypergridNode;
    }
    
    await addMcpServer(serverName, transport);
    setServerName('');
    setTransportType('websocket');
    setUrl('ws://localhost:10125');
    setHypergridUrl('http://localhost:8080/operator:hypergrid:ware.hypr/shim/mcp');
    setHypergridToken('');
    setHypergridClientId('');
    setHypergridNode('');
    setShowAddForm(false);
    
    // Refresh servers list after adding
    setTimeout(() => loadMcpServers(), 500);
  };

  const handleConnect = async (serverId: string) => {
    setConnectingServers(prev => new Set(prev).add(serverId));
    try {
      await connectMcpServer(serverId);
      // Poll for connection status update
      let attempts = 0;
      const pollInterval = setInterval(async () => {
        await loadMcpServers();
        attempts++;
        const server = mcpServers.find(s => s.id === serverId);
        if (server?.connected || attempts > 10) {
          clearInterval(pollInterval);
          setConnectingServers(prev => {
            const next = new Set(prev);
            next.delete(serverId);
            return next;
          });
        }
      }, 500);
    } catch (error) {
      setConnectingServers(prev => {
        const next = new Set(prev);
        next.delete(serverId);
        return next;
      });
    }
  };

  const handleDisconnect = async (serverId: string) => {
    await disconnectMcpServer(serverId);
    // Refresh servers list after disconnecting
    setTimeout(() => loadMcpServers(), 500);
  };

  const handleRemove = async (serverId: string) => {
    if (confirm('Are you sure you want to remove this MCP server?')) {
      await removeMcpServer(serverId);
      // Refresh servers list after removing
      setTimeout(() => loadMcpServers(), 500);
    }
  };

  return (
    <div className="component-container">
      <div className="component-header">
        <h2>MCP Servers</h2>
        <button 
          className="btn btn-primary"
          onClick={() => setShowAddForm(!showAddForm)}
        >
          {showAddForm ? 'Cancel' : 'Add MCP Server'}
        </button>
      </div>

      {error && (
        <div className="error-message">
          {error}
        </div>
      )}

      {showAddForm && (
        <form onSubmit={handleSubmit} className="mcp-server-form">
          <div className="form-group">
            <label htmlFor="server-name">Server Name</label>
            <input
              id="server-name"
              type="text"
              value={serverName}
              onChange={(e) => setServerName(e.target.value)}
              placeholder="My MCP Server"
              required
            />
          </div>
          
          <div className="form-group">
            <label htmlFor="transport-type">Transport Type</label>
            <select
              id="transport-type"
              value={transportType}
              onChange={(e) => setTransportType(e.target.value)}
              className="form-select"
            >
              <option value="websocket">WebSocket</option>
              <option value="hypergrid">Hypergrid</option>
            </select>
          </div>
          
          {transportType === 'websocket' && (
            <div className="form-group">
              <label htmlFor="url">WebSocket URL</label>
              <input
                id="url"
                type="text"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="ws://localhost:10125"
                required
              />
              <small className="form-help">
                URL of the WebSocket MCP server or ws-mcp wrapper
              </small>
            </div>
          )}
          
          {transportType === 'hypergrid' && (
            <>
              <div className="form-group">
                <label htmlFor="hypergrid-url">Hypergrid API URL</label>
                <input
                  id="hypergrid-url"
                  type="text"
                  value={hypergridUrl}
                  onChange={(e) => setHypergridUrl(e.target.value)}
                  placeholder="http://localhost:8080/operator:hypergrid:ware.hypr/shim/mcp"
                  required
                />
                <small className="form-help">
                  Base URL for the Hypergrid API endpoint
                </small>
              </div>
              
              <div className="form-group">
                <label htmlFor="hypergrid-token">Authentication Token</label>
                <input
                  id="hypergrid-token"
                  type="text"
                  value={hypergridToken}
                  onChange={(e) => setHypergridToken(e.target.value)}
                  placeholder="Enter your hypergrid token (optional for initial connection)"
                />
                <small className="form-help">
                  Token for authenticating with the Hypergrid network
                </small>
              </div>
              
              <div className="form-group">
                <label htmlFor="hypergrid-client-id">Client ID</label>
                <input
                  id="hypergrid-client-id"
                  type="text"
                  value={hypergridClientId}
                  onChange={(e) => setHypergridClientId(e.target.value)}
                  placeholder="Enter your client ID"
                  required
                />
                <small className="form-help">
                  Unique identifier for this client
                </small>
              </div>
              
              <div className="form-group">
                <label htmlFor="hypergrid-node">Node Name</label>
                <input
                  id="hypergrid-node"
                  type="text"
                  value={hypergridNode}
                  onChange={(e) => setHypergridNode(e.target.value)}
                  placeholder="Enter your Hyperware node name"
                  required
                />
                <small className="form-help">
                  Name of your Hyperware node
                </small>
              </div>
            </>
          )}
          
          <button type="submit" className="btn btn-primary" disabled={isLoading}>
            {isLoading ? 'Adding...' : 'Add Server'}
          </button>
        </form>
      )}

      <div className="component-content">
        <div className="mcp-servers-list">
          {mcpServers.length === 0 ? (
            <p className="empty-state">No MCP servers configured</p>
          ) : (
          mcpServers.map((server) => {
            const isConnecting = connectingServers.has(server.id);
            return (
              <div key={server.id} className="mcp-server-item">
                <div className="mcp-server-info">
                  <h3>{server.name}</h3>
                  <p>
                    Status: {
                      isConnecting ? '🟡 Connecting...' :
                      server.connected ? '🟢 Connected' : 
                      '🔴 Disconnected'
                    }
                  </p>
                  <p>
                    Transport: {server.transport.transportType === 'hypergrid' ? 
                      `Hypergrid - ${server.transport.hypergridNode || 'Not configured'}` :
                      `WebSocket - ${server.transport.url || 'No URL specified'}`
                    }
                  </p>
                  <p>Tools: {server.tools.length}</p>
                  {server.tools.length > 0 && (
                    <details className="mcp-server-tools">
                      <summary>Available Tools</summary>
                      <ul>
                        {server.tools.map((tool, index) => (
                          <li key={index}>
                            <strong>{tool.name}</strong>: {tool.description}
                            {tool.inputSchemaJson && <span style={{ marginLeft: '8px', color: '#667eea', fontSize: '0.8em' }}>(✓ Schema)</span>}
                          </li>
                        ))}
                      </ul>
                    </details>
                  )}
                </div>
                <div className="mcp-server-actions">
                  {!server.connected && !isConnecting && (
                    <button
                      className="btn btn-success"
                      onClick={() => handleConnect(server.id)}
                      disabled={isLoading}
                    >
                      Connect
                    </button>
                  )}
                  {server.connected && (
                    <button
                      className="btn btn-warning"
                      onClick={() => handleDisconnect(server.id)}
                      disabled={isLoading}
                    >
                      Disconnect
                    </button>
                  )}
                  <button
                    className="btn btn-danger btn-icon"
                    onClick={() => handleRemove(server.id)}
                    disabled={isLoading}
                    title="Remove server"
                  >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <line x1="18" y1="6" x2="6" y2="18"></line>
                      <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                  </button>
                </div>
              </div>
            );
          })
          )}
        </div>
      </div>
    </div>
  );
}