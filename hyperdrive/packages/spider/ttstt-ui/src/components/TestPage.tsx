import React, { useState } from 'react';
import useTtsttStore from '../store/ttstt';

function TestPage() {
  const {
    testTts,
    startRecording,
    stopRecording,
    isRecording,
    audioUrl,
    transcribedText,
    isLoading,
    error,
  } = useTtsttStore();

  // TTS state
  const [ttsText, setTtsText] = useState('');

  const handleTts = async (e: React.FormEvent) => {
    e.preventDefault();
    if (ttsText.trim()) {
      await testTts(ttsText);
    }
  };

  // Handle recording toggle
  const handleRecordingToggle = async () => {
    if (isRecording) {
      await stopRecording();
    } else {
      await startRecording();
    }
  };

  return (
    <div className="test-page">
      {/* TTS Test Section */}
      <section className="card">
        <h2>Text-to-Speech Test</h2>
        <form onSubmit={handleTts} className="form-group">
          <textarea
            value={ttsText}
            onChange={(e) => setTtsText(e.target.value)}
            placeholder="Enter text to convert to speech..."
            rows={4}
            disabled={isLoading}
            className="mb-3"
          />
          <button type="submit" disabled={isLoading || !ttsText.trim()} className="primary">
            {isLoading ? 'Processing...' : 'Convert to Speech'}
          </button>
        </form>

        {audioUrl && (
          <div className="mt-4">
            <h3 className="mb-2">Generated Audio:</h3>
            <audio controls src={audioUrl} style={{ width: '100%' }} />
          </div>
        )}
      </section>

      {/* STT Test Section */}
      <section className="card">
        <h2>Speech-to-Text Test</h2>
        <div className="flex justify-center gap-3 mb-4">
          <button
            className={`${isRecording ? 'danger' : 'primary'}`}
            onClick={handleRecordingToggle}
            disabled={isLoading}
            style={{ padding: '1rem 2rem', fontSize: '1rem' }}
          >
            {isLoading ? 'Processing...' : isRecording ? '⏹️ Stop Recording' : '🎙️ Start Recording'}
          </button>
          {isRecording && (
            <button
              className="primary"
              onClick={async () => {
                await stopRecording();
              }}
              disabled={isLoading}
              style={{ padding: '1rem 2rem', fontSize: '1rem' }}
            >
              📤 Send Recording
            </button>
          )}
        </div>

        {transcribedText && (
          <div className="card" style={{ background: 'var(--background)', marginTop: '1rem' }}>
            <h3 className="mb-2">Transcribed Text:</h3>
            <p style={{ fontSize: '1.1rem', lineHeight: '1.6' }}>{transcribedText}</p>
          </div>
        )}
      </section>

      {error && (
        <div className="error">
          <strong>Error:</strong> {error}
        </div>
      )}
    </div>
  );
}

export default TestPage;