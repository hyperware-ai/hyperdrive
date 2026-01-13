import React from 'react';
import './ChatSearch.css';

interface ChatSearchProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}

const ChatSearch: React.FC<ChatSearchProps> = ({ value, onChange, placeholder }) => {
  return (
    <div className="chat-search">
      <span className="search-icon">🔍</span>
      <input
        type="text"
        placeholder={placeholder || 'Search chats...'}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="search-input"
      />
    </div>
  );
};

export default ChatSearch;
