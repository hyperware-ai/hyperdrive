import React, { useState, useRef, useCallback } from 'react';
import './FileUpload.css';
import { useChatStore } from '../../store/chat';
import * as Caller from '#caller-utils';

interface FileUploadProps {
  onClose: () => void;
}

interface UploadStatus {
  progress: number;
  error?: string;
}

const { upload_file } = Caller.Chat;

const FileUpload: React.FC<FileUploadProps> = ({ onClose }) => {
  const { activeChat, settings } = useChatStore();
  const [isUploading, setIsUploading] = useState(false);
  const [uploadStatus, setUploadStatus] = useState<{ [filename: string]: UploadStatus }>({});
  const pendingUploadsRef = useRef(0);

  const readFileAsBase64 = useCallback((file: File): Promise<string> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();

      reader.onprogress = (event) => {
        if (event.lengthComputable) {
          const percentComplete = (event.loaded / event.total) * 50; // 50% for reading
          setUploadStatus(prev => ({
            ...prev,
            [file.name]: { ...prev[file.name], progress: percentComplete }
          }));
        }
      };

      reader.onload = (event) => {
        if (event.target?.result) {
          const base64 = (event.target.result as string).split(',')[1];
          resolve(base64);
        } else {
          reject(new Error('Failed to read file'));
        }
      };

      reader.onerror = () => reject(new Error('Failed to read file'));
      reader.readAsDataURL(file);
    });
  }, []);

  const uploadSingleFile = useCallback(async (file: File, chatId: string, maxSizeBytes: number) => {
    const filename = file.name;

    // Check file size
    if (file.size > maxSizeBytes) {
      const maxMb = Math.round(maxSizeBytes / (1024 * 1024));
      setUploadStatus(prev => ({
        ...prev,
        [filename]: { progress: 0, error: `Exceeds ${maxMb}MB limit` }
      }));
      return;
    }

    // Set initial progress
    setUploadStatus(prev => ({ ...prev, [filename]: { progress: 0 } }));

    try {
      const base64 = await readFileAsBase64(file);

      // Update progress to show uploading
      setUploadStatus(prev => ({ ...prev, [filename]: { progress: 50 } }));

      // Upload file
      await upload_file({
        chat_id: chatId,
        filename,
        mime_type: file.type || 'application/octet-stream',
        data: base64,
        reply_to: null
      });

      // Mark as complete
      setUploadStatus(prev => ({ ...prev, [filename]: { progress: 100 } }));
    } catch (error) {
      console.error('Error uploading file:', error);
      setUploadStatus(prev => ({
        ...prev,
        [filename]: { progress: 0, error: 'Upload failed' }
      }));
    }
  }, [readFileAsBase64]);

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files || files.length === 0 || !activeChat) return;

    const maxSizeBytes = (settings.max_file_size_mb || 10) * 1024 * 1024;
    const fileArray = Array.from(files);

    setIsUploading(true);
    pendingUploadsRef.current = fileArray.length;

    // Upload all files in parallel
    await Promise.all(
      fileArray.map(file => uploadSingleFile(file, activeChat.id, maxSizeBytes))
    );

    // Close dialog after a short delay to show completion
    setTimeout(() => {
      setIsUploading(false);
      // Check if all uploads succeeded (no errors)
      const hasErrors = Object.values(uploadStatus).some(s => s.error);
      if (!hasErrors) {
        onClose();
      }
    }, 1000);
  };

  return (
    <div className="file-upload-overlay" onClick={onClose}>
      <div className="file-upload-menu" onClick={(e) => e.stopPropagation()}>
        {/* Show upload progress if uploading */}
        {Object.keys(uploadStatus).length > 0 && (
          <div className="upload-progress-container">
            {Object.entries(uploadStatus).map(([filename, status]) => (
              <div key={filename} className={`upload-progress-item ${status.error ? 'has-error' : ''}`}>
                <div className="upload-filename">{filename}</div>
                {status.error ? (
                  <div className="upload-error">{status.error}</div>
                ) : (
                  <>
                    <div className="upload-progress-bar">
                      <div
                        className="upload-progress-fill"
                        style={{ width: `${status.progress}%` }}
                      />
                    </div>
                    <div className="upload-progress-text">{Math.round(status.progress)}%</div>
                  </>
                )}
              </div>
            ))}
          </div>
        )}
        
        {/* Show upload options when not uploading */}
        {!isUploading && (
          <>
            <button className="upload-option">
              <label htmlFor="file-input">
                <span className="material-symbols-outlined" aria-hidden="true">
                  attach_file
                </span>
                <span>Choose File</span>
                <input
                  id="file-input"
                  type="file"
                  onChange={handleFileSelect}
                  style={{ display: 'none' }}
                  multiple
                />
              </label>
            </button>
            <button className="upload-option">
              <label htmlFor="image-input">
                <span className="material-symbols-outlined" aria-hidden="true">
                  image
                </span>
                <span>Choose Image</span>
                <input
                  id="image-input"
                  type="file"
                  accept="image/*"
                  onChange={handleFileSelect}
                  style={{ display: 'none' }}
                />
              </label>
            </button>
            <button className="upload-option" onClick={onClose}>
              Cancel
            </button>
          </>
        )}
      </div>
    </div>
  );
};

export default FileUpload;
