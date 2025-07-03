import React from 'react';
import './Toolbar.css';

interface ToolbarProps {
  viewMode: 'list' | 'grid';
  onViewModeChange: (mode: 'list' | 'grid') => void;
  onNewMenu: () => void;
  onUploadFiles: () => void;
  onUploadFolder: () => void;
  onRefresh: () => void;
}

const Toolbar: React.FC<ToolbarProps> = ({
  viewMode,
  onViewModeChange,
  onNewMenu,
  onUploadFiles,
  onUploadFolder,
  onRefresh
}) => {
  const toggleViewMode = () => {
    onViewModeChange(viewMode === 'list' ? 'grid' : 'list');
  };

  return (
    <div className="toolbar">
      <div className="toolbar-group">
        <button onClick={onNewMenu} title="Create New" className="icon-button">
          ➕
        </button>
        <button onClick={onUploadFiles} title="Upload Files" className="icon-button">
          📄
        </button>
        <button onClick={onUploadFolder} title="Upload Folder" className="icon-button">
          📁
        </button>
        <button onClick={onRefresh} title="Refresh" className="icon-button">
          🔄
        </button>
      </div>
      
      <div className="toolbar-group">
        <button
          onClick={toggleViewMode}
          title={viewMode === 'list' ? 'Switch to Grid View' : 'Switch to List View'}
          className="icon-button"
        >
          {viewMode === 'list' ? '⊞' : '☰'}
        </button>
      </div>
    </div>
  );
};

export default Toolbar;