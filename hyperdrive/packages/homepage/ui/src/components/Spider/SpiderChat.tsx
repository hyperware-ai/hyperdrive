import React, { useEffect, useRef, useState, useCallback, useMemo } from 'react';
import { useSpiderStore } from '../../store/spider';
import { SpiderMessage } from '../../types/spider';
import SpiderMessageMenu from './SpiderMessageMenu';
import './SpiderChat.css';

interface SpiderChatProps {
  onBack: () => void;
}

const SpiderChat: React.FC<SpiderChatProps> = ({ onBack }) => {
  const {
    messages,
    isConnected,
    isLoading,
    error,
    streamingMessage,
    replyingTo,
    pendingReplyToId,
    isHistoryLoaded,
    connect,
    sendMessage,
    cancelRequest,
    clearMessages,
    setReplyingTo,
    loadHistory,
  } = useSpiderStore();

  const [inputValue, setInputValue] = useState('');
  const [isClosing, setIsClosing] = useState(false);
  const [menuMessage, setMenuMessage] = useState<SpiderMessage | null>(null);
  const [menuPosition, setMenuPosition] = useState({ x: 0, y: 0 });
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const startXRef = useRef(0);
  const startYRef = useRef(0);
  const [swipeX, setSwipeX] = useState(0);
  const [isSwiping, setIsSwiping] = useState(false);
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const touchStartRef = useRef<{ x: number; y: number } | null>(null);

  // Connect on mount
  useEffect(() => {
    if (!isConnected) {
      connect().catch(console.error);
    }
  }, [connect, isConnected]);

  useEffect(() => {
    if (isConnected && !isHistoryLoaded) {
      loadHistory().catch(console.error);
    }
  }, [isConnected, isHistoryLoaded, loadHistory]);

  // Scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, streamingMessage]);

  useEffect(() => {
    if (replyingTo) {
      inputRef.current?.focus();
    }
  }, [replyingTo]);

  useEffect(() => {
    return () => {
      if (longPressTimerRef.current) {
        clearTimeout(longPressTimerRef.current);
      }
    };
  }, []);

  const handleSubmit = useCallback(
    async (e?: React.FormEvent) => {
      e?.preventDefault();
      const trimmed = inputValue.trim();
      if (!trimmed || isLoading) return;

      setInputValue('');
      try {
        const replyToId = replyingTo?.id ?? null;
        setReplyingTo(null);
        await sendMessage(trimmed, replyToId);
      } catch (err) {
        console.error('Failed to send message:', err);
      }
    },
    [inputValue, isLoading, sendMessage]
  );

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  // Handle back button with animation
  const handleBack = useCallback(() => {
    setIsClosing(true);
    setTimeout(() => {
      onBack();
    }, 300);
  }, [onBack]);

  // Swipe handlers for navigation
  const handleTouchStart = (e: React.TouchEvent) => {
    if (e.touches[0].clientX < 30) {
      startXRef.current = e.touches[0].clientX;
      startYRef.current = e.touches[0].clientY;
      setIsSwiping(false);
    }
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    if (startXRef.current === 0) return;

    const currentX = e.touches[0].clientX;
    const currentY = e.touches[0].clientY;
    const deltaX = currentX - startXRef.current;
    const deltaY = Math.abs(currentY - startYRef.current);

    if (deltaX > 10 && deltaY < 50 && startXRef.current < 30) {
      setIsSwiping(true);
      const limitedDeltaX = Math.min(deltaX, window.innerWidth * 0.8);
      setSwipeX(limitedDeltaX);

      if (limitedDeltaX >= window.innerWidth * 0.3 && 'vibrate' in navigator) {
        navigator.vibrate(10);
      }
    }
  };

  const handleTouchEnd = () => {
    if (swipeX >= window.innerWidth * 0.3) {
      handleBack();
    }
    setSwipeX(0);
    setIsSwiping(false);
    startXRef.current = 0;
  };

  const clearLongPressTimer = () => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
  };

  const openMessageMenu = (message: SpiderMessage, position: { x: number; y: number }) => {
    setMenuMessage(message);
    setMenuPosition(position);
  };

  const handleMessageContextMenu = (
    e: React.MouseEvent<HTMLDivElement>,
    message: SpiderMessage
  ) => {
    e.preventDefault();
    openMessageMenu(message, { x: e.clientX, y: e.clientY });
  };

  const handleMessageTouchStart = (
    e: React.TouchEvent<HTMLDivElement>,
    message: SpiderMessage
  ) => {
    if (e.touches.length !== 1) return;
    const { clientX, clientY } = e.touches[0];
    touchStartRef.current = { x: clientX, y: clientY };
    clearLongPressTimer();
    longPressTimerRef.current = setTimeout(() => {
      openMessageMenu(message, { x: clientX, y: clientY });
      clearLongPressTimer();
    }, 500);
  };

  const handleMessageTouchMove = (e: React.TouchEvent<HTMLDivElement>) => {
    if (!touchStartRef.current || e.touches.length !== 1) return;
    const { clientX, clientY } = e.touches[0];
    const deltaX = Math.abs(clientX - touchStartRef.current.x);
    const deltaY = Math.abs(clientY - touchStartRef.current.y);
    if (deltaX > 10 || deltaY > 10) {
      clearLongPressTimer();
    }
  };

  const handleMessageTouchEnd = () => {
    clearLongPressTimer();
    touchStartRef.current = null;
  };

  const getMessageText = (msg: SpiderMessage): string => {
    if (msg.content?.text) return msg.content.text;
    if (msg.content?.base_six_four_audio) return '[Audio message]';
    return '';
  };

  const getSenderLabel = (msg: SpiderMessage) => (msg.role === 'user' ? 'You' : 'Spider');

  const formatTimestamp = (timestamp: number): string => {
    const date = new Date(timestamp * 1000);
    return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
  };

  const messageById = useMemo(() => {
    const map = new Map<string, SpiderMessage>();
    messages.forEach((msg) => {
      if (msg.id) {
        map.set(msg.id, msg);
      }
    });
    return map;
  }, [messages]);

  const streamingReplyTo = pendingReplyToId ? messageById.get(pendingReplyToId) : null;

  return (
    <div
      className={`spider-chat ${isSwiping ? 'swiping' : ''} ${isClosing ? 'closing' : ''}`}
      onTouchStart={handleTouchStart}
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
      style={{
        transform: `translateX(${swipeX}px)`,
        transition: isSwiping ? 'none' : 'transform 0.3s ease-out',
        opacity: isSwiping ? 1 - (swipeX / (window.innerWidth * 1.5)) : 1
      }}
    >
      <div className="spider-chat-header">
        <button className="spider-back-button" onClick={handleBack}>
          ←
        </button>
        <div className="spider-header-info">
          <div className="spider-avatar-wrap">
            <div className="spider-avatar">
              <span className="spider-icon">S</span>
            </div>
            <span className="spider-glow" aria-hidden="true" />
          </div>
          <div className="spider-header-text">
            <span className="spider-header-name">
              Spider
              <span className="spider-badge">AI</span>
            </span>
            <span className="spider-status">
              {isConnected ? 'Connected' : 'Connecting...'}
            </span>
          </div>
        </div>
        <button
          className="spider-clear-button"
          onClick={clearMessages}
          title="Clear conversation"
        >
          🗑️
        </button>
      </div>

      {error && (
        <div className="spider-error">
          <span>{error}</span>
          <button onClick={() => useSpiderStore.getState().setError(null)}>Dismiss</button>
        </div>
      )}

      <div className="spider-messages">
        {messages.length === 0 && !streamingMessage && (
          <div className="spider-empty">
            <div className="spider-empty-icon">S</div>
            <h3>Chat with Spider</h3>
            <p>Spider is your AI assistant. Ask anything!</p>
          </div>
        )}

        {messages.map((msg, index) => {
          const replyTarget = msg.replyTo ? messageById.get(msg.replyTo) : null;
          return (
          <div
            key={msg.id ?? index}
            className={`spider-message ${msg.role === 'user' ? 'user' : 'assistant'}`}
            onContextMenu={(e) => handleMessageContextMenu(e, msg)}
            onTouchStart={(e) => handleMessageTouchStart(e, msg)}
            onTouchMove={handleMessageTouchMove}
            onTouchEnd={handleMessageTouchEnd}
            onTouchCancel={handleMessageTouchEnd}
          >
            {replyTarget && (
              <div className="spider-reply-preview">
                <div className="spider-reply-label">↩ Reply to {getSenderLabel(replyTarget)}</div>
                <div className="spider-reply-text">{getMessageText(replyTarget)}</div>
              </div>
            )}
            <div className="spider-message-content">
              {getMessageText(msg)}
            </div>
            <div className="spider-message-time">
              {formatTimestamp(msg.timestamp)}
            </div>
          </div>
          );
        })}

        {streamingMessage && (
          <div className="spider-message assistant streaming">
            {streamingReplyTo && (
              <div className="spider-reply-preview">
                <div className="spider-reply-label">↩ Reply to {getSenderLabel(streamingReplyTo)}</div>
                <div className="spider-reply-text">{getMessageText(streamingReplyTo)}</div>
              </div>
            )}
            <div className="spider-message-content">
              {streamingMessage}
              <span className="cursor-blink">|</span>
            </div>
          </div>
        )}

        {isLoading && !streamingMessage && (
          <div className="spider-message assistant loading">
            <div className="spider-typing-indicator">
              <span></span>
              <span></span>
              <span></span>
            </div>
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      <div className="spider-input-wrapper">
        {replyingTo && (
          <div className="spider-replying">
            <div className="spider-replying-info">
              <span className="spider-replying-label">
                Replying to {getSenderLabel(replyingTo)}
              </span>
              <button
                className="spider-cancel-reply"
                onClick={() => setReplyingTo(null)}
                aria-label="Cancel reply"
              >
                ✕
              </button>
            </div>
            <div className="spider-replying-content">{getMessageText(replyingTo)}</div>
          </div>
        )}
        <form className="spider-input-container" onSubmit={handleSubmit}>
          <div className="spider-message-actions">
            <button
              className="spider-action-button"
              type="button"
              onClick={() => {/* TODO: file upload */}}
              aria-label="Attach file"
            >
              📎
            </button>
            <button
              className="spider-action-button"
              type="button"
              onClick={() => {/* TODO: voice note */}}
              aria-label="Record voice note"
            >
              🎤
            </button>
          </div>
          <textarea
            ref={inputRef}
            className="spider-input"
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Message Spider..."
            rows={1}
            disabled={isLoading}
          />
          {isLoading ? (
            <button
              type="button"
              className="spider-send-button cancel"
              onClick={cancelRequest}
              title="Cancel"
            >
              ⏹
            </button>
          ) : (
            <button
              type="submit"
              className="spider-send-button"
              disabled={!inputValue.trim()}
              title="Send"
            >
              ➤
            </button>
          )}
        </form>
      </div>
      {menuMessage && (
        <SpiderMessageMenu
          message={menuMessage}
          position={menuPosition}
          onReply={(message) => setReplyingTo(message)}
          onClose={() => setMenuMessage(null)}
        />
      )}
    </div>
  );
};

export default SpiderChat;
