import React, { useEffect, useState } from 'react';
import { Chat as api } from '#caller-utils';
import { useChatStore } from '../../store/chat';
import ChatListItem from './ChatListItem';
import ChatSearch from './ChatSearch';
import NewChatButton from './NewChatButton';
import NewChatModal from './NewChatModal';
import './ChatList.css';

const ChatList: React.FC = () => {
  const { chats, searchIndex } = useChatStore();
  const [searchQuery, setSearchQuery] = useState('');
  const [showNewChat, setShowNewChat] = useState(false);
  const [filteredChats, setFilteredChats] = useState(chats);

  useEffect(() => {
    let cancelled = false;

    const runSearch = async () => {
      if (!searchQuery.trim()) {
        setFilteredChats(chats);
        return;
      }
      const results = await searchIndex(searchQuery, {
        scope: api.SearchScope.Chats,
        limit: 100,
      });
      if (cancelled) return;

      const rankByChat = new Map<string, number>();
      results.forEach((result, idx) => {
        if (result.chat_id && !rankByChat.has(result.chat_id)) {
          rankByChat.set(result.chat_id, idx);
        }
      });

      const matches = chats.filter((chat) => rankByChat.has(chat.id));
      matches.sort(
        (a, b) => (rankByChat.get(a.id) ?? 0) - (rankByChat.get(b.id) ?? 0),
      );
      setFilteredChats(matches);
    };

    runSearch();
    return () => {
      cancelled = true;
    };
  }, [searchQuery, chats, searchIndex]);

  return (
    <div className="chat-list-container">
      <div className="chat-list-header">
        <ChatSearch value={searchQuery} onChange={setSearchQuery} />
        <NewChatButton onClick={() => setShowNewChat(true)} />
      </div>
      
      <div className="chat-list">
        {filteredChats.length > 0 ? (
          filteredChats.map((chat, index) => (
            <ChatListItem key={chat.id} chat={chat} index={index} />
          ))
        ) : (
          <div className="empty-state">
            {searchQuery ? 'No chats found' : 'No chats yet. Start a new conversation!'}
          </div>
        )}
      </div>
      
      {showNewChat && (
        <NewChatModal onClose={() => setShowNewChat(false)} />
      )}
    </div>
  );
};

export default ChatList;
