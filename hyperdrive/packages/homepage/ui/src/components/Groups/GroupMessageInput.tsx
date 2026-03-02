import React, { useEffect, useRef, useState } from 'react';
import GroupFileUpload from './GroupFileUpload';
import VoiceNote from '../Chat/VoiceNote';
import { useGroupStore } from '../../store/groups';
import { getChatBasePath } from '../../utils/chatBase';
import './GroupMessageInput.css';

interface ReplyingToMessage {
  id: string;
  sender: string;
  content: string;
}

interface EditingMessage {
  id: string;
  content: string;
}

interface GroupMessageInputProps {
  onSend: (content: string, replyTo?: string | null) => Promise<void>;
  onEdit?: (messageId: string, newContent: string) => void;
  disabled?: boolean;
  disabledReason?: string;
  replyingTo?: ReplyingToMessage | null;
  onCancelReply?: () => void;
  editingMessage?: EditingMessage | null;
  onCancelEdit?: () => void;
}

const GroupMessageInput: React.FC<GroupMessageInputProps> = ({
  onSend,
  onEdit,
  disabled,
  disabledReason,
  replyingTo,
  onCancelReply,
  editingMessage,
  onCancelEdit,
}) => {
  const [message, setMessage] = useState('');
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const isMobile = /iPhone|iPad|iPod|Android/i.test(navigator.userAgent);
  const [showFileUpload, setShowFileUpload] = useState(false);
  const [showVoiceNote, setShowVoiceNote] = useState(false);
  const {
    activeGroupId,
    activeThreadId,
    draftThread,
    createThread,
    clearDraftThread,
    setActiveThread,
    refreshActiveGroup,
  } = useGroupStore();

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

  const sendGroupVoiceNote = async (payload: { base64: string; duration: number; mimeType: string }) => {
    if (disabled) return;
    if (!activeGroupId) {
      throw new Error('No active group');
    }

    let threadId = activeThreadId;
    const replyToId = replyingTo?.id ?? null;

    const postVoiceNote = async (targetThreadId: string) => {
      const response = await fetch(buildApiUrl('/api/send-group-voice-note'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          SendGroupVoiceNote: {
            group_id: activeGroupId,
            thread_id: targetThreadId,
            reply_to: replyToId,
            audio_data: payload.base64,
            duration: payload.duration,
            mime_type: payload.mimeType,
          },
        }),
      });

      if (!response.ok) {
        throw new Error(`Voice note failed with status ${response.status}`);
      }

      const json = await response.json();
      return parseApiResponse<{ message?: { message_id?: string } }>(json);
    };

    if (!threadId && draftThread) {
      const parentThreadId = draftThread.parentThreadId;
      if (!draftThread.rootMessageId && parentThreadId) {
        const rootRes = await postVoiceNote(parentThreadId);
        const rootMessageId = rootRes.message?.message_id;
        if (rootMessageId) {
          const title = `Voice note ${payload.duration}s`;
          const newThreadId = await createThread(title, parentThreadId, rootMessageId);
          if (newThreadId) {
            clearDraftThread();
            setActiveThread(newThreadId);
          }
        }
        await refreshActiveGroup();
        onCancelReply?.();
        return;
      }

      const title = `Voice note ${payload.duration}s`;
      const newThreadId = await createThread(
        title,
        parentThreadId,
        draftThread.rootMessageId,
      );
      if (!newThreadId) {
        throw new Error('Failed to create thread');
      }
      clearDraftThread();
      setActiveThread(newThreadId);
      threadId = newThreadId;
    }

    if (!threadId) {
      throw new Error('Select a thread to share voice notes');
    }

    await postVoiceNote(threadId);
    await refreshActiveGroup();
    onCancelReply?.();
  };

  useEffect(() => {
    if (!disabled) {
      inputRef.current?.focus();
    }
  }, [disabled]);

  // Focus input when replying
  useEffect(() => {
    if (replyingTo) {
      inputRef.current?.focus();
    }
  }, [replyingTo]);

  // Focus input and populate when editing
  useEffect(() => {
    if (editingMessage) {
      setMessage(editingMessage.content);
      inputRef.current?.focus();
    }
  }, [editingMessage]);

  const handleSend = async () => {
    const text = message.trim();
    if (!text || disabled) return;
    setMessage('');

    if (editingMessage && onEdit) {
      // Handle edit
      const editId = editingMessage.id;
      onCancelEdit?.();
      if (text !== editingMessage.content) {
        onEdit(editId, text);
      }
    } else {
      // Handle send
      const replyToId = replyingTo?.id || null;
      onCancelReply?.();
      await onSend(text, replyToId);
    }
    inputRef.current?.focus();
  };

  const handleCancelEdit = () => {
    onCancelEdit?.();
    setMessage('');
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey && !isMobile) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="group-message-input">
      {editingMessage && (
        <div className="group-edit-preview">
          <div className="group-edit-info">
            <span className="group-edit-label">Editing message</span>
            <button
              className="group-cancel-edit"
              onClick={handleCancelEdit}
              aria-label="Cancel edit"
            >
              <span className="material-symbols-outlined" aria-hidden="true">
                close
              </span>
            </button>
          </div>
          <div className="group-edit-content">{editingMessage.content}</div>
        </div>
      )}
      {replyingTo && !editingMessage && (
        <div className="group-reply-preview">
          <div className="group-reply-info">
            <span className="group-reply-label">Replying to {replyingTo.sender}</span>
            <button
              className="group-cancel-reply"
              onClick={onCancelReply}
              aria-label="Cancel reply"
            >
              <span className="material-symbols-outlined" aria-hidden="true">
                close
              </span>
            </button>
          </div>
          <div className="group-reply-content">{replyingTo.content}</div>
        </div>
      )}
      {disabled && disabledReason && (
        <div className="group-input-warning">{disabledReason}</div>
      )}
      <div className={`group-input-row ${disabled ? 'disabled' : ''} ${editingMessage ? 'editing' : ''}`}>
        {!editingMessage && (
          <div className="group-actions">
            <button
              className="group-action-button"
              type="button"
              onClick={() => setShowFileUpload(true)}
              disabled={disabled}
              aria-label="Attach file"
            >
              <span className="material-symbols-outlined" aria-hidden="true">
                attach_file
              </span>
            </button>
            <button
              className="group-action-button"
              type="button"
              onClick={() => setShowVoiceNote(true)}
              disabled={disabled}
              aria-label="Record voice note"
            >
              <span className="material-symbols-outlined" aria-hidden="true">
                mic
              </span>
            </button>
          </div>
        )}
        <textarea
          ref={inputRef}
          placeholder={editingMessage ? 'Edit your message…' : disabled ? 'Sending disabled' : 'Type a message…'}
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={handleKeyDown}
          rows={1}
          disabled={disabled}
        />
        <button
          className={`group-send ${editingMessage ? 'edit-mode' : ''}`}
          onClick={handleSend}
          disabled={disabled || message.trim().length === 0}
          aria-label={editingMessage ? 'Save edit' : 'Send message'}
        >
          <span className="material-symbols-outlined" aria-hidden="true">
            {editingMessage ? 'check' : 'send'}
          </span>
        </button>
      </div>
      {showFileUpload && <GroupFileUpload onClose={() => setShowFileUpload(false)} />}
      {showVoiceNote && (
        <VoiceNote
          onClose={() => setShowVoiceNote(false)}
          onSend={sendGroupVoiceNote}
        />
      )}
    </div>
  );
};

export default GroupMessageInput;
