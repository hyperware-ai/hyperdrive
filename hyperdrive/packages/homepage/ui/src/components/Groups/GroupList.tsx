import React, { useEffect, useState } from 'react';
import { Chat as api } from '#caller-utils';
import { useGroupStore } from '../../store/groups';
import { useChatStore } from '../../store/chat';
import GroupListItem from './GroupListItem';
import GroupCreateModal from './GroupCreateModal';
import './GroupList.css';

const GroupList: React.FC = () => {
  const {
    groups,
    loadGroups,
    openGroup,
    activeGroupId,
    replication,
    isLoading,
    error,
    fetchReplicationState,
  } = useGroupStore();
  const { connectionStatus, searchIndex } = useChatStore();
  const [search, setSearch] = useState('');
  const [showCreate, setShowCreate] = useState(false);
  const [filteredGroups, setFilteredGroups] = useState(groups);

  useEffect(() => {
    if (connectionStatus === 'connected') {
      loadGroups();
      fetchReplicationState(null);
    }
  }, [connectionStatus]);

  useEffect(() => {
    let cancelled = false;

    const runSearch = async () => {
      if (!search.trim()) {
        setFilteredGroups(groups);
        return;
      }
      const results = await searchIndex(search, {
        scope: api.SearchScope.Groups,
        limit: 100,
      });
      if (cancelled) return;

      const rankByGroup = new Map<string, number>();
      results.forEach((result, idx) => {
        if (result.group_id && !rankByGroup.has(result.group_id)) {
          rankByGroup.set(result.group_id, idx);
        }
      });

      const matches = groups.filter((group) => rankByGroup.has(group.group_id));
      matches.sort(
        (a, b) =>
          (rankByGroup.get(a.group_id) ?? 0) -
          (rankByGroup.get(b.group_id) ?? 0),
      );
      setFilteredGroups(matches);
    };

    runSearch();
    return () => {
      cancelled = true;
    };
  }, [search, groups, searchIndex]);

  return (
    <div className="group-list-container">
      <div className="group-list-header">
        <div className="group-search">
          <input
            type="text"
            placeholder="Find a group..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <button
          className="group-refresh"
          onClick={() => {
            loadGroups();
            fetchReplicationState(null);
          }}
          disabled={connectionStatus !== 'connected'}
        >
          ⟳
        </button>
        <button
          className="group-create"
          onClick={() => setShowCreate(true)}
          disabled={connectionStatus !== 'connected'}
        >
          + New
        </button>
      </div>

      {error && <div className="group-error-banner">{error}</div>}

      <div className="group-list">
        {isLoading && groups.length === 0 ? (
          <div className="group-empty">Loading groups…</div>
        ) : filteredGroups.length > 0 ? (
          filteredGroups.map((group, index) => (
            <GroupListItem
              key={group.group_id}
              summary={group}
              replicationState={replication[group.group_id]}
              isActive={group.group_id === activeGroupId}
              onSelect={(id) => openGroup(id)}
              index={index}
            />
          ))
        ) : (
          <div className="group-empty">
            {search ? 'No groups match your search.' : 'No groups yet. Create one to get started.'}
          </div>
        )}
      </div>

      {showCreate && <GroupCreateModal onClose={() => setShowCreate(false)} />}
    </div>
  );
};

export default GroupList;
