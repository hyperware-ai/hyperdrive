import React, { useState } from 'react';
import { AuthAnthropic } from '../auth/anthropic';
import { useSpiderStore } from '../store/spider';

interface ClaudeLoginProps {
  onSuccess?: () => void;
  onCancel?: () => void;
}

export const ClaudeLogin: React.FC<ClaudeLoginProps> = ({ onSuccess, onCancel }) => {
  const [isLoading, setIsLoading] = useState(false);
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [verifier, setVerifier] = useState<string | null>(null);
  const [authCode, setAuthCode] = useState('');
  const [error, setError] = useState<string | null>(null);
  
  const handleStartAuth = async () => {
    setIsLoading(true);
    setError(null);
    
    try {
      const { url, verifier: v } = await AuthAnthropic.authorize();
      setAuthUrl(url);
      setVerifier(v);
      
      // Open auth URL in new window
      window.open(url, '_blank', 'width=600,height=700');
    } catch (err) {
      setError('Failed to start authentication');
      console.error(err);
    } finally {
      setIsLoading(false);
    }
  };
  
  const handleExchangeCode = async () => {
    if (!authCode || !verifier) {
      setError('Please enter the authorization code');
      return;
    }
    
    setIsLoading(true);
    setError(null);
    
    try {
      const tokens = await AuthAnthropic.exchange(authCode, verifier);
      
      // Store tokens in localStorage
      localStorage.setItem('claude_oauth', JSON.stringify({
        refresh: tokens.refresh,
        access: tokens.access,
        expires: tokens.expires,
      }));
      
      // Store as API key in Spider
      const store = useSpiderStore.getState();
      await store.setApiKey('anthropic-oauth', tokens.access);
      
      onSuccess?.();
    } catch (err) {
      setError('Invalid authorization code. Please try again.');
      console.error(err);
    } finally {
      setIsLoading(false);
    }
  };
  
  
  return (
    <div className="claude-login">
      <div className="space-y-4">
        <div>
          <h3 className="text-lg font-semibold mb-2">Login with Claude</h3>
          <p className="text-sm text-gray-600 mb-4">
            Authenticate with your Claude.ai account to use your subscription for chat.
          </p>
        </div>
        
        
        {!authUrl ? (
          <div>
            <button
              onClick={handleStartAuth}
              disabled={isLoading}
              className="w-full bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600 disabled:opacity-50"
            >
              {isLoading ? 'Starting...' : 'Start Authentication'}
            </button>
          </div>
        ) : (
          <div className="space-y-4">
            <div className="bg-blue-50 p-4 rounded">
              <p className="text-sm">
                A new window has opened for authentication. After you authorize the app,
                you'll receive a code. Please paste it below:
              </p>
              <a 
                href={authUrl} 
                target="_blank" 
                rel="noopener noreferrer"
                className="text-blue-500 underline text-sm"
              >
                Click here if the window didn't open
              </a>
            </div>
            
            <div>
              <label className="block text-sm font-medium mb-2">
                Authorization Code:
              </label>
              <input
                type="text"
                value={authCode}
                onChange={(e) => setAuthCode(e.target.value)}
                placeholder="Paste the code here"
                className="w-full px-3 py-2 border border-gray-300 rounded focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
            
            <button
              onClick={handleExchangeCode}
              disabled={isLoading || !authCode}
              className="w-full bg-green-500 text-white px-4 py-2 rounded hover:bg-green-600 disabled:opacity-50"
            >
              {isLoading ? 'Verifying...' : 'Complete Authentication'}
            </button>
          </div>
        )}
        
        {error && (
          <div className="bg-red-50 text-red-600 p-3 rounded text-sm">
            {error}
          </div>
        )}
        
        {onCancel && (
          <button
            onClick={onCancel}
            className="w-full bg-gray-300 text-gray-700 px-4 py-2 rounded hover:bg-gray-400"
          >
            Cancel
          </button>
        )}
      </div>
    </div>
  );
};