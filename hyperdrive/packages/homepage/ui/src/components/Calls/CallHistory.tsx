import React from 'react';
import './CallHistory.css';

const CallHistory: React.FC = () => {
  return (
    <div className="call-history-container">
      <div className="empty-state">
        <span className="empty-icon">📞</span>
        <h3>Voice Calls Coming Soon</h3>
        <p>Voice call functionality will be available in a future update.</p>
      </div>
    </div>
  );
};

export default CallHistory;