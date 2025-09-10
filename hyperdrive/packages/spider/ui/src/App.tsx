import { useState, useEffect } from 'react';
import './App.css';
import { useSpiderStore } from './store/spider';
import ApiKeys from './components/ApiKeys';
import SpiderKeys from './components/SpiderKeys';
import McpServers from './components/McpServers';
import Conversations from './components/Conversations';
import Chat from './components/Chat';
import Settings from './components/Settings';

export type TabType = 'chat' | 'api-keys' | 'spider-keys' | 'mcp-servers' | 'conversations' | 'settings';

function App() {
  const { initialize, clearError, apiKeys } = useSpiderStore();
  const [activeTab, setActiveTab] = useState<TabType>('api-keys');

  useEffect(() => {
    initialize().then(() => {
      // After initialization, check if we should switch to Chat tab if API keys exist
      const store = useSpiderStore.getState();
      if (store.apiKeys.length > 0) {
        setActiveTab('chat');
      }
      // Otherwise stay on API Keys tab
    });
  }, [initialize]);

  // Clear error when switching tabs
  const handleTabChange = (tab: TabType) => {
    clearError();
    setActiveTab(tab);
  };

  // Export for Conversations component to use
  useEffect(() => {
    (window as any).switchToChat = () => setActiveTab('chat');
  }, []);

  return (
    <div className="app">
      <header className="app-header">
        <span className="app-icon" title="Spider MCP Client">🕷️</span>
        <nav className="app-nav">
          <button 
            className={`nav-btn ${activeTab === 'chat' ? 'active' : ''}`}
            onClick={() => handleTabChange('chat')}
            title="Chat"
          >
            <span className="nav-btn-text">Chat</span>
            <span className="nav-btn-icon">💬</span>
          </button>
          <button 
            className={`nav-btn ${activeTab === 'api-keys' ? 'active' : ''}`}
            onClick={() => handleTabChange('api-keys')}
            title="API Keys"
          >
            <span className="nav-btn-text">API Keys</span>
            <span className="nav-btn-icon">🔑</span>
          </button>
          <button 
            className={`nav-btn ${activeTab === 'spider-keys' ? 'active' : ''}`}
            onClick={() => handleTabChange('spider-keys')}
            title="Spider Keys"
          >
            <span className="nav-btn-text">Spider</span>
            <span className="nav-btn-icon">🕸️</span>
          </button>
          <button 
            className={`nav-btn ${activeTab === 'mcp-servers' ? 'active' : ''}`}
            onClick={() => handleTabChange('mcp-servers')}
            title="MCP Servers"
          >
            <span className="nav-btn-text">MCP</span>
            <span className="nav-btn-icon">🖥️</span>
          </button>
          <button 
            className={`nav-btn ${activeTab === 'conversations' ? 'active' : ''}`}
            onClick={() => handleTabChange('conversations')}
            title="History"
          >
            <span className="nav-btn-text">History</span>
            <span className="nav-btn-icon">📜</span>
          </button>
          <button 
            className={`nav-btn ${activeTab === 'settings' ? 'active' : ''}`}
            onClick={() => handleTabChange('settings')}
            title="Settings"
          >
            <span className="nav-btn-text">Settings</span>
            <span className="nav-btn-icon">⚙️</span>
          </button>
        </nav>
      </header>
      
      <main className={`app-main ${activeTab === 'chat' ? 'chat-view' : ''}`}>
        {activeTab === 'chat' && <Chat />}
        {activeTab === 'api-keys' && <ApiKeys />}
        {activeTab === 'spider-keys' && <SpiderKeys />}
        {activeTab === 'mcp-servers' && <McpServers />}
        {activeTab === 'conversations' && <Conversations />}
        {activeTab === 'settings' && <Settings />}
      </main>
    </div>
  );
}

export default App;