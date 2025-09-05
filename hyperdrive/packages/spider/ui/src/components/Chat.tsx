import React, { useState, useRef, useEffect } from 'react';
import { useSpiderStore } from '../store/spider';
import ReactMarkdown from 'react-markdown';
import { webSocketService } from '../services/websocket';

interface ToolCall {
  id: string;
  tool_name: string;
  parameters: string;
}

interface ToolResult {
  tool_call_id: string;
  result: string;
}

function ToolCallModal({ toolCall, toolResult, onClose }: {
  toolCall: ToolCall;
  toolResult?: ToolResult;
  onClose: () => void;
}) {
  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text).then(() => {
      // Could add a toast notification here
    }).catch(err => {
      console.error('Failed to copy:', err);
    });
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Tool Call Details: {toolCall.tool_name}</h3>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>
        <div className="modal-body">
          <div className="modal-section">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <h4>Tool Call</h4>
              <button 
                className="btn-icon copy-btn"
                onClick={() => copyToClipboard(JSON.stringify(toolCall, null, 2))}
                title="Copy to clipboard"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                </svg>
              </button>
            </div>
            <pre className="json-display">
              {JSON.stringify(toolCall, null, 2)}
            </pre>
          </div>
          {toolResult && (
            <div className="modal-section">
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <h4>Tool Result</h4>
                <button 
                  className="btn-icon copy-btn"
                  onClick={() => copyToClipboard(JSON.stringify(toolResult, null, 2))}
                  title="Copy to clipboard"
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                  </svg>
                </button>
              </div>
              <pre className="json-display">
                {JSON.stringify(toolResult, null, 2)}
              </pre>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default function Chat() {
  const {
    activeConversation,
    isLoading,
    error,
    sendMessage,
    clearActiveConversation,
    cancelRequest,
    wsConnected,
    useWebSocket
  } = useSpiderStore();
  const [message, setMessage] = useState('');
  const [selectedToolCall, setSelectedToolCall] = useState<{call: ToolCall, result?: ToolResult} | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);
  const messagesEndRef = useRef<HTMLDivElement | null>(null);
  const chatMessagesRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // Auto-scroll to bottom when new messages arrive
  const scrollToBottom = (smooth: boolean = true) => {
    if (messagesEndRef.current) {
      messagesEndRef.current.scrollIntoView({ behavior: smooth ? 'smooth' : 'auto', block: 'end' });
    }
  };

  // Scroll on new messages or loading state changes
  useEffect(() => {
    // Use a small delay to ensure DOM has updated
    const timer = setTimeout(() => {
      scrollToBottom();
    }, 100);
    return () => clearTimeout(timer);
  }, [activeConversation?.messages?.length, isLoading]);

  // Scroll immediately when conversation changes
  useEffect(() => {
    scrollToBottom(false);
  }, [activeConversation?.id]);

  // Auto-focus input when loading completes (assistant finishes responding)
  useEffect(() => {
    if (!isLoading && inputRef.current) {
      inputRef.current.focus();
    }
  }, [isLoading]);

  // Helper to get tool emoji - always use the same emoji
  const getToolEmoji = (toolName: string) => {
    return '🔧';
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!message.trim() || isLoading) return;

    const controller = new AbortController();
    abortControllerRef.current = controller;

    try {
      await sendMessage(message, controller.signal);
      setMessage('');
      // Scroll after sending message
      setTimeout(() => scrollToBottom(), 50);
    } catch (err: any) {
      if (err.name !== 'AbortError') {
        console.error('Failed to send message:', err);
      }
    } finally {
      abortControllerRef.current = null;
    }
  };

  const handleCancel = () => {
    // Cancel HTTP request if using HTTP
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
      abortControllerRef.current = null;
    }
    
    // Cancel WebSocket request if using WebSocket
    if (useWebSocket && wsConnected) {
      webSocketService.sendCancel();
    }
    
    // Update store state
    if (cancelRequest) {
      cancelRequest();
    }
  };

  const handleNewConversation = () => {
    clearActiveConversation();
  };

  return (
    <div className="chat-container">
      <div className="chat-header">
        <h2>Chat</h2>
        <div style={{ display: 'flex', alignItems: 'center', gap: '1rem' }}>
          {useWebSocket && (
            <span 
              className={`ws-status ${wsConnected ? 'ws-connected' : 'ws-disconnected'}`}
              title={wsConnected ? 'WebSocket connected - messages update in real-time' : 'WebSocket disconnected - using HTTP'}
            >
              {wsConnected ? '🟢' : '🔴'} {wsConnected ? 'Live' : 'HTTP'}
            </span>
          )}
          <button
            onClick={handleNewConversation}
            className="btn btn-icon new-conversation-btn"
            title="New Conversation"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M12 20h9"/>
              <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/>
              <path d="M15 5L19 9"/>
            </svg>
          </button>
        </div>
      </div>

      {error && (
        <div className="error-message">
          {error}
        </div>
      )}

      <div className="chat-messages" ref={chatMessagesRef}>
        {activeConversation?.messages.map((msg, index) => {
          const toolCalls = msg.toolCallsJson ? JSON.parse(msg.toolCallsJson) as ToolCall[] : null;
          const toolResults = msg.toolResultsJson ? JSON.parse(msg.toolResultsJson) as ToolResult[] : null;
          const nextMsg = activeConversation.messages[index + 1];
          const hasToolResult = nextMsg?.role === 'tool' && nextMsg.toolResultsJson;

          return (
            <React.Fragment key={index}>
              {msg.role !== 'tool' && (msg.content && msg.content.trim() && msg.content !== '[Tool calls pending]') && (
                <div className={`message message-${msg.role}`}>
                  <div className="message-content">
                    <ReactMarkdown>{msg.content}</ReactMarkdown>
                  </div>
                </div>
              )}

              {/* Display tool calls as separate message-like items */}
              {toolCalls && toolCalls.map((toolCall, toolIndex) => {
                // Find the corresponding tool result in the next message if it's a tool message
                const toolResultFromNext = nextMsg?.role === 'tool' && nextMsg.toolResultsJson
                  ? (JSON.parse(nextMsg.toolResultsJson) as ToolResult[])?.find(r => r.tool_call_id === toolCall.id)
                  : null;

                const isLastMessage = index === activeConversation.messages.length - 1;
                const isWaitingForResult = isLastMessage && isLoading && !toolResultFromNext;

                return (
                  <div key={`tool-${index}-${toolIndex}`} className="message message-tool">
                    <span className="tool-emoji">{getToolEmoji(toolCall.tool_name)}</span>
                    {isWaitingForResult ? (
                      <>
                        <span className="tool-name">{toolCall.tool_name}</span>
                        <span className="spinner tool-spinner"></span>
                      </>
                    ) : (
                      <button
                        className="tool-link"
                        onClick={() => setSelectedToolCall({ call: toolCall, result: toolResultFromNext })}
                      >
                        {toolCall.tool_name}
                      </button>
                    )}
                  </div>
                );
              })}
            </React.Fragment>
          );
        }) || (
          <div className="empty-chat">
            <p>Start a conversation by typing a message below</p>
          </div>
        )}
        {isLoading && activeConversation && (
          <div className="message message-assistant message-thinking">
            <div className="message-content">
              <div className="thinking-indicator">
                <span className="spinner"></span>
                <span>Thinking...</span>
              </div>
            </div>
          </div>
        )}
        <div ref={messagesEndRef} style={{ float: 'left', clear: 'both' }} />
      </div>

      {selectedToolCall && (
        <ToolCallModal
          toolCall={selectedToolCall.call}
          toolResult={selectedToolCall.result}
          onClose={() => setSelectedToolCall(null)}
        />
      )}

      <form onSubmit={handleSubmit} className="chat-input-form">
        <input
          ref={inputRef}
          type="text"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder={isLoading ? "Thinking..." : "Type your message..."}
          disabled={isLoading}
          className={`chat-input ${isLoading ? 'chat-input-thinking' : ''}`}
        />
        {isLoading ? (
          <button
            type="button"
            onClick={handleCancel}
            className="btn btn-danger"
          >
            Cancel
          </button>
        ) : (
          <button
            type="submit"
            disabled={!message.trim()}
            className="btn btn-primary"
          >
            Send
          </button>
        )}
      </form>
    </div>
  );
}
