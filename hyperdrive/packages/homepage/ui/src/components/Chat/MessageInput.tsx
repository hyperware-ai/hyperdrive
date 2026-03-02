import React, { useState, useRef, useEffect } from 'react';
import * as Caller from '#caller-utils';
import { useChatStore } from '../../store/chat';
import FileUpload from './FileUpload';
import VoiceNote from './VoiceNote';
import './MessageInput.css';

interface MessageInputProps {
  chatId: string;
  onSendMessage?: () => void;
}

const MessageInput: React.FC<MessageInputProps> = ({ chatId, onSendMessage }) => {
  const [message, setMessage] = useState('');
  const [showFileUpload, setShowFileUpload] = useState(false);
  const [showVoiceNote, setShowVoiceNote] = useState(false);
  const { sendMessage, replyingTo, setReplyingTo, editingMessage, setEditingMessage, editMessage } = useChatStore();
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Detect if user is on mobile device (avoid touch-enabled laptops)
  const isMobile = /iPhone|iPad|iPod|Android/i.test(navigator.userAgent);

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

  const handleSubmit = async (e?: React.FormEvent) => {
    e?.preventDefault();
    const messageText = message.trim();
    if (messageText) {
      // Clear input immediately to prevent double send
      setMessage('');

      if (editingMessage) {
        // Handle edit
        const editId = editingMessage.id;
        setEditingMessage(null);
        if (messageText !== editingMessage.content) {
          await editMessage(editId, messageText);
        }
      } else {
        // Handle send
        const replyToId = replyingTo?.id;
        setReplyingTo(null);
        await sendMessage(chatId, messageText, replyToId);
        onSendMessage?.();
        // Trigger send pulse animation on input container
        if (containerRef.current) {
          containerRef.current.classList.add('just-sent');
          setTimeout(() => containerRef.current?.classList.remove('just-sent'), 350);
        }
      }
      inputRef.current?.focus();
    }
  };

  const handleCancelEdit = () => {
    setEditingMessage(null);
    setMessage('');
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // On desktop: Enter sends, Shift+Enter adds newline
    // On mobile: Enter adds newline, send button must be used
    if (e.key === 'Enter' && !e.shiftKey && !isMobile) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleSendVoiceNote = async (payload: { base64: string; duration: number; mimeType: string }) => {
    const replyToId = replyingTo?.id || null;
    setReplyingTo(null);
    await Caller.Chat.send_voice_note({
      chat_id: chatId,
      audio_data: payload.base64,
      duration: payload.duration,
      reply_to: replyToId,
    });
    onSendMessage?.();
  };

  return (
    <div className="message-input-wrapper">
      {editingMessage && (
        <div className="edit-preview">
          <div className="edit-info">
            <span className="edit-label">Editing message</span>
            <button
              className="cancel-edit"
              onClick={handleCancelEdit}
              aria-label="Cancel edit"
            >
              <span className="material-symbols-outlined" aria-hidden="true">
                close
              </span>
            </button>
          </div>
          <div className="edit-content">{editingMessage.content}</div>
        </div>
      )}
      {replyingTo && !editingMessage && (
        <div className="reply-preview">
          <div className="reply-info">
            <span className="reply-label">Replying to {replyingTo.sender}</span>
            <button
              className="cancel-reply"
              onClick={() => setReplyingTo(null)}
              aria-label="Cancel reply"
            >
              <span className="material-symbols-outlined" aria-hidden="true">
                close
              </span>
            </button>
          </div>
          <div className="reply-content">{replyingTo.content}</div>
        </div>
      )}

      <div ref={containerRef} className={`message-input-container ${editingMessage ? 'editing' : ''}`}>
        {!editingMessage && (
          <div className="message-actions">
            <button
              className="message-action-button"
              type="button"
              onClick={() => setShowFileUpload(true)}
              aria-label="Attach file"
            >
              <span className="material-symbols-outlined" aria-hidden="true">
                attach_file
              </span>
            </button>
            <button
              className="message-action-button"
              type="button"
              onClick={() => setShowVoiceNote(true)}
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
          className="message-input"
          placeholder={editingMessage ? 'Edit your message…' : 'Type a message…'}
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={handleKeyDown}
          rows={1}
        />
        <button
          className={`send-button ${editingMessage ? 'edit-mode' : ''}`}
          onClick={() => handleSubmit()}
          disabled={!message.trim()}
          aria-label={editingMessage ? 'Save edit' : 'Send message'}
        >
          <span className="material-symbols-outlined" aria-hidden="true">
            {editingMessage ? 'check' : 'send'}
          </span>
        </button>
      </div>
      {showFileUpload && <FileUpload onClose={() => setShowFileUpload(false)} />}
      {showVoiceNote && (
        <VoiceNote
          onClose={() => setShowVoiceNote(false)}
          onSend={handleSendVoiceNote}
        />
      )}
    </div>
  );
};

export default MessageInput;
