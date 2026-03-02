import React, { useRef, useState, useEffect } from 'react';
import fixWebmDuration from 'fix-webm-duration';
import { useChatStore } from '../../store/chat';
import './VoiceNote.css';

interface VoiceNoteProps {
  onClose: () => void;
  onSend: (payload: { base64: string; duration: number; mimeType: string }) => Promise<void>;
}

const VoiceNote: React.FC<VoiceNoteProps> = ({ onClose, onSend }) => {
  const { settings } = useChatStore();
  const [isRecording, setIsRecording] = useState(false);
  const [recordingTime, setRecordingTime] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [isSending, setIsSending] = useState(false);
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const timerRef = useRef<number | null>(null);
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

  const startRecording = async () => {
    setError(null);

    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
      if (!window.isSecureContext) {
        setError('Microphone access requires HTTPS or localhost.');
      } else {
        setError('Audio recording is not supported in this browser.');
      }
      return;
    }

    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      streamRef.current = stream;
      const mimeType = pickMimeType();
      if (!mimeType) {
        setError('Audio recording requires WebM support.');
        stopTracks();
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
    } catch (err) {
      console.error('Failed to start recording:', err);
      setError('Microphone permission denied.');
      stopTracks();
    }
  };

  const finishRecording = async (shouldSend: boolean) => {
    const recorder = mediaRecorderRef.current;
    if (!recorder) {
      setIsRecording(false);
      clearTimer();
      stopTracks();
      return;
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
      return;
    }

    if (!blob.size) {
      setError('No audio captured.');
      return;
    }
    if (blob.size > maxSizeBytes) {
      const maxMb = Math.round(maxSizeBytes / (1024 * 1024));
      setError(`Voice note exceeds ${maxMb}MB limit.`);
      return;
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
      onClose();
    } catch (err) {
      console.error('Failed to send voice note:', err);
      setError('Failed to send voice note.');
    } finally {
      setIsSending(false);
    }
  };

  const stopRecording = async () => {
    await finishRecording(true);
  };

  const cancelRecording = async () => {
    await finishRecording(false);
    onClose();
  };

  useEffect(() => {
    return () => {
      clearTimer();
      if (mediaRecorderRef.current?.state === 'recording') {
        mediaRecorderRef.current.stop();
      }
      stopTracks();
    };
  }, []);

  return (
    <div className="voice-note-overlay" onClick={onClose}>
      <div className="voice-note-modal" onClick={(e) => e.stopPropagation()}>
        {isRecording ? (
          <>
            <div className="recording-indicator">
              <span className="recording-dot"></span>
              Recording... {recordingTime}s
            </div>
            {error && <div className="voice-note-error">{error}</div>}
            <button className="stop-button" onClick={stopRecording} disabled={isSending}>
              {isSending ? 'Sending…' : 'Stop & Send'}
            </button>
          </>
        ) : (
          <>
            <p>Tap to record a voice note</p>
            {error && <div className="voice-note-error">{error}</div>}
            <button className="record-button" onClick={startRecording} disabled={isSending}>
              <span className="material-symbols-outlined" aria-hidden="true">
                mic
              </span>
              <span>Start Recording</span>
            </button>
          </>
        )}
        <button className="cancel-button" onClick={isRecording ? cancelRecording : onClose}>
          Cancel
        </button>
      </div>
    </div>
  );
};

export default VoiceNote;
