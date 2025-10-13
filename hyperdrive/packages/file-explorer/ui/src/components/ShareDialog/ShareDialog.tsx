import React, { useState, useEffect, useRef } from 'react';
import { FileExplorer } from '../../lib/api';
import useFileExplorerStore from '../../store/fileExplorer';
import QRCode from 'qrcode';
import './ShareDialog.css';

interface ShareDialogProps {
  file: FileExplorer.FileInfo;
  onClose: () => void;
}

const ShareDialog: React.FC<ShareDialogProps> = ({ file, onClose }) => {
  const [authScheme, setAuthScheme] = useState<FileExplorer.AuthScheme>(FileExplorer.AuthScheme.Public);
  const [shareLink, setShareLink] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [qrCodeDataUrl, setQrCodeDataUrl] = useState<string>('');
  const [copied, setCopied] = useState(false);
  const { addSharedLink } = useFileExplorerStore();
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Detect if we're in iOS standalone mode (PWA)
  const isIosPwa = () => {
    return (window.navigator as any).standalone === true ||
           window.matchMedia('(display-mode: standalone)').matches;
  };

  // Auto-generate share link on mount
  useEffect(() => {
    handleShare();
  }, []);

  // Generate QR code when share link is available
  useEffect(() => {
    if (shareLink) {
      const generateQrCode = async () => {
        try {
          if (isIosPwa()) {
            // For iOS PWA, use canvas rendering
            if (canvasRef.current) {
              await QRCode.toCanvas(canvasRef.current, shareLink, {
                width: 200,
                margin: 2,
                color: {
                  dark: '#000000',
                  light: '#FFFFFF'
                }
              });
            }
          } else {
            // For desktop and regular mobile browsers, use data URL
            const dataUrl = await QRCode.toDataURL(shareLink, {
              width: 200,
              margin: 2,
              color: {
                dark: '#000000',
                light: '#FFFFFF'
              }
            });
            setQrCodeDataUrl(dataUrl);
          }
        } catch (err) {
          console.error('Failed to generate QR code:', err);
        }
      };

      generateQrCode();
    }
  }, [shareLink]);

  const handleShare = async () => {
    setLoading(true);
    try {
      const link = await FileExplorer.share_file(file.path, authScheme);

      // Remove first element of origin (e.g., http://foo.bar.com -> http://bar.com)
      let origin = window.location.origin;
      const urlParts = new URL(origin);
      const hostParts = urlParts.hostname.split('.');
      if (hostParts.length > 2) {
        // Remove the first subdomain
        hostParts.shift();
        urlParts.hostname = hostParts.join('.');
        origin = urlParts.toString().replace(/\/$/, ''); // Remove trailing slash
      }

      const fullLink = `${origin}${link}`;
      setShareLink(fullLink);
      addSharedLink(file.path, fullLink);

      // Auto-copy to clipboard with fallback
      try {
        await navigator.clipboard.writeText(fullLink);
        setCopied(true);
      } catch (clipboardErr) {
        // Fallback for when clipboard API fails
        const textArea = document.createElement('textarea');
        textArea.value = fullLink;
        textArea.style.position = 'fixed';
        textArea.style.left = '-999999px';
        document.body.appendChild(textArea);
        textArea.focus();
        textArea.select();
        try {
          document.execCommand('copy');
          setCopied(true);
        } catch (e) {
          console.error('Failed to copy to clipboard:', e);
        }
        document.body.removeChild(textArea);
      }
    } catch (err) {
      console.error('Failed to share file:', err);
    } finally {
      setLoading(false);
    }
  };

  const copyToClipboard = async () => {
    try {
      await navigator.clipboard.writeText(shareLink);
      setCopied(true);
    } catch (clipboardErr) {
      // Fallback for when clipboard API fails
      const textArea = document.createElement('textarea');
      textArea.value = shareLink;
      textArea.style.position = 'fixed';
      textArea.style.left = '-999999px';
      document.body.appendChild(textArea);
      textArea.focus();
      textArea.select();
      try {
        document.execCommand('copy');
        setCopied(true);
      } catch (e) {
        console.error('Failed to copy to clipboard:', e);
      }
      document.body.removeChild(textArea);
    }
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

            {shareLink && (
              <div className="qr-container">
                <div className="qr-code">
                  {isIosPwa() ? (
                    <canvas ref={canvasRef} width="200" height="200" />
                  ) : (
                    qrCodeDataUrl && <img src={qrCodeDataUrl} alt="QR Code" />
                  )}
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
