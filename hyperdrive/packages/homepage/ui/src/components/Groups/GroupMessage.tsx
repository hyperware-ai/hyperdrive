import React, { useCallback, useMemo, useState, useRef, useEffect } from 'react';
import { Chat } from '#caller-utils';
import { GroupMessage as GroupMessageType } from '../../types/groups';
import GroupMessageMenu from './GroupMessageMenu';
import { useChatStore } from '../../store/chat';
import { useGroupStore } from '../../store/groups';
import { getChatBasePath } from '../../utils/chatBase';
import ReactMarkdown from 'react-markdown';
import remarkBreaks from 'remark-breaks';
import remarkHwProtocol from '../../utils/remarkHwProtocol';
import { normalizeMessageContent } from '../../utils/normalizeMessageContent';
import Avatar from '../Common/Avatar';
import { SpacingClass } from '../../utils/messageSpacing';
import './GroupMessage.css';

interface ChildThreadInfo {
  id: string;
  title: string | null;
}

interface GroupMessageProps {
  message: GroupMessageType;
  currentNode?: string | null;
  onStartThread?: (parentThreadId: string, rootMessageId?: string) => void | Promise<void>;
  onOpenThread?: (threadId: string) => void;
  isActiveThread?: boolean;
  onReply?: (messageId: string) => void;
  onSetEditingMessage?: (message: { id: string; content: string }) => void;
  onDelete?: (messageId: string) => void;
  onReact?: (messageId: string, emoji: string) => void;
  childThread?: ChildThreadInfo | null;
  allMessages?: GroupMessageType[];
  isFirstFromSender?: boolean;
  isLastFromSender?: boolean;
  spacingClass?: SpacingClass;
}

const formatTime = (timestamp: number) => {
  const date = new Date(timestamp * 1000);
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
};

const formatFileSize = (bytes: number) => {
  if (!bytes || Number.isNaN(bytes)) return '0 KB';
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(bytes / 1024).toFixed(1)} KB`;
};


const buildApiUrl = (path: string) => {
  const basePath = getChatBasePath();

  if (path.startsWith(`${basePath}/`)) {
    return path;
  }

  return `${basePath}${path}`;
};

const parseApiResponse = <T,>(response: any): T => {
  if (response && typeof response === 'object') {
    if ('Ok' in response && response.Ok !== undefined) {
      return response.Ok as T;
    }
    if ('Err' in response && response.Err !== undefined) {
      throw new Error(response.Err as string);
    }
  }
  return response as T;
};

const GroupMessage = React.forwardRef<HTMLDivElement, GroupMessageProps>(
  ({ message, currentNode, onStartThread, onOpenThread, isActiveThread, onReply, onSetEditingMessage, onDelete, onReact, childThread, allMessages, isFirstFromSender = true, isLastFromSender = true, spacingClass = 'wide' }, ref) => {
    const isMine = currentNode && message.sender === currentNode;
    const statusLabel =
      message.status === 'sending'
        ? 'Sending…'
        : message.status === 'failed'
        ? 'Failed'
        : '';
    const [menuPosition, setMenuPosition] = useState<{ x: number; y: number } | null>(null);
    const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const touchStartRef = useRef<{ x: number; y: number } | null>(null);

    // Clean up timer on unmount
    useEffect(() => {
      return () => {
        if (longPressTimerRef.current) {
          clearTimeout(longPressTimerRef.current);
        }
      };
    }, []);

    const handleContextMenu = (e: React.MouseEvent) => {
      e.preventDefault();
      setMenuPosition({ x: e.clientX, y: e.clientY });
    };

    const groupedReactions = useMemo(() => {
      const grouped: Record<string, string[]> = {};
      message.reactions?.forEach((reaction) => {
        if (!grouped[reaction.emoji]) {
          grouped[reaction.emoji] = [];
        }
        grouped[reaction.emoji].push(reaction.user);
      });
      return grouped;
    }, [message.reactions]);

    const viewerId = currentNode ?? '';
    const { settings } = useChatStore();
    const { activeGroupId } = useGroupStore();
    const [attachmentUrls, setAttachmentUrls] = useState<Record<string, string>>({});
    const attachmentUrlsRef = useRef<Record<string, string>>({});

    const handleReaction = (emoji: string) => {
      if (onReact) {
        onReact(message.id, emoji);
      }
    };

    const replyToMessage = message.replyTo && allMessages
      ? allMessages.find(m => m.id === message.replyTo)
      : null;

    const handleReplyClick = () => {
      if (message.replyTo) {
        const element = document.getElementById(`group-message-${message.replyTo}`);
        if (element) {
          element.scrollIntoView({ behavior: 'smooth', block: 'center' });
          element.classList.add('highlight');
          setTimeout(() => element.classList.remove('highlight'), 2000);
        }
      }
    };

    // Touch handlers for iOS long press
    const handleTouchStart = (e: React.TouchEvent) => {
      const touch = e.touches[0];
      touchStartRef.current = { x: touch.clientX, y: touch.clientY };

      // Clear any existing timer
      if (longPressTimerRef.current) {
        clearTimeout(longPressTimerRef.current);
      }

      // Start long press timer (500ms)
      longPressTimerRef.current = setTimeout(() => {
        setMenuPosition({ x: touch.clientX, y: touch.clientY });
        // Haptic feedback
        if ('vibrate' in navigator) {
          navigator.vibrate(10);
        }
      }, 500);
    };

    const handleTouchMove = (e: React.TouchEvent) => {
      if (!touchStartRef.current) return;

      const touch = e.touches[0];
      const deltaX = Math.abs(touch.clientX - touchStartRef.current.x);
      const deltaY = Math.abs(touch.clientY - touchStartRef.current.y);

      // Cancel long press if finger moves too much
      if (deltaX > 10 || deltaY > 10) {
        if (longPressTimerRef.current) {
          clearTimeout(longPressTimerRef.current);
          longPressTimerRef.current = null;
        }
      }
    };

    const handleTouchEnd = () => {
      if (longPressTimerRef.current) {
        clearTimeout(longPressTimerRef.current);
        longPressTimerRef.current = null;
      }
      touchStartRef.current = null;
    };

    const normalizedContent = useMemo(
      () => normalizeMessageContent(message.content),
      [message.content],
    );

    const extractFilePayload = (payload: unknown): { bytes: Uint8Array | null; mimeType?: string } => {
      if (Array.isArray(payload)) {
        if (
          payload.length === 2 &&
          typeof payload[0] === 'string' &&
          Array.isArray(payload[1]) &&
          payload[1].every((value) => typeof value === 'number')
        ) {
          return { bytes: new Uint8Array(payload[1]), mimeType: payload[0] };
        }

        if (payload.every((value) => typeof value === 'number')) {
          return { bytes: new Uint8Array(payload as number[]) };
        }
      }
      return { bytes: null };
    };

    const fetchAttachmentBytes = useCallback(async (attachmentId: string) => {
      if (!activeGroupId) {
        throw new Error('No active group');
      }

      const response = await fetch(buildApiUrl('/api/download-group-file'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          DownloadGroupFile: {
            group_id: activeGroupId,
            attachment_id: attachmentId,
          },
        }),
      });

      if (!response.ok) {
        throw new Error(`Download failed with status ${response.status}`);
      }

      const json = await response.json();
      const payload = parseApiResponse<unknown>(json);
      return extractFilePayload(payload);
    }, [activeGroupId]);

    const handleAttachmentDownload = async (attachment: Chat.AttachmentDescriptor) => {
      try {
        const { bytes, mimeType } = await fetchAttachmentBytes(attachment.attachment_id);
        if (!bytes) {
          console.error('Unexpected attachment payload for', attachment.attachment_id);
          return;
        }

        const arrayBuffer = new Uint8Array(bytes).buffer;
        const blob = new Blob([arrayBuffer], {
          type: mimeType || attachment.mime_type || 'application/octet-stream',
        });
        const objectUrl = URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.href = objectUrl;
        link.download = attachment.filename || attachment.attachment_id;
        document.body.appendChild(link);
        link.click();
        link.remove();
        setTimeout(() => URL.revokeObjectURL(objectUrl), 0);
      } catch (error) {
        console.error('Failed to download attachment:', error);
      }
    };

    useEffect(() => {
      return () => {
        Object.values(attachmentUrlsRef.current).forEach((url) => {
          if (url.startsWith('blob:')) {
            URL.revokeObjectURL(url);
          }
        });
      };
    }, []);

    const isVoiceNote = message.type === Chat.MessageType.VoiceNote;

    useEffect(() => {
      if (!activeGroupId) return;
      const previewAttachments = message.attachments.filter((attachment) => {
        if (attachment.mime_type?.startsWith('audio/')) {
          return isVoiceNote;
        }
        if (attachment.mime_type?.startsWith('image/')) {
          return settings?.show_images;
        }
        return false;
      });

      previewAttachments.forEach(async (attachment) => {
        if (attachmentUrlsRef.current[attachment.attachment_id]) {
          return;
        }

        if (attachment.uri?.startsWith('data:')) {
          attachmentUrlsRef.current[attachment.attachment_id] = attachment.uri;
          setAttachmentUrls((prev) => ({
            ...prev,
            [attachment.attachment_id]: attachment.uri as string,
          }));
          return;
        }

        try {
          const { bytes, mimeType } = await fetchAttachmentBytes(attachment.attachment_id);
          if (!bytes) {
            return;
          }
          const arrayBuffer = new Uint8Array(bytes).buffer;
          const blob = new Blob([arrayBuffer], {
            type: mimeType || attachment.mime_type || 'application/octet-stream',
          });
          const objectUrl = URL.createObjectURL(blob);
          attachmentUrlsRef.current[attachment.attachment_id] = objectUrl;
          setAttachmentUrls((prev) => ({ ...prev, [attachment.attachment_id]: objectUrl }));
        } catch (error) {
          console.error('Failed to load group attachment preview:', error);
        }
      });
    }, [activeGroupId, fetchAttachmentBytes, isVoiceNote, message.attachments, settings?.show_images]);

    const renderMessageContent = useMemo(() => (
      <ReactMarkdown
        remarkPlugins={[remarkBreaks, remarkHwProtocol]}
        urlTransform={(url: string) => {
          if (url.startsWith('hw://')) {
            return url;
          }
          return url;
        }}
        components={{
          a: ({ href, children }) => {
            const imageRegex = /\.(jpg|jpeg|png|gif|webp|svg|bmp)$/i;
            const isHwProtocol = href?.startsWith('hw://');
            const linkColor = isMine ? '#ffffff' : '#4da6ff';

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
                        display: 'block',
                      }}
                      onError={(e) => {
                        const target = e.target as HTMLImageElement;
                        target.style.display = 'none';
                        const link = document.createElement('a');
                        link.href = href;
                        link.target = '_blank';
                        link.rel = 'noopener noreferrer';
                        link.textContent = href;
                        link.style.color = linkColor;
                        link.style.textDecoration = 'underline';
                        target.parentNode?.replaceChild(link, target);
                      }}
                    />
                  </a>
                </div>
              );
            }

            if (isHwProtocol) {
              return (
                <a
                  href={href}
                  style={{
                    color: linkColor,
                    textDecoration: 'underline',
                    cursor: 'pointer',
                  }}
                >
                  {children}
                </a>
              );
            }

            return (
              <a
                href={href}
                target="_blank"
                rel="noopener noreferrer"
                style={{
                  color: linkColor,
                  textDecoration: 'underline',
                }}
              >
                {children}
              </a>
            );
          },
          p: ({ children }) => (
            <p style={{ margin: '4px 0', wordBreak: 'break-word' }}>{children}</p>
          ),
          code: ({ children, ...props }) => {
            const inline = !(
              'className' in props &&
              typeof props.className === 'string' &&
              props.className.includes('language-')
            );
            if (inline) {
              return (
                <code
                  style={{
                    backgroundColor: isMine
                      ? 'rgba(0,0,0,0.2)'
                      : 'rgba(0,0,0,0.1)',
                    padding: '2px 4px',
                    borderRadius: '3px',
                    fontSize: '0.9em',
                  }}
                >
                  {children}
                </code>
              );
            }
            return (
              <pre
                style={{
                  backgroundColor: isMine
                    ? 'rgba(0,0,0,0.2)'
                    : 'rgba(0,0,0,0.1)',
                  padding: '8px',
                  borderRadius: '4px',
                  overflowX: 'auto',
                  fontSize: '0.9em',
                }}
              >
                <code>{children}</code>
              </pre>
            );
          },
          ul: ({ children }) => (
            <ul style={{ margin: '4px 0', paddingLeft: '20px' }}>{children}</ul>
          ),
          ol: ({ children }) => (
            <ol style={{ margin: '4px 0', paddingLeft: '20px' }}>{children}</ol>
          ),
          blockquote: ({ children }) => (
            <blockquote
              style={{
                borderLeft: `3px solid ${
                  isMine ? 'rgba(255,255,255,0.3)' : 'rgba(0,0,0,0.2)'
                }`,
                paddingLeft: '12px',
                margin: '8px 0',
                fontStyle: 'italic',
              }}
            >
              {children}
            </blockquote>
          ),
          h1: ({ children }) => (
            <h1 style={{ fontSize: '1.3em', fontWeight: 'bold', margin: '8px 0 4px 0' }}>
              {children}
            </h1>
          ),
          h2: ({ children }) => (
            <h2 style={{ fontSize: '1.2em', fontWeight: 'bold', margin: '6px 0 4px 0' }}>
              {children}
            </h2>
          ),
          h3: ({ children }) => (
            <h3 style={{ fontSize: '1.1em', fontWeight: 'bold', margin: '4px 0' }}>
              {children}
            </h3>
          ),
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
                  margin: '8px 0',
                }}
              />
            );
          },
        }}
      >
        {normalizedContent}
      </ReactMarkdown>
    ), [normalizedContent, settings?.show_images, isMine]);

    const hasAttachments = message.attachments && message.attachments.length > 0;
    const firstAttachmentLabel =
      message.attachments?.[0]?.filename ||
      message.attachments?.[0]?.mime_type ||
      'Attachment';
    const shouldRenderText =
      !hasAttachments ||
      (!isVoiceNote &&
        message.content &&
        message.content.trim() &&
        message.content.trim() !== firstAttachmentLabel);
    const attachmentLinkColor = isMine ? '#ffffff' : '#4da6ff';

    return (
      <>
        <div className={`group-message ${isMine ? 'mine' : ''} spacing-${spacingClass}`} ref={ref} id={`group-message-${message.id}`}>
          {/* Avatar for others - show on last message of consecutive group */}
          {!isMine && isLastFromSender && (
            <div className="group-message-avatar">
              <Avatar name={message.sender} size="small" />
            </div>
          )}
          {!isMine && !isLastFromSender && (
            <div className="group-message-avatar-spacer" />
          )}

          <div className="group-message-content">
            {/* Sender name - show on first message of consecutive group */}
            {!isMine && isFirstFromSender && (
              <div className="group-message-sender">{message.sender}</div>
            )}

            {/* Reply preview */}
            {replyToMessage && (
              <div className={`message-bubble__reply ${isMine ? '' : ''}`} onClick={handleReplyClick}>
                <div className="message-bubble__reply-label">↩ Reply to {replyToMessage.sender}</div>
                <div className="message-bubble__reply-content">{replyToMessage.content}</div>
              </div>
            )}

            <div
              className={`group-message-bubble message-bubble ${isMine ? 'message-bubble--own' : 'message-bubble--other'}`}
              onContextMenu={handleContextMenu}
              onTouchStart={handleTouchStart}
              onTouchMove={handleTouchMove}
              onTouchEnd={handleTouchEnd}
            >
              <div className="group-message-text">
                {hasAttachments && (
                  <div className="group-attachments">
                    {message.attachments.map((attachment) => {
                      const isImage = attachment.mime_type?.startsWith('image/');
                      const isAudio = isVoiceNote && attachment.mime_type?.startsWith('audio/');
                      const previewUrl = attachmentUrls[attachment.attachment_id];
                      const filename = attachment.filename || attachment.attachment_id;

                      return (
                        <div key={attachment.attachment_id} className="group-attachment">
                          {isImage && settings?.show_images && previewUrl && (
                            <img
                              src={previewUrl}
                              alt={filename}
                              className="group-attachment-image"
                              onClick={() => handleAttachmentDownload(attachment)}
                            />
                          )}
                          {isImage && settings?.show_images && !previewUrl && (
                            <div className="group-attachment-loading">Loading image…</div>
                          )}
                          {isAudio && previewUrl && (
                            <div className="message-bubble__audio">
                              <audio
                                controls
                                src={previewUrl}
                              />
                            </div>
                          )}
                          {isAudio && !previewUrl && (
                            <div className="message-bubble__audio">
                              <div className="group-attachment-loading">Loading audio…</div>
                            </div>
                          )}
                          {isAudio ? null : (
                            <>
                              <a
                                href={attachment.uri || '#'}
                                onClick={(e) => {
                                  e.preventDefault();
                                  handleAttachmentDownload(attachment);
                                }}
                                style={{
                                  color: attachmentLinkColor,
                                  textDecoration: 'underline',
                                  textUnderlineOffset: '2px',
                                  display: 'inline-block',
                                }}
                              >
                                {filename}
                              </a>
                              <div className="group-attachment-size">
                                {formatFileSize(attachment.size_bytes)}
                              </div>
                            </>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
                {shouldRenderText && renderMessageContent}
              </div>

              {/* Timestamp inside bubble */}
              <div className="message-bubble__footer">
                <span>{formatTime(message.timestamp)}</span>
                {statusLabel && <span className="group-message-status">{statusLabel}</span>}
              </div>

              {/* Reactions inside bubble */}
              {message.reactions && message.reactions.length > 0 && (
                <div className="message-bubble__reactions">
                  {Object.entries(groupedReactions).map(([emoji, users]) => (
                    <button
                      key={emoji}
                      className={`reaction ${
                        users.includes(viewerId) ? 'reacted' : ''
                      }`}
                      onClick={() => handleReaction(emoji)}
                      title={users.join(', ')}
                    >
                      {emoji} {users.length > 1 && users.length}
                    </button>
                  ))}
                </div>
              )}

              {/* Thread indicator */}
              {childThread && onOpenThread && (
                <button
                  type="button"
                  className="group-message-thread-indicator has-thread"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    onOpenThread(childThread.id);
                  }}
                >
                  <span className="thread-icon">↳</span>
                  <span className="thread-label">Thread</span>
                </button>
              )}
            </div>
          </div>
        </div>
        {menuPosition && (
          <GroupMessageMenu
            message={message}
            position={menuPosition}
            onClose={() => setMenuPosition(null)}
            onStartThread={onStartThread}
            onOpenThread={onOpenThread}
            onReply={onReply}
            onSetEditingMessage={onSetEditingMessage}
            onDelete={onDelete}
            onReact={onReact}
            currentNode={currentNode}
            childThread={childThread}
          />
        )}
      </>
    );
  });

export default GroupMessage;
