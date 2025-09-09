import { useEffect } from 'react';
import { useSpiderStore } from '../store/spider';

export default function Conversations() {
  const { conversations, loadConversations, loadConversation, isLoading } = useSpiderStore();

  useEffect(() => {
    loadConversations();
  }, [loadConversations]);

  const handleSelectConversation = async (id: string) => {
    await loadConversation(id);
    // Switch to Chat tab after loading conversation
    if ((window as any).switchToChat) {
      (window as any).switchToChat();
    }
  };

  return (
    <div className="component-container">
      <div className="component-header">
        <h2>Conversation History</h2>
        <button 
          className="btn btn-secondary"
          onClick={() => loadConversations()}
          disabled={isLoading}
        >
          Refresh
        </button>
      </div>

      <div className="component-content">
        <div className="conversations-list">
          {conversations.length === 0 ? (
            <p className="empty-state">No conversations yet</p>
          ) : (
          conversations.map((conv) => (
            <div 
              key={conv.id} 
              className="conversation-item"
              onClick={() => handleSelectConversation(conv.id)}
            >
              <div className="conversation-info">
                <h3>Conversation {conv.id.substring(0, 8)}...</h3>
                <p>Client: {conv.metadata.client}</p>
                <p>Started: {conv.metadata.start_time}</p>
                <p>Messages: {conv.messages.length}</p>
                <p>Provider: {conv.llm_provider}</p>
              </div>
            </div>
          ))
          )}
        </div>
      </div>
    </div>
  );
}