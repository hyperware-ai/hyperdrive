import React, { useMemo } from 'react';
import { Chat } from '#caller-utils';
import Message from './Message';
import { getMessageSpacing, isDifferentDay, SpacingClass } from '../../utils/messageSpacing';
import './MessageList.css';

interface MessageListProps {
  messages: Chat.ChatMessage[];
}

const MessageList: React.FC<MessageListProps> = ({ messages }) => {
  // Compute spacing for each message
  const messageSpacing = useMemo(() => {
    const spacing = new Map<string, SpacingClass>();
    messages.forEach((msg, idx) => {
      const prev = messages[idx - 1];
      const isNewDate = prev ? isDifferentDay(prev.timestamp, msg.timestamp) : true;
      spacing.set(msg.id, getMessageSpacing({
        currentTimestamp: msg.timestamp,
        currentSender: msg.sender,
        prevTimestamp: prev?.timestamp,
        prevSender: prev?.sender,
        isNewDate,
      }));
    });
    return spacing;
  }, [messages]);

  const formatDate = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    const today = new Date();
    const yesterday = new Date(today);
    yesterday.setDate(yesterday.getDate() - 1);
    
    if (date.toDateString() === today.toDateString()) {
      return 'Today';
    } else if (date.toDateString() === yesterday.toDateString()) {
      return 'Yesterday';
    } else {
      return date.toLocaleDateString('en-US', { 
        weekday: 'long', 
        month: 'long', 
        day: 'numeric' 
      });
    }
  };

  let lastDate = '';

  return (
    <div className="message-list">
      {messages.map((message) => {
        const messageDate = formatDate(message.timestamp);
        const showDate = messageDate !== lastDate;
        lastDate = messageDate;
        
        return (
          <React.Fragment key={message.id}>
            {showDate && (
              <div className="date-separator">
                <span>{messageDate}</span>
              </div>
            )}
            <Message
              message={message}
              isOwn={message.sender === (window as any).our?.node}
              spacingClass={messageSpacing.get(message.id)}
            />
          </React.Fragment>
        );
      })}
    </div>
  );
};

export default MessageList;