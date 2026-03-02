import React, { useCallback, useEffect, useRef, useState } from 'react';
import '../Chat/FileUpload.css';
import { useGroupStore } from '../../store/groups';
import { getChatBasePath } from '../../utils/chatBase';
import { useChatStore } from '../../store/chat';

interface GroupFileUploadProps {
  onClose: () => void;
}

interface UploadStatus {
  filename: string;
  progress: number;
  error?: string;
}

interface SendGroupMessageResponse {
  message?: { message_id?: string };
}

const buildApiUrl = (path: string) => {
  const basePath = getChatBasePath();

  if (path.startsWith(`${basePath}/`)) {
    return path;
  }

  return `${basePath}${path}`;
};

const parseApiResponse = <T,>(response: any): T => {
  if (response && typeof response === 'object') {
    if ('Ok' in response && response.Ok !== undefined) {
      return response.Ok as T;
    }
    if ('Err' in response && response.Err !== undefined) {
      throw new Error(response.Err as string);
    }
  }
  return response as T;
};
const CLOSE_ANIMATION_MS = 220;

const GroupFileUpload: React.FC<GroupFileUploadProps> = ({ onClose }) => {
  const {
    activeGroupId,
    activeThreadId,
    draftThread,
    createThread,
    clearDraftThread,
    setActiveThread,
    refreshActiveGroup,
    replyingTo,
    setReplyingTo,
  } = useGroupStore();
  const { settings } = useChatStore();
  const [isUploading, setIsUploading] = useState(false);
  const [isClosing, setIsClosing] = useState(false);
  const [uploadStatus, setUploadStatus] = useState<Record<string, UploadStatus>>({});
  const closeTimeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (closeTimeoutRef.current !== null) {
        window.clearTimeout(closeTimeoutRef.current);
      }
    };
  }, []);

  const requestClose = useCallback(() => {
    if (isClosing) return;
    setIsClosing(true);
    closeTimeoutRef.current = window.setTimeout(() => {
      onClose();
    }, CLOSE_ANIMATION_MS);
  }, [isClosing, onClose]);

  const readFileAsBase64 = useCallback((file: File, fileKey: string): Promise<string> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      const filename = file.name || 'attachment';

      reader.onprogress = (event) => {
        if (event.lengthComputable) {
          const percentComplete = (event.loaded / event.total) * 50;
          setUploadStatus((prev) => ({
            ...prev,
            [fileKey]: {
              filename,
              progress: percentComplete,
              error: prev[fileKey]?.error,
            },
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

  const uploadFileToThread = useCallback(async (
    groupId: string,
    threadId: string,
    replyTo: string | null,
    file: File,
    base64: string,
  ): Promise<SendGroupMessageResponse> => {
    const response = await fetch(buildApiUrl('/api/upload-group-file'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        UploadGroupFile: {
          group_id: groupId,
          thread_id: threadId,
          reply_to: replyTo,
          filename: file.name || 'attachment',
          mime_type: file.type || 'application/octet-stream',
          data: base64,
        },
      }),
    });

    if (!response.ok) {
      throw new Error(`Upload failed with status ${response.status}`);
    }

    const json = await response.json();
    return parseApiResponse<SendGroupMessageResponse>(json);
  }, []);

  const uploadSingleFile = useCallback(
    async (file: File, maxSizeBytes: number, fileKey: string): Promise<boolean> => {
      const filename = file.name || 'attachment';

      if (!activeGroupId) {
        setUploadStatus((prev) => ({
          ...prev,
          [fileKey]: { filename, progress: 0, error: 'No active group' },
        }));
        return false;
      }

      if (file.size > maxSizeBytes) {
        const maxMb = Math.round(maxSizeBytes / (1024 * 1024));
        setUploadStatus((prev) => ({
          ...prev,
          [fileKey]: { filename, progress: 0, error: `Exceeds ${maxMb}MB limit` },
        }));
        return false;
      }

      setUploadStatus((prev) => ({ ...prev, [fileKey]: { filename, progress: 0 } }));

      try {
        const base64 = await readFileAsBase64(file, fileKey);
        setUploadStatus((prev) => ({ ...prev, [fileKey]: { filename, progress: 50 } }));

        let threadId = activeThreadId;
        const replyToId = replyingTo?.id ?? null;

        if (!threadId && draftThread) {
          const parentThreadId = draftThread.parentThreadId;
          const title =
            filename.length > 50 ? `${filename.substring(0, 50).trim()}…` : filename;

          if (!draftThread.rootMessageId) {
            const rootRes = await uploadFileToThread(
              activeGroupId,
              parentThreadId,
              replyToId,
              file,
              base64,
            );
            const rootMessageId = rootRes.message?.message_id;
            if (rootMessageId) {
              const newThreadId = await createThread(title, parentThreadId, rootMessageId);
              if (newThreadId) {
                clearDraftThread();
                setActiveThread(newThreadId);
              }
            }
            setUploadStatus((prev) => ({ ...prev, [fileKey]: { filename, progress: 100 } }));
            return true;
          }

          const newThreadId = await createThread(
            title,
            parentThreadId,
            draftThread.rootMessageId,
          );
          if (!newThreadId) {
            setUploadStatus((prev) => ({
              ...prev,
              [fileKey]: { filename, progress: 0, error: 'Failed to create thread' },
            }));
            return false;
          }
          clearDraftThread();
          setActiveThread(newThreadId);
          threadId = newThreadId;
        }

        if (!threadId) {
          setUploadStatus((prev) => ({
            ...prev,
            [fileKey]: { filename, progress: 0, error: 'Select a thread to share files' },
          }));
          return false;
        }

        await uploadFileToThread(activeGroupId, threadId, replyToId, file, base64);
        setUploadStatus((prev) => ({ ...prev, [fileKey]: { filename, progress: 100 } }));
        return true;
      } catch (error) {
        console.error('Error uploading group file:', error);
        setUploadStatus((prev) => ({
          ...prev,
          [fileKey]: { filename, progress: 0, error: 'Upload failed' },
        }));
        return false;
      }
    },
    [
      activeGroupId,
      activeThreadId,
      createThread,
      clearDraftThread,
      draftThread,
      readFileAsBase64,
      replyingTo?.id,
      setActiveThread,
      uploadFileToThread,
    ],
  );

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files || files.length === 0) return;

    const maxSizeBytes = (settings.max_file_size_mb || 10) * 1024 * 1024;
    const fileArray = Array.from(files).map((file, index) => ({
      file,
      key: `${file.name || 'attachment'}-${file.lastModified}-${index}`,
    }));

    setIsUploading(true);

    const results = await Promise.all(
      fileArray.map(({ file, key }) => uploadSingleFile(file, maxSizeBytes, key)),
    );

    const hasSuccess = results.some(Boolean);
    const hasErrors = results.some((result) => !result);

    if (hasSuccess) {
      await refreshActiveGroup();
      setReplyingTo(null);
    }

    setTimeout(() => {
      setIsUploading(false);
      if (!hasErrors) {
        requestClose();
      }
    }, 1000);
  };

  const handleMenuClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    const isActionClick = Boolean(target.closest('button, label, input'));
    e.stopPropagation();
    if (!isActionClick) {
      requestClose();
    }
  };

  return (
    <div
      className={`file-upload-overlay ${isClosing ? 'closing' : ''}`}
      onClick={requestClose}
    >
      <div className="file-upload-menu" onClick={handleMenuClick}>
        {Object.keys(uploadStatus).length > 0 && (
          <div className="upload-progress-container">
            {Object.entries(uploadStatus).map(([fileKey, status]) => (
              <div key={fileKey} className={`upload-progress-item ${status.error ? 'has-error' : ''}`}>
                <div className="upload-filename">{status.filename}</div>
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

        {!isUploading && (
          <>
            <button className="upload-option">
              <label htmlFor="group-file-input">
                Choose File
                <input
                  id="group-file-input"
                  type="file"
                  onChange={handleFileSelect}
                  style={{ display: 'none' }}
                  multiple
                />
              </label>
            </button>
            <button className="upload-option">
              <label htmlFor="group-image-input">
                Choose Image
                <input
                  id="group-image-input"
                  type="file"
                  accept="image/*"
                  onChange={handleFileSelect}
                  style={{ display: 'none' }}
                />
              </label>
            </button>
            <button className="upload-option" onClick={requestClose}>
              Cancel
            </button>
          </>
        )}
      </div>
    </div>
  );
};

export default GroupFileUpload;
