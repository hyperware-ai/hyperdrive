import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import fixWebmDuration from 'fix-webm-duration';
import { useChatStore } from '../../store/chat';
import './VoiceNote.css';

interface VoiceNoteProps {
  onClose: () => void;
  onSend: (payload: { base64: string; duration: number; mimeType: string }) => Promise<void>;
}

const CLOSE_ANIMATION_MS = 220;

const VoiceNote: React.FC<VoiceNoteProps> = ({ onClose, onSend }) => {
  const { settings } = useChatStore();
  const [isStarting, setIsStarting] = useState(true);
  const [isRecording, setIsRecording] = useState(false);
  const [recordingTime, setRecordingTime] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [isSending, setIsSending] = useState(false);
  const [isClosing, setIsClosing] = useState(false);
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const timerRef = useRef<number | null>(null);
  const closeTimeoutRef = useRef<number | null>(null);
  const startTimeRef = useRef<number>(0);
  const mimeTypeRef = useRef<string>('audio/webm');
  const maxSizeBytes = (settings?.max_file_size_mb || 10) * 1024 * 1024;
  const audioBitsPerSecond = 32000;

  const pickMimeType = () => {
    if (typeof MediaRecorder === 'undefined') {
      return '';
    }
    const candidates = ['audio/webm;codecs=opus', 'audio/webm'];
    return candidates.find((type) => MediaRecorder.isTypeSupported(type)) ?? '';
  };

  const blobToBase64 = (blob: Blob): Promise<string> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onloadend = () => {
        const result = reader.result;
        if (typeof result === 'string') {
          const base64 = result.split(',')[1];
          resolve(base64);
        } else {
          reject(new Error('Failed to read audio data'));
        }
      };
      reader.onerror = () => reject(new Error('Failed to read audio data'));
      reader.readAsDataURL(blob);
    });
  };

  const clearTimer = () => {
    if (timerRef.current) {
      window.clearInterval(timerRef.current);
      timerRef.current = null;
    }
  };

  const stopTracks = () => {
    streamRef.current?.getTracks().forEach((track) => track.stop());
    streamRef.current = null;
  };

  const requestClose = useCallback(() => {
    if (isClosing) return;
    setIsClosing(true);
    closeTimeoutRef.current = window.setTimeout(() => {
      onClose();
    }, CLOSE_ANIMATION_MS);
  }, [isClosing, onClose]);

  const startRecording = useCallback(async () => {
    setError(null);
    setIsStarting(true);

    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
      if (!window.isSecureContext) {
        setError('Microphone access requires HTTPS or localhost.');
      } else {
        setError('Audio recording is not supported in this browser.');
      }
      setIsStarting(false);
      return;
    }

    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      streamRef.current = stream;
      const mimeType = pickMimeType();
      if (!mimeType) {
        setError('Audio recording requires WebM support.');
        stopTracks();
        setIsStarting(false);
        return;
      }
      mimeTypeRef.current = mimeType;
      const recorder = new MediaRecorder(stream, {
        mimeType,
        audioBitsPerSecond,
      });
      mediaRecorderRef.current = recorder;
      chunksRef.current = [];

      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          chunksRef.current.push(event.data);
        }
      };

      recorder.start();
      startTimeRef.current = Date.now();
      setRecordingTime(0);
      setIsRecording(true);
      clearTimer();
      timerRef.current = window.setInterval(() => {
        const elapsed = Math.floor((Date.now() - startTimeRef.current) / 1000);
        setRecordingTime(elapsed);
      }, 1000);
      setIsStarting(false);
    } catch (err) {
      console.error('Failed to start recording:', err);
      setError('Microphone permission denied.');
      stopTracks();
      setIsStarting(false);
    }
  }, []);

  const finishRecording = useCallback(async (shouldSend: boolean): Promise<boolean> => {
    const recorder = mediaRecorderRef.current;
    if (!recorder) {
      setIsRecording(false);
      clearTimer();
      stopTracks();
      return !shouldSend;
    }

    const stopped = new Promise<Blob>((resolve) => {
      recorder.onstop = () => {
        const blob = new Blob(chunksRef.current, {
          type: mimeTypeRef.current || 'audio/webm',
        });
        stopTracks();
        resolve(blob);
      };
    });

    if (recorder.state === 'recording') {
      recorder.requestData();
    }
    recorder.stop();
    clearTimer();
    setIsRecording(false);

    const blob = await stopped;
    if (!shouldSend) {
      return true;
    }

    if (!blob.size) {
      setError('No audio captured.');
      return false;
    }
    if (blob.size > maxSizeBytes) {
      const maxMb = Math.round(maxSizeBytes / (1024 * 1024));
      setError(`Voice note exceeds ${maxMb}MB limit.`);
      return false;
    }

    try {
      setIsSending(true);
      const duration = Math.max(
        1,
        Math.round((Date.now() - startTimeRef.current) / 1000),
      );
      // Fix WebM duration metadata so audio player shows correct length
      const fixedBlob = await fixWebmDuration(blob, duration * 1000);
      const base64 = await blobToBase64(fixedBlob);
      const mimeType = (blob.type || mimeTypeRef.current || 'audio/webm').split(';')[0];
      await onSend({ base64, duration, mimeType });
      return true;
    } catch (err) {
      console.error('Failed to send voice note:', err);
      setError('Failed to send voice note.');
      return false;
    } finally {
      setIsSending(false);
    }
  }, [maxSizeBytes, onSend]);

  const sendRecording = useCallback(async () => {
    if (isSending || isClosing) return;
    if (!isRecording) {
      return;
    }
    const sent = await finishRecording(true);
    if (sent) {
      requestClose();
    }
  }, [finishRecording, isClosing, isRecording, isSending, requestClose]);

  const cancelRecording = useCallback(async () => {
    if (isSending || isClosing) return;
    await finishRecording(false);
    requestClose();
  }, [finishRecording, isClosing, isSending, requestClose]);

  useEffect(() => {
    startRecording();
    return () => {
      if (closeTimeoutRef.current !== null) {
        window.clearTimeout(closeTimeoutRef.current);
      }
      clearTimer();
      if (mediaRecorderRef.current?.state === 'recording') {
        mediaRecorderRef.current.stop();
      }
      stopTracks();
    };
  }, [startRecording]);

  const sheet = (
    <div
      className={`voice-note-overlay ${isClosing ? 'closing' : ''}`}
      onClick={sendRecording}
    >
      <div className="voice-note-sheet">
        <div className="recording-indicator">
          <span className="recording-dot"></span>
          {isSending ? 'Sending…' : `Recording ${recordingTime}s`}
        </div>
        <p className="voice-note-instruction">
          {isStarting ? 'Preparing microphone…' : 'Recording, tap anywhere to send'}
        </p>
        {error && <div className="voice-note-error">{error}</div>}
        <button
          className="cancel-button"
          onClick={(e) => {
            e.stopPropagation();
            void cancelRecording();
          }}
          disabled={isSending}
        >
          Cancel
        </button>
      </div>
    </div>
  );

  if (typeof document === 'undefined') {
    return null;
  }

  return createPortal(sheet, document.body);
};

export default VoiceNote;
