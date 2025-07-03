import React, { useState, useEffect } from 'react';
import { FileInfo, AuthScheme, shareFile } from '../../lib/api';
import useFileExplorerStore from '../../store/fileExplorer';
import QRCode from 'qrcode';
import './ShareDialog.css';

interface ShareDialogProps {
  file: FileInfo;
  onClose: () => void;
}

const ShareDialog: React.FC<ShareDialogProps> = ({ file, onClose }) => {
  const [authScheme, setAuthScheme] = useState<AuthScheme>("Public");
  const [shareLink, setShareLink] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [qrCode, setQrCode] = useState<string>('');
  const [copied, setCopied] = useState(false);
  const { addSharedLink } = useFileExplorerStore();

  // Auto-generate share link on mount
  useEffect(() => {
    handleShare();
  }, []);

  const handleShare = async () => {
    setLoading(true);
    try {
      const link = await shareFile(file.path, authScheme);
      const fullLink = `${window.location.origin}${link}`;
      setShareLink(fullLink);
      addSharedLink(file.path, fullLink);
      
      // Auto-copy to clipboard
      await navigator.clipboard.writeText(fullLink);
      setCopied(true);
      
      // Generate QR code
      const qrDataUrl = await QRCode.toDataURL(fullLink);
      setQrCode(qrDataUrl);
    } catch (err) {
      console.error('Failed to share file:', err);
    } finally {
      setLoading(false);
    }
  };

  const copyToClipboard = async () => {
    await navigator.clipboard.writeText(shareLink);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="share-dialog-overlay" onClick={onClose}>
      <div className="share-dialog" onClick={e => e.stopPropagation()}>
        <h3>Share File: {file.name}</h3>
        
        {loading ? (
          <div className="loading-container">
            <p>Generating share link...</p>
          </div>
        ) : shareLink ? (
          <>
            <div className="share-success-message">
              <p>✓ Share link created and copied to clipboard!</p>
            </div>

            <div className="share-link-container">
              <input
                type="text"
                value={shareLink}
                readOnly
                className="share-link-input"
              />
              <button onClick={copyToClipboard}>
                {copied ? '✓ Copied' : 'Copy'}
              </button>
            </div>

            {qrCode && (
              <div className="qr-container">
                <div className="qr-code">
                  <img src={qrCode} alt="QR Code" />
                </div>
              </div>
            )}
          </>
        ) : (
          <p>Failed to generate share link</p>
        )}

        <div className="dialog-actions">
          <button onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
};

export default ShareDialog;