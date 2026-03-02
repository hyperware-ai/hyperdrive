import React, { useState, useMemo, useRef, useEffect } from 'react';
import { Chat } from '#caller-utils';
import MessageMenu from './MessageMenu';
import './Message.css';
import * as Caller from '#caller-utils';
import { useChatStore } from '../../store/chat';
import ReactMarkdown from 'react-markdown';
import remarkBreaks from 'remark-breaks';
import remarkHwProtocol from '../../utils/remarkHwProtocol';
import { normalizeMessageContent } from '../../utils/normalizeMessageContent';
import { SpacingClass } from '../../utils/messageSpacing';
import { getChatBasePath } from '../../utils/chatBase';

interface MessageProps {
  message: Chat.ChatMessage;
  isOwn: boolean;
  spacingClass?: SpacingClass;
}

const { add_reaction, remove_reaction } = Caller.Chat;
const truncateAddress = (value: string, head = 6, tail = 4): string =>
  value.length <= head + tail + 3 ? value : `${value.slice(0, head)}...${value.slice(-tail)}`;

const Message: React.FC<MessageProps> = ({ message, isOwn, spacingClass = 'wide' }) => {
  const [showMenu, setShowMenu] = useState(false);
  const [menuPosition, setMenuPosition] = useState({ x: 0, y: 0 });
  const [swipeX, setSwipeX] = useState(0);
  const [isSwiping, setIsSwiping] = useState(false);
  const [audioUrl, setAudioUrl] = useState<string | null>(null);
  const audioUrlRef = useRef<string | null>(null);
  const { activeChat, settings, setReplyingTo } = useChatStore();
  const messageRef = useRef<HTMLDivElement>(null);
  const isOfficial = message.sender === 'dao.hypr';
  const paymentInfo = message.payment_info;
  const isPaymentEvent = message.message_type === Chat.MessageType.Payment && Boolean(paymentInfo);
  const startXRef = useRef(0);
  const startYRef = useRef(0);
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const touchStartTimeRef = useRef(0);

  // Clean up timer on unmount
  useEffect(() => {
    return () => {
      if (longPressTimerRef.current) {
        clearTimeout(longPressTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    return () => {
      if (audioUrlRef.current && audioUrlRef.current.startsWith('blob:')) {
        URL.revokeObjectURL(audioUrlRef.current);
      }
    };
  }, []);

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    return date.toLocaleTimeString('en-US', { 
      hour: '2-digit', 
      minute: '2-digit' 
    });
  };

  const getStatusIcon = () => {
    switch (message.status) {
      case Chat.MessageStatus.Sending:
        return '...';
      case Chat.MessageStatus.Sent:
        return '✓';
      case Chat.MessageStatus.Delivered:
        return '✓✓';
      case Chat.MessageStatus.Failed:
        return '❌';
      default:
        return '';
    }
  };

  const buildDownloadUrl = (rawUrl: string) => {
    if (!rawUrl.startsWith('/')) {
      return rawUrl;
    }

    const basePath = getChatBasePath();

    if (rawUrl.startsWith(`${basePath}/`)) {
      return rawUrl;
    }

    return `${basePath}${rawUrl}`;
  };

  const extractFileRef = (rawUrl: string) => {
    const filesIndex = rawUrl.indexOf('/files/');
    if (filesIndex === -1) {
      return null;
    }

    const rest = rawUrl
      .slice(filesIndex + '/files/'.length)
      .split('?')[0]
      .split('#')[0];
    const [chatId, fileId] = rest.split('/');

    if (!chatId || !fileId) {
      return null;
    }

    return { chatId, fileId };
  };

  const handleFileDownload = async (e: React.MouseEvent<HTMLAnchorElement>) => {
    e.preventDefault();

    if (!message.file_info) {
      return;
    }

    const downloadBlob = (blob: Blob) => {
      const objectUrl = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = objectUrl;
      link.download = message.file_info?.filename || 'download';
      document.body.appendChild(link);
      link.click();
      link.remove();
      setTimeout(() => URL.revokeObjectURL(objectUrl), 0);
    };

    const fileUrl = message.file_info.url;
    if (fileUrl.startsWith('data:')) {
      try {
        const response = await fetch(fileUrl);
        const blob = await response.blob();
        downloadBlob(blob);
      } catch (error) {
        console.error('Failed to download image data URL:', error);
      }
      return;
    }

    const fileRef = extractFileRef(fileUrl);
    if (!fileRef) {
      console.error('Unsupported file URL:', fileUrl);
      return;
    }

    try {
      const response = await fetch(buildDownloadUrl('/api/download-file'), {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          DownloadFile: {
            chat_id: fileRef.chatId,
            file_id: fileRef.fileId,
          },
        }),
      });
      if (!response.ok) {
        throw new Error(`Download failed with status ${response.status}`);
      }

      const json = await response.json();
      if (json && typeof json === 'object' && 'Err' in json) {
        throw new Error((json as { Err: string }).Err);
      }

      const payload = json && typeof json === 'object' && 'Ok' in json
        ? (json as { Ok: unknown }).Ok
        : json;
      let mimeType = message.file_info.mime_type || 'application/octet-stream';
      let fileBytes: number[] | null = null;

      if (Array.isArray(payload)) {
        if (payload.length === 2 && typeof payload[0] === 'string' && Array.isArray(payload[1])) {
          mimeType = payload[0];
          fileBytes = payload[1] as number[];
        } else if (payload.every((value) => typeof value === 'number')) {
          fileBytes = payload as number[];
        }
      }

      if (!fileBytes) {
        console.error('Unexpected file payload shape:', payload);
        return;
      }

      const blob = new Blob([new Uint8Array(fileBytes)], { type: mimeType });
      downloadBlob(blob);
    } catch (error) {
      console.error('Failed to download file:', error);
    }
  };

  const handleLongPress = (e: React.MouseEvent | React.TouchEvent) => {
    e.preventDefault();
    const rect = (e.target as HTMLElement).getBoundingClientRect();
    setMenuPosition({ x: rect.left, y: rect.top });
    setShowMenu(true);
  };

  const isAudioMessage = Boolean(
    message.file_info && message.message_type === Chat.MessageType.VoiceNote,
  );

  useEffect(() => {
    if (!isAudioMessage || !message.file_info) {
      if (audioUrlRef.current && audioUrlRef.current.startsWith('blob:')) {
        URL.revokeObjectURL(audioUrlRef.current);
      }
      audioUrlRef.current = null;
      setAudioUrl(null);
      return;
    }

    const fileUrl = message.file_info.url;
    if (fileUrl.startsWith('data:')) {
      setAudioUrl(fileUrl);
      return;
    }

    const fileRef = extractFileRef(fileUrl);
    if (!fileRef) {
      return;
    }

    let cancelled = false;
    const fetchAudio = async () => {
      try {
        const response = await fetch(buildDownloadUrl('/api/download-file'), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            DownloadFile: {
              chat_id: fileRef.chatId,
              file_id: fileRef.fileId,
            },
          }),
        });
        if (!response.ok) {
          throw new Error(`Download failed with status ${response.status}`);
        }

        const json = await response.json();
        if (json && typeof json === 'object' && 'Err' in json) {
          throw new Error((json as { Err: string }).Err);
        }
        const payload =
          json && typeof json === 'object' && 'Ok' in json ? (json as { Ok: unknown }).Ok : json;
        let mimeType = message.file_info?.mime_type || 'audio/webm';
        let fileBytes: number[] | null = null;

        if (Array.isArray(payload)) {
          if (payload.length === 2 && typeof payload[0] === 'string' && Array.isArray(payload[1])) {
            mimeType = payload[0];
            fileBytes = payload[1] as number[];
          } else if (payload.every((value) => typeof value === 'number')) {
            fileBytes = payload as number[];
          }
        }

        if (!fileBytes || cancelled) {
          return;
        }

        const blob = new Blob([new Uint8Array(fileBytes)], { type: mimeType });
        const objectUrl = URL.createObjectURL(blob);
        if (audioUrlRef.current && audioUrlRef.current.startsWith('blob:')) {
          URL.revokeObjectURL(audioUrlRef.current);
        }
        audioUrlRef.current = objectUrl;
        setAudioUrl(objectUrl);
      } catch (error) {
        console.error('Failed to load audio:', error);
      }
    };

    fetchAudio();

    return () => {
      cancelled = true;
    };
  }, [isAudioMessage, message.file_info?.url, message.file_info?.mime_type, message.message_type]);

  // Swipe handlers for swipe-to-reply
  const handleTouchStart = (e: React.TouchEvent) => {
    startXRef.current = e.touches[0].clientX;
    startYRef.current = e.touches[0].clientY;
    touchStartTimeRef.current = Date.now();
    setIsSwiping(false);
    
    // Start long press timer for iOS
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
    }
    
    longPressTimerRef.current = setTimeout(() => {
      // Trigger long press after 500ms
      const touch = e.touches[0];
      const rect = messageRef.current?.getBoundingClientRect();
      if (rect) {
        setMenuPosition({ x: touch.clientX, y: touch.clientY });
        setShowMenu(true);
        // Haptic feedback for long press
        if ('vibrate' in navigator) {
          navigator.vibrate(10);
        }
      }
    }, 500);
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    const currentX = e.touches[0].clientX;
    const currentY = e.touches[0].clientY;
    const deltaX = currentX - startXRef.current;
    const deltaY = Math.abs(currentY - startYRef.current);
    
    // Cancel long press if user moves finger too much
    if (Math.abs(deltaX) > 10 || deltaY > 10) {
      if (longPressTimerRef.current) {
        clearTimeout(longPressTimerRef.current);
        longPressTimerRef.current = null;
      }
    }
    
    // Only trigger swipe if horizontal movement is greater than vertical
    if (Math.abs(deltaX) > 10 && deltaY < 50) {
      setIsSwiping(true);
      // Limit swipe distance
      const limitedDeltaX = Math.min(Math.max(deltaX, -80), 80);
      setSwipeX(limitedDeltaX);
      
      // Add haptic feedback when reaching threshold
      if (Math.abs(limitedDeltaX) >= 60 && 'vibrate' in navigator) {
        navigator.vibrate(10);
      }
    }
  };

  const handleTouchEnd = () => {
    // Clear long press timer
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
    
    // Check if it was a quick tap (less than 200ms) to prevent accidental swipe-to-reply
    const touchDuration = Date.now() - touchStartTimeRef.current;
    
    if (Math.abs(swipeX) >= 60 && touchDuration > 200) {
      // Trigger reply action
      setReplyingTo(message);
      // Add visual feedback
      if (messageRef.current) {
        messageRef.current.classList.add('reply-triggered');
        setTimeout(() => {
          messageRef.current?.classList.remove('reply-triggered');
        }, 300);
      }
    }
    setSwipeX(0);
    setIsSwiping(false);
  };

  const handleReaction = async (emoji: string) => {
    try {
      const ourNode = (window as any).our?.node;
      console.log('[REACTION] Our node:', ourNode);
      console.log('[REACTION] Message reactions:', message.reactions);
      
      // Check if user already reacted with this emoji
      const existingReaction = message.reactions?.find(
        r => r.user === ourNode && r.emoji === emoji
      );
      
      console.log('[REACTION] Existing reaction found:', existingReaction);
      console.log('[REACTION] Chat ID:', activeChat?.id, 'Message ID:', message.id);
      
      if (existingReaction) {
        console.log('[REACTION] Removing reaction:', emoji);
        const result = await remove_reaction({ 
          chat_id: activeChat?.id || '',
          message_id: message.id, 
          emoji 
        });
        console.log('[REACTION] Remove reaction result:', result);
      } else {
        console.log('[REACTION] Adding reaction:', emoji);
        const result = await add_reaction({ 
          chat_id: activeChat?.id || '',
          message_id: message.id, 
          emoji 
        });
        console.log('[REACTION] Add reaction result:', result);
      }
      
      // WebSocket will handle the update
    } catch (err) {
      console.error('[REACTION] Error handling reaction:', err);
    }
  };

  const groupReactions = () => {
    const grouped: { [emoji: string]: string[] } = {};
    message.reactions?.forEach(reaction => {
      if (!grouped[reaction.emoji]) {
        grouped[reaction.emoji] = [];
      }
      grouped[reaction.emoji].push(reaction.user);
    });
    return grouped;
  };

  // Render message content with markdown support
  const normalizedContent = useMemo(
    () => normalizeMessageContent(message.content),
    [message.content],
  );

  const renderMessageContent = useMemo(() => {
    return (
      <ReactMarkdown
        remarkPlugins={[remarkBreaks, remarkHwProtocol]}
        urlTransform={(url: string) => {
          // Allow hw:// protocol links to pass through unchanged
          if (url.startsWith('hw://')) {
            return url;
          }
          // For other URLs, return as-is (React Markdown will handle security)
          return url;
        }}
        components={{
          // Custom link rendering
          a: ({ href, children }) => {
            const imageRegex = /\.(jpg|jpeg|png|gif|webp|svg|bmp)$/i;
            const isHwProtocol = href?.startsWith('hw://');
            
            // Check if link is an image URL
            if (href && imageRegex.test(href) && settings?.show_images) {
              return (
                <div style={{ margin: '8px 0' }}>
                  <a href={href} target="_blank" rel="noopener noreferrer">
                    <img 
                      src={href} 
                      alt="Image" 
                      style={{ 
                        maxWidth: '100%', 
                        maxHeight: '300px',
                        borderRadius: '8px',
                        display: 'block'
                      }}
                      onError={(e) => {
                        // If image fails to load, show as link instead
                        const target = e.target as HTMLImageElement;
                        target.style.display = 'none';
                        const link = document.createElement('a');
                        link.href = href;
                        link.target = '_blank';
                        link.rel = 'noopener noreferrer';
                        link.textContent = href;
                        link.style.color = isOwn ? '#ffffff' : '#4da6ff';
                        link.style.textDecoration = 'underline';
                        target.parentNode?.replaceChild(link, target);
                      }}
                    />
                  </a>
                </div>
              );
            }
            
            // hw:// protocol links - let hw-protocol-watcher handle them
            if (isHwProtocol) {
              return (
                <a 
                  href={href}
                  style={{ 
                    color: isOwn ? '#ffffff' : '#4da6ff',
                    textDecoration: 'underline',
                    cursor: 'pointer'
                  }}
                >
                  {children}
                </a>
              );
            }
            
            // Regular link
            return (
              <a 
                href={href} 
                target="_blank" 
                rel="noopener noreferrer"
                style={{ 
                  color: isOwn ? '#ffffff' : '#4da6ff',
                  textDecoration: 'underline'
                }}
              >
                {children}
              </a>
            );
          },
          // Custom paragraph rendering to handle spacing
          p: ({ children }) => (
            <p style={{ margin: '4px 0', wordBreak: 'break-word' }}>{children}</p>
          ),
          // Custom code rendering
          code: ({ children, ...props }) => {
            const inline = !('className' in props && typeof props.className === 'string' && props.className.includes('language-'));
            if (inline) {
              return (
                <code style={{ 
                  backgroundColor: isOwn ? 'rgba(0,0,0,0.2)' : 'rgba(0,0,0,0.1)',
                  padding: '2px 4px',
                  borderRadius: '3px',
                  fontSize: '0.9em'
                }}>
                  {children}
                </code>
              );
            }
            return (
              <pre style={{ 
                backgroundColor: isOwn ? 'rgba(0,0,0,0.2)' : 'rgba(0,0,0,0.1)',
                padding: '8px',
                borderRadius: '4px',
                overflowX: 'auto',
                fontSize: '0.9em'
              }}>
                <code>{children}</code>
              </pre>
            );
          },
          // Custom list rendering
          ul: ({ children }) => (
            <ul style={{ margin: '4px 0', paddingLeft: '20px' }}>{children}</ul>
          ),
          ol: ({ children }) => (
            <ol style={{ margin: '4px 0', paddingLeft: '20px' }}>{children}</ol>
          ),
          // Custom blockquote rendering
          blockquote: ({ children }) => (
            <blockquote style={{ 
              borderLeft: `3px solid ${isOwn ? 'rgba(255,255,255,0.3)' : 'rgba(0,0,0,0.2)'}`,
              paddingLeft: '12px',
              margin: '8px 0',
              fontStyle: 'italic'
            }}>
              {children}
            </blockquote>
          ),
          // Custom heading rendering
          h1: ({ children }) => (
            <h1 style={{ fontSize: '1.3em', fontWeight: 'bold', margin: '8px 0 4px 0' }}>{children}</h1>
          ),
          h2: ({ children }) => (
            <h2 style={{ fontSize: '1.2em', fontWeight: 'bold', margin: '6px 0 4px 0' }}>{children}</h2>
          ),
          h3: ({ children }) => (
            <h3 style={{ fontSize: '1.1em', fontWeight: 'bold', margin: '4px 0' }}>{children}</h3>
          ),
          // Custom image rendering
          img: ({ src, alt }) => {
            if (!settings?.show_images) return null;
            return (
              <img 
                src={src} 
                alt={alt} 
                style={{ 
                  maxWidth: '100%', 
                  maxHeight: '300px',
                  borderRadius: '8px',
                  display: 'block',
                  margin: '8px 0'
                }}
              />
            );
          },
        }}
      >
        {normalizedContent}
      </ReactMarkdown>
    );
  }, [normalizedContent, settings?.show_images, isOwn]);

  return (
    <>
      <div 
        ref={messageRef}
        id={`message-${message.id}`}
        className={`message ${isOwn ? 'own' : 'other'} ${isOfficial ? 'official-message' : ''} ${isPaymentEvent ? 'payment-event' : ''} ${isSwiping ? 'swiping' : ''} spacing-${spacingClass}`}
        onContextMenu={handleLongPress}
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
        style={{
          transform: `translateX(${swipeX}px)`,
          transition: isSwiping ? 'none' : 'transform 0.2s ease-out'
        }}
      >
        {message.reply_to && (
          <div 
            className="reply-to"
            onClick={() => {
              // Scroll to the original message
              const element = document.getElementById(`message-${message.reply_to}`);
              if (element) {
                element.scrollIntoView({ behavior: 'smooth', block: 'center' });
                // Add a highlight animation
                element.classList.add('highlight');
                setTimeout(() => element.classList.remove('highlight'), 2000);
              }
            }}
            style={{ cursor: 'pointer' }}
          >
            <div className="reply-to-label">↩ Reply</div>
            <div className="reply-to-content">
              {activeChat?.messages.find(m => m.id === message.reply_to)?.content || 'Message not found'}
            </div>
          </div>
        )}

        <div className="message-content">
          {/* If this is a file/image message with file info, show it specially */}
          {isPaymentEvent && paymentInfo ? (
            <a
              className="payment-event-card"
              href={paymentInfo.explorer_url}
              target="_blank"
              rel="noopener noreferrer"
              title="Open transaction on BaseScan"
            >
              <div className="payment-event-pill">Payment Event</div>
              <div className="payment-event-main">
                <strong>{isOwn ? 'You' : activeChat?.counterparty_profile?.name || message.sender}</strong>
                <span>sent</span>
                <strong>{paymentInfo.amount} {paymentInfo.coin_name}</strong>
                <span>to {truncateAddress(paymentInfo.to_address)}</span>
              </div>
              <div className="payment-event-meta">
                <span className="payment-event-hash">{truncateAddress(paymentInfo.tx_hash, 10, 8)}</span>
                <span className="payment-event-link">View on BaseScan ↗</span>
              </div>
            </a>
          ) : isAudioMessage && message.file_info ? (
            <div className="dm-audio-wrapper">
              {audioUrl ? (
                <audio
                  controls
                  src={audioUrl}
                  className="dm-audio-player"
                />
              ) : (
                <div style={{ fontSize: '12px', opacity: 0.7 }}>Loading audio…</div>
              )}
            </div>
          ) : message.file_info && message.message_type === 'Image' && settings?.show_images ? (
            <div>
              <img 
                src={message.file_info.url} 
                alt={message.file_info.filename}
                style={{ 
                  maxWidth: '100%', 
                  maxHeight: '300px',
                  borderRadius: '8px',
                  display: 'block',
                  marginBottom: '8px'
                }}
              />
              <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                <a
                  href={buildDownloadUrl(message.file_info.url)}
                  onClick={handleFileDownload}
                  download={message.file_info.filename}
                  style={{ 
                    color: isOwn ? '#ffffff' : '#4da6ff',
                    textDecoration: 'underline',
                    textUnderlineOffset: '2px',
                    display: 'inline-block'
                  }}
                >
                  {message.file_info.filename}
                </a>
                <div style={{ fontSize: '12px', opacity: 0.8 }}>
                  {(message.file_info.size / 1024).toFixed(1)} KB
                </div>
              </div>
            </div>
          ) : message.file_info && message.message_type === 'File' ? (
            <div>
              <a 
                href={buildDownloadUrl(message.file_info.url)}
                onClick={handleFileDownload}
                download={message.file_info.filename}
                style={{ 
                  color: isOwn ? '#ffffff' : '#4da6ff',
                  textDecoration: 'underline',
                  textUnderlineOffset: '2px',
                  display: 'inline-block'
                }}
              >
                {message.file_info.filename}
              </a>
              <div style={{ fontSize: '12px', opacity: 0.8, marginTop: '4px' }}>
                {(message.file_info.size / 1024).toFixed(1)} KB
              </div>
            </div>
          ) : (
            renderMessageContent
          )}
        </div>
        
        <div className="message-footer">
          <span className="message-time">{formatTime(message.timestamp)}</span>
          {isOwn && (
            <span className="message-status">{getStatusIcon()}</span>
          )}
        </div>
        
        {message.reactions && message.reactions.length > 0 && (
          <div className="message-reactions">
            {Object.entries(groupReactions()).map(([emoji, users]) => (
              <button
                key={emoji}
                className={`reaction ${users.includes((window as any).our?.node || '') ? 'reacted' : ''}`}
                onClick={() => handleReaction(emoji)}
                title={users.join(', ')}
              >
                {emoji} {users.length > 1 && users.length}
              </button>
            ))}
          </div>
        )}
      </div>
      
      {showMenu && (
        <MessageMenu 
          message={message}
          isOwn={isOwn}
          position={menuPosition}
          onClose={() => setShowMenu(false)}
        />
      )}
    </>
  );
};

export default Message;
