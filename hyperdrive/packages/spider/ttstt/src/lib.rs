// TTSTT - Text-to-Speech & Speech-to-Text Wrapper
// Provides a unified interface for multiple TTS/STT providers

use hyperprocess_macro::*;
use hyperware_process_lib::{
    homepage::add_to_homepage,
    hyperapp::SaveOptions,
    our,
    vfs::{
        create_drive,
        directory::directory_async::open_dir_async,
        file::file_async::{create_file_async, open_file_async},
    },
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Import OpenAI clients
use hyperware_openai_stt::{client::TranscriptionClient, types::Model as OpenAISttModel};
use hyperware_openai_tts::{
    client::SpeechClient,
    types::{AudioFormat as OpenAIAudioFormat, TtsModel as OpenAITtsModel, Voice as OpenAIVoice},
};

// Import ElevenLabs clients
use hyperware_elevenlabs_tts::{
    AudioFormat as ElevenLabsAudioFormat, SpeechClient as ElevenLabsSpeechClient,
    TtsModel as ElevenLabsTtsModel, Voice as ElevenLabsVoice,
};

// Provider abstraction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Provider {
    #[serde(rename = "OpenAI")]
    OpenAI,
    #[serde(rename = "ElevenLabs")]
    ElevenLabs,
    // Future: PlayAI, Groq
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderConfig {
    provider: Provider,
    api_key: String,
    is_default_tts: bool,
    is_default_stt: bool,
    default_voice: Option<String>,
    default_speed: Option<f32>,
}

// TTS Types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TtsReq {
    text: String,
    provider: Option<Provider>,
    voice: Option<String>,
    model: Option<String>,
    format: Option<String>,
    speed: Option<f32>,
    api_key: Option<String>, // For request authentication
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TtsRes {
    audio_data: String, // Base64 encoded
    format: String,
    provider: Provider,
}

// STT Types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SttReq {
    audio_data: String, // Base64 encoded
    provider: Option<Provider>,
    model: Option<String>,
    language: Option<String>,
    api_key: Option<String>, // For request authentication
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SttRes {
    text: String,
    provider: Provider,
}

// Storage Types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RequestType {
    TTS,
    STT,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioTextPair {
    id: String,
    text: String,
    audio_data: String, // Base64 encoded
    audio_format: String,
    provider: Provider,
    timestamp: String,
    request_type: RequestType,
    metadata: Vec<(String, String)>, // Using Vec instead of HashMap for WIT compatibility
}

// API Key Management
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ApiKeyRole {
    Admin,
    Requestor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiKey {
    key: String,
    role: ApiKeyRole,
    created_at: String,
    name: String,
}

// Request/Response types for endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTtsReq {
    text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSttReq {
    audio_data: String, // Field name matches frontend "audioData" -> "audio_data"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddProviderReq {
    api_key: Option<String>,
    config: ProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveProviderReq {
    api_key: Option<String>,
    provider: Provider,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetDefaultProviderReq {
    api_key: Option<String>,
    provider: Provider,
    provider_type: String, // "tts" or "stt"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateApiKeyReq {
    api_key: Option<String>,
    name: String,
    role: ApiKeyRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateApiKeyRes {
    key: String,
    name: String,
    role: ApiKeyRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeApiKeyReq {
    api_key: Option<String>,
    key_to_revoke: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListApiKeysReq {
    api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    name: String,
    role: ApiKeyRole,
    created_at: String,
    key_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    provider: Provider,
    is_default_tts: bool,
    is_default_stt: bool,
    default_voice: Option<String>,
    default_speed: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetHistoryReq {
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAudioTextPairReq {
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAdminKeyRes {
    admin_key: String,
    message: String,
}

// App State
#[derive(Default, Serialize, Deserialize)]
pub struct TtsttState {
    // Provider configurations
    providers: Vec<ProviderConfig>,

    // TTSTT API keys
    api_keys: Vec<ApiKey>,

    // Settings
    default_tts_provider: Option<Provider>,
    default_stt_provider: Option<Provider>,

    // Admin key (generated on first init)
    admin_key: String,

    // Storage path for audio-text pairs
    storage_initialized: bool,
}

// Helper methods (outside of hyperprocess impl block)
impl TtsttState {
    // Helper: Validate API key and check permissions
    fn validate_api_key(&self, api_key: Option<String>, require_admin: bool) -> Result<(), String> {
        // Allow requests from spider without authentication
        let source_addr = hyperware_process_lib::hyperapp::source();
        if source_addr.process() == "spider"
            && source_addr.package() == "spider"
            && source_addr.publisher() == "sys"
        {
            return Ok(());
        }

        let key = api_key.ok_or("API key required")?;

        let api_key_entry = self
            .api_keys
            .iter()
            .find(|k| k.key == key)
            .ok_or("Invalid API key")?;

        if require_admin && !matches!(api_key_entry.role, ApiKeyRole::Admin) {
            return Err("Admin permission required".to_string());
        }

        Ok(())
    }

    // Helper: Get provider config
    fn get_provider_config(&self, provider: &Provider) -> Result<&ProviderConfig, String> {
        self.providers
            .iter()
            .find(|p| p.provider == *provider)
            .ok_or_else(|| format!("Provider {:?} not configured", provider))
    }

    // OpenAI TTS implementation
    async fn handle_openai_tts(&self, request: TtsReq) -> Result<TtsRes, String> {
        let config = self.get_provider_config(&Provider::OpenAI)?;

        // Create OpenAI TTS client
        let client = SpeechClient::new(&config.api_key);

        // Map voice string to OpenAI voice enum, use provider default if not specified
        let voice_str = request
            .voice
            .as_deref()
            .or(config.default_voice.as_deref())
            .unwrap_or("nova");

        let voice = match voice_str {
            "alloy" => OpenAIVoice::Alloy,
            "ash" => OpenAIVoice::Ash,
            "ballad" => OpenAIVoice::Ballad,
            "coral" => OpenAIVoice::Coral,
            "echo" => OpenAIVoice::Echo,
            "fable" => OpenAIVoice::Fable,
            "onyx" => OpenAIVoice::Onyx,
            "nova" => OpenAIVoice::Nova,
            "sage" => OpenAIVoice::Sage,
            "shimmer" => OpenAIVoice::Shimmer,
            "verse" => OpenAIVoice::Verse,
            _ => OpenAIVoice::Nova, // Default to Nova
        };

        // Map model string to OpenAI model enum
        let model = match request.model.as_deref() {
            Some("tts-1") => OpenAITtsModel::Tts1,
            Some("tts-1-hd") => OpenAITtsModel::Tts1Hd,
            Some("gpt-4o-mini-tts") => OpenAITtsModel::Gpt4oMiniTts,
            _ => OpenAITtsModel::Gpt4oMiniTts, // Default to gpt-4o-mini-tts
        };

        // Map format string to OpenAI format enum
        let format = match request.format.as_deref() {
            Some("mp3") => OpenAIAudioFormat::Mp3,
            Some("opus") => OpenAIAudioFormat::Opus,
            Some("aac") => OpenAIAudioFormat::Aac,
            Some("flac") => OpenAIAudioFormat::Flac,
            Some("wav") => OpenAIAudioFormat::Wav,
            Some("pcm") => OpenAIAudioFormat::Pcm,
            _ => OpenAIAudioFormat::Mp3, // Default
        };

        // Build and execute request
        let mut builder = client
            .synthesize()
            .model(model)
            .voice(voice)
            .input(request.text.clone())
            .response_format(format);

        // Set speed: use request speed, then provider default, then 1.5
        let speed = request.speed.or(config.default_speed).unwrap_or(1.5);
        builder = builder.speed(speed);

        let response = builder
            .execute()
            .await
            .map_err(|e| format!("OpenAI TTS error: {:?}", e))?;

        Ok(TtsRes {
            audio_data: BASE64.encode(&response.audio_data),
            format: request.format.unwrap_or("mp3".to_string()),
            provider: Provider::OpenAI,
        })
    }

    // VFS Storage helpers
    async fn ensure_storage_initialized(&mut self) -> Result<(), String> {
        if self.storage_initialized {
            return Ok(());
        }

        // Create a drive for audio_pairs storage
        match create_drive(our().package_id(), "audio_pairs", Some(5)) {
            Ok(drive_path) => {
                println!("Created audio_pairs drive at: {}", drive_path);
            }
            Err(e) => {
                // Drive might already exist, which is fine
                println!("Note: audio_pairs drive may already exist: {:?}", e);
            }
        }

        self.storage_initialized = true;
        Ok(())
    }

    async fn save_audio_text_pair(&self, pair: &AudioTextPair) -> Result<(), String> {
        let base_path = format!("/{}/audio_pairs/{}", our().package_id(), pair.id);

        // Create directory for this pair
        open_dir_async(&base_path, true, Some(5))
            .await
            .map_err(|e| format!("Failed to create pair directory: {:?}", e))?;

        // Create metadata struct (without audio data to keep it small)
        #[derive(Serialize)]
        struct PairMetadata<'a> {
            id: &'a str,
            text: &'a str,
            audio_format: &'a str,
            provider: &'a Provider,
            timestamp: &'a str,
            request_type: &'a RequestType,
            metadata: &'a Vec<(String, String)>,
        }

        let metadata = PairMetadata {
            id: &pair.id,
            text: &pair.text,
            audio_format: &pair.audio_format,
            provider: &pair.provider,
            timestamp: &pair.timestamp,
            request_type: &pair.request_type,
            metadata: &pair.metadata,
        };

        let metadata_path = format!("{}/metadata.json", base_path);
        let metadata_file = create_file_async(&metadata_path, Some(5))
            .await
            .map_err(|e| format!("Failed to create metadata file: {:?}", e))?;

        let metadata_json = serde_json::to_string(&metadata)
            .map_err(|e| format!("Failed to serialize metadata: {}", e))?;

        metadata_file
            .write(metadata_json.as_bytes())
            .await
            .map_err(|e| format!("Failed to write metadata: {:?}", e))?;

        // Save audio data
        let audio_ext = match pair.audio_format.as_str() {
            "webm" => "webm",
            "mp3" => "mp3",
            _ => "audio",
        };
        let audio_path = format!("{}/audio.{}", base_path, audio_ext);
        let audio_file = create_file_async(&audio_path, Some(5))
            .await
            .map_err(|e| format!("Failed to create audio file: {:?}", e))?;

        // Decode base64 and write raw audio
        let audio_bytes = BASE64
            .decode(&pair.audio_data)
            .map_err(|e| format!("Failed to decode audio data: {}", e))?;

        audio_file
            .write(&audio_bytes)
            .await
            .map_err(|e| format!("Failed to write audio: {:?}", e))?;

        Ok(())
    }

    async fn load_audio_text_pairs(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AudioTextPair>, String> {
        let base_path = format!("/{}/audio_pairs", our().package_id());

        // Open directory
        let dir = open_dir_async(&base_path, false, Some(5))
            .await
            .map_err(|e| format!("Failed to open storage directory: {:?}", e))?;

        // Read directory entries
        let entries = dir
            .read()
            .await
            .map_err(|e| format!("Failed to read directory: {:?}", e))?;

        // Sort by name (which includes timestamp) in reverse for most recent first
        let mut pair_dirs: Vec<_> = entries
            .into_iter()
            .filter(|e| e.file_type == hyperware_process_lib::vfs::FileType::Directory)
            .collect();
        pair_dirs.sort_by(|a, b| b.path.cmp(&a.path));

        // Apply pagination
        let paginated: Vec<_> = pair_dirs.into_iter().skip(offset).take(limit).collect();

        // Load each pair
        let mut pairs = Vec::new();
        for entry in paginated {
            match self.load_audio_text_pair_by_path(&entry.path).await {
                Ok(pair) => pairs.push(pair),
                Err(e) => eprintln!("Failed to load pair from {}: {}", entry.path, e),
            }
        }

        Ok(pairs)
    }

    async fn load_audio_text_pair_by_id(&self, id: &str) -> Result<AudioTextPair, String> {
        let path = format!("/{}/audio_pairs/{}", our().package_id(), id);
        self.load_audio_text_pair_by_path(&path).await
    }

    async fn load_audio_text_pair_by_path(&self, path: &str) -> Result<AudioTextPair, String> {
        // Define metadata struct for deserialization
        #[derive(Deserialize)]
        struct PairMetadata {
            id: String,
            text: String,
            audio_format: String,
            provider: Provider,
            timestamp: String,
            request_type: RequestType,
            metadata: Vec<(String, String)>,
        }

        // Load metadata
        let metadata_path = format!("{}/metadata.json", path);
        let metadata_file = open_file_async(&metadata_path, false, Some(5))
            .await
            .map_err(|e| format!("Failed to open metadata file: {:?}", e))?;

        let metadata_str = metadata_file
            .read_to_string()
            .await
            .map_err(|e| format!("Failed to read metadata: {:?}", e))?;

        let metadata: PairMetadata = serde_json::from_str(&metadata_str)
            .map_err(|e| format!("Failed to parse metadata: {}", e))?;

        // Determine audio file extension
        let audio_ext = match metadata.audio_format.as_str() {
            "webm" => "webm",
            "mp3" => "mp3",
            _ => "audio",
        };

        // Load audio data
        let audio_path = format!("{}/audio.{}", path, audio_ext);
        let audio_file = open_file_async(&audio_path, false, Some(5))
            .await
            .map_err(|e| format!("Failed to open audio file: {:?}", e))?;

        let audio_bytes = audio_file
            .read()
            .await
            .map_err(|e| format!("Failed to read audio: {:?}", e))?;

        // Encode audio to base64
        let audio_data = BASE64.encode(&audio_bytes);

        // Construct AudioTextPair
        Ok(AudioTextPair {
            id: metadata.id,
            text: metadata.text,
            audio_data,
            audio_format: metadata.audio_format,
            provider: metadata.provider,
            timestamp: metadata.timestamp,
            request_type: metadata.request_type,
            metadata: metadata.metadata,
        })
    }

    // ElevenLabs TTS implementation
    async fn handle_elevenlabs_tts(&self, request: TtsReq) -> Result<TtsRes, String> {
        let config = self.get_provider_config(&Provider::ElevenLabs)?;

        // Create ElevenLabs TTS client
        let client = ElevenLabsSpeechClient::new(&config.api_key);

        // Map voice string to ElevenLabs voice enum, use provider default if not specified
        let voice_str = request
            .voice
            .as_deref()
            .or(config.default_voice.as_deref())
            .unwrap_or("Rachel");

        let voice = match voice_str.to_lowercase().as_str() {
            "rachel" => ElevenLabsVoice::Rachel,
            "drew" => ElevenLabsVoice::Drew,
            "clyde" => ElevenLabsVoice::Clyde,
            "paul" => ElevenLabsVoice::Paul,
            "aria" => ElevenLabsVoice::Aria,
            "domi" => ElevenLabsVoice::Domi,
            "dave" => ElevenLabsVoice::Dave,
            "roger" => ElevenLabsVoice::Roger,
            "fin" => ElevenLabsVoice::Fin,
            "sarah" => ElevenLabsVoice::Sarah,
            _ => ElevenLabsVoice::Rachel, // Default to Rachel
        };

        // Map model string to ElevenLabs model enum
        let model = match request.model.as_deref() {
            Some("eleven_v3") => ElevenLabsTtsModel::ElevenV3,
            Some("eleven_multilingual_v2") => ElevenLabsTtsModel::ElevenMultilingualV2,
            Some("eleven_flash_v2_5") => ElevenLabsTtsModel::ElevenFlashV25,
            Some("eleven_turbo_v2_5") => ElevenLabsTtsModel::ElevenTurboV25,
            _ => ElevenLabsTtsModel::ElevenMultilingualV2, // Default
        };

        // Map format string to ElevenLabs format enum (clone for later use)
        let format = match request.format.as_deref() {
            Some("mp3") | Some("mp3_44100_128") => ElevenLabsAudioFormat::Mp3_44100_128,
            Some("mp3_22050_32") => ElevenLabsAudioFormat::Mp3_22050_32,
            Some("mp3_44100_32") => ElevenLabsAudioFormat::Mp3_44100_32,
            Some("mp3_44100_64") => ElevenLabsAudioFormat::Mp3_44100_64,
            Some("mp3_44100_96") => ElevenLabsAudioFormat::Mp3_44100_96,
            Some("mp3_44100_192") => ElevenLabsAudioFormat::Mp3_44100_192,
            Some("pcm") | Some("pcm_44100") => ElevenLabsAudioFormat::Pcm44100,
            Some("pcm_16000") => ElevenLabsAudioFormat::Pcm16000,
            Some("pcm_22050") => ElevenLabsAudioFormat::Pcm22050,
            Some("pcm_24000") => ElevenLabsAudioFormat::Pcm24000,
            Some("ulaw") | Some("ulaw_8000") => ElevenLabsAudioFormat::Ulaw8000,
            _ => ElevenLabsAudioFormat::Mp3_44100_128, // Default
        };

        // Build and execute request
        let mut builder = client
            .synthesize()
            .text(request.text.clone())
            .model(model)
            .voice(voice)
            .output_format(format.clone());

        // Set speed if provided (ElevenLabs doesn't have a direct speed parameter,
        // but we can map it to stability/similarity_boost for similar effect)
        if let Some(speed) = request.speed.or(config.default_speed) {
            // Map speed (0.25-4.0 for OpenAI) to stability/similarity (0.0-1.0 for ElevenLabs)
            // Higher speed = lower stability for more dynamic speech
            let stability = (2.0 - speed.min(2.0).max(0.5)) / 1.5; // Maps speed 0.5-2.0 to stability 1.0-0.0
            let similarity = 0.75; // Keep similarity consistent for clarity

            builder = builder.stability(stability).similarity_boost(similarity);
        }

        let response = builder
            .execute()
            .await
            .map_err(|e| format!("ElevenLabs TTS error: {:?}", e))?;

        // Determine the actual format string for the response
        let format_str = match format {
            ElevenLabsAudioFormat::Mp3_22050_32 => "mp3",
            ElevenLabsAudioFormat::Mp3_44100_32 => "mp3",
            ElevenLabsAudioFormat::Mp3_44100_64 => "mp3",
            ElevenLabsAudioFormat::Mp3_44100_96 => "mp3",
            ElevenLabsAudioFormat::Mp3_44100_128 => "mp3",
            ElevenLabsAudioFormat::Mp3_44100_192 => "mp3",
            ElevenLabsAudioFormat::Pcm16000 => "pcm",
            ElevenLabsAudioFormat::Pcm22050 => "pcm",
            ElevenLabsAudioFormat::Pcm24000 => "pcm",
            ElevenLabsAudioFormat::Pcm44100 => "pcm",
            ElevenLabsAudioFormat::Ulaw8000 => "ulaw",
        };

        Ok(TtsRes {
            audio_data: BASE64.encode(&response.audio_data),
            format: format_str.to_string(),
            provider: Provider::ElevenLabs,
        })
    }

    // OpenAI STT implementation
    async fn handle_openai_stt(&self, request: SttReq) -> Result<SttRes, String> {
        let config = self.get_provider_config(&Provider::OpenAI)?;

        // Create OpenAI STT client
        let client = TranscriptionClient::new(&config.api_key);

        // Decode base64 audio data
        let audio_data = BASE64
            .decode(&request.audio_data)
            .map_err(|e| format!("Failed to decode audio data: {}", e))?;

        // Map model string to OpenAI model enum
        let model = match request.model.as_deref() {
            Some("whisper-1") => OpenAISttModel::Whisper1,
            Some("gpt-4o-transcribe") => OpenAISttModel::Gpt4oTranscribe,
            Some("gpt-4o-mini-transcribe") => OpenAISttModel::Gpt4oMiniTranscribe,
            _ => OpenAISttModel::Whisper1, // Default
        };

        // Build and execute request
        let mut builder = client
            .transcribe()
            .file(audio_data, "audio.webm")
            .model(model);

        if let Some(lang) = request.language.clone() {
            builder = builder.language(lang);
        }

        let response = builder
            .execute()
            .await
            .map_err(|e| format!("OpenAI STT error: {:?}", e))?;

        Ok(SttRes {
            text: response.text,
            provider: Provider::OpenAI,
        })
    }
}

#[hyperprocess(
    name = "TTSTT",
    ui = Some(HttpBindingConfig::default()),
    ui_path = "ttstt-ui",
    endpoints = vec![
        Binding::Http {
            path: "/api",
            config: HttpBindingConfig::new(false, false, false, None)
        }
    ],
    save_config = SaveOptions::OnDiff,
    wit_world = "ttstt-sys-v0"
)]
impl TtsttState {
    #[init]
    async fn initialize(&mut self) {
        add_to_homepage("Spider TTSTT", None, Some("/"), None);

        // Generate initial admin key if not exists
        if self.admin_key.is_empty() {
            self.admin_key = format!("ttstt-admin-{}", Uuid::new_v4());
            println!("Generated admin API key: {}", self.admin_key);

            // Add to API keys list
            self.api_keys.push(ApiKey {
                key: self.admin_key.clone(),
                role: ApiKeyRole::Admin,
                created_at: Utc::now().to_rfc3339(),
                name: "Initial Admin Key".to_string(),
            });
        }

        // Ensure storage is initialized
        if let Err(e) = self.ensure_storage_initialized().await {
            eprintln!("Failed to initialize storage: {}", e);
        }

        let our_node = our().node.clone();
        println!("TTSTT initialized on node: {}", our_node);
    }

    #[local]
    #[http]
    async fn tts(&mut self, request: TtsReq) -> Result<TtsRes, String> {
        // Validate API key if provided
        if request.api_key.is_some() {
            self.validate_api_key(request.api_key.clone(), false)?;
        }

        // Determine provider
        let provider = request
            .provider
            .clone()
            .or(self.default_tts_provider.clone())
            .ok_or("No provider specified and no default configured")?;

        // Handle request based on provider
        let response = match provider {
            Provider::OpenAI => self.handle_openai_tts(request.clone()).await?,
            Provider::ElevenLabs => self.handle_elevenlabs_tts(request.clone()).await?,
        };

        // Store audio-text pair to VFS
        let pair = AudioTextPair {
            id: Uuid::new_v4().to_string(),
            text: request.text.clone(),
            audio_data: response.audio_data.clone(),
            audio_format: response.format.clone(),
            provider: response.provider.clone(),
            timestamp: Utc::now().to_rfc3339(),
            request_type: RequestType::TTS,
            metadata: vec![],
        };

        // Save to VFS
        if let Err(e) = self.save_audio_text_pair(&pair).await {
            eprintln!("Failed to save audio-text pair: {}", e);
        }

        Ok(response)
    }

    #[local]
    #[http]
    async fn stt(&mut self, request: SttReq) -> Result<SttRes, String> {
        // Validate API key if provided
        if request.api_key.is_some() {
            self.validate_api_key(request.api_key.clone(), false)?;
        }

        // Determine provider
        let provider = request
            .provider
            .clone()
            .or(self.default_stt_provider.clone())
            .ok_or("No provider specified and no default configured")?;

        // Handle request based on provider
        let response = match provider {
            Provider::OpenAI => self.handle_openai_stt(request.clone()).await?,
            Provider::ElevenLabs => {
                return Err("ElevenLabs does not support Speech-to-Text".to_string());
            }
        };

        // Store audio-text pair to VFS
        let pair = AudioTextPair {
            id: Uuid::new_v4().to_string(),
            text: response.text.clone(),
            audio_data: request.audio_data.clone(),
            audio_format: "webm".to_string(), // Default for recorded audio
            provider: response.provider.clone(),
            timestamp: Utc::now().to_rfc3339(),
            request_type: RequestType::STT,
            metadata: vec![],
        };

        // Save to VFS
        if let Err(e) = self.save_audio_text_pair(&pair).await {
            eprintln!("Failed to save audio-text pair: {}", e);
        }

        Ok(response)
    }

    #[http]
    async fn test_tts(&mut self, request: TestTtsReq) -> Result<TtsRes, String> {
        // Create TTS request with defaults
        let tts_request = TtsReq {
            text: request.text,
            provider: self.default_tts_provider.clone(),
            voice: Some("nova".to_string()), // Default to nova
            model: Some("gpt-4o-mini-tts".to_string()), // Default to gpt-4o-mini-tts
            format: Some("mp3".to_string()),
            speed: Some(1.5), // Default to 1.5x speed
            api_key: None,
        };

        // Process request
        self.tts(tts_request).await
    }

    #[http]
    async fn test_stt(&mut self, request: TestSttReq) -> Result<SttRes, String> {
        // Create STT request with defaults
        let stt_request = SttReq {
            audio_data: request.audio_data,
            provider: self.default_stt_provider.clone(),
            model: None,
            language: None,
            api_key: None,
        };

        // Process request
        self.stt(stt_request).await
    }

    #[local]
    #[http]
    async fn add_provider(&mut self, request: AddProviderReq) -> Result<String, String> {
        self.validate_api_key(request.api_key, true)?;

        let config = request.config;

        // Remove existing config for this provider
        self.providers.retain(|p| p.provider != config.provider);

        // Update default providers if needed
        if config.is_default_tts {
            self.default_tts_provider = Some(config.provider.clone());
            // Clear other default TTS
            for provider in &mut self.providers {
                provider.is_default_tts = false;
            }
        }

        if config.is_default_stt {
            self.default_stt_provider = Some(config.provider.clone());
            // Clear other default STT
            for provider in &mut self.providers {
                provider.is_default_stt = false;
            }
        }

        self.providers.push(config);

        Ok("Provider added successfully".to_string())
    }

    #[local]
    #[http]
    async fn remove_provider(&mut self, request: RemoveProviderReq) -> Result<String, String> {
        self.validate_api_key(request.api_key, true)?;

        let provider = request.provider;

        self.providers.retain(|p| p.provider != provider);

        // Clear defaults if removed
        if self.default_tts_provider == Some(provider.clone()) {
            self.default_tts_provider = None;
        }
        if self.default_stt_provider == Some(provider) {
            self.default_stt_provider = None;
        }

        Ok("Provider removed successfully".to_string())
    }

    #[local]
    #[http]
    async fn get_providers(&self) -> Result<Vec<ProviderInfo>, String> {
        // Return providers without API keys
        let safe_providers: Vec<ProviderInfo> = self
            .providers
            .iter()
            .map(|p| ProviderInfo {
                provider: p.provider.clone(),
                is_default_tts: p.is_default_tts,
                is_default_stt: p.is_default_stt,
                default_voice: p.default_voice.clone(),
                default_speed: p.default_speed,
            })
            .collect();

        Ok(safe_providers)
    }

    #[local]
    #[http]
    async fn set_default_provider(
        &mut self,
        request: SetDefaultProviderReq,
    ) -> Result<String, String> {
        self.validate_api_key(request.api_key, true)?;

        let provider = request.provider;
        let provider_type = request.provider_type.as_str();

        // Ensure provider exists
        let exists = self.providers.iter().any(|p| p.provider == provider);
        if !exists {
            return Err("Provider not configured".to_string());
        }

        match provider_type {
            "tts" => {
                // Clear other defaults and set new one
                for p in &mut self.providers {
                    p.is_default_tts = p.provider == provider;
                }
                self.default_tts_provider = Some(provider);
            }
            "stt" => {
                // Clear other defaults and set new one
                for p in &mut self.providers {
                    p.is_default_stt = p.provider == provider;
                }
                self.default_stt_provider = Some(provider);
            }
            _ => return Err("Invalid type: must be 'tts' or 'stt'".to_string()),
        }

        Ok("Default provider set successfully".to_string())
    }

    #[local]
    #[http]
    async fn generate_api_key(
        &mut self,
        request: GenerateApiKeyReq,
    ) -> Result<GenerateApiKeyRes, String> {
        self.validate_api_key(request.api_key, true)?;

        let name = request.name;
        let role = request.role;

        let new_key = ApiKey {
            key: format!(
                "ttstt-{}-{}",
                if matches!(role, ApiKeyRole::Admin) {
                    "admin"
                } else {
                    "req"
                },
                Uuid::new_v4()
            ),
            role: role.clone(),
            created_at: Utc::now().to_rfc3339(),
            name: name.to_string(),
        };

        let key_value = new_key.key.clone();
        let name_clone = name.clone();
        self.api_keys.push(new_key);

        // Return the newly generated key
        Ok(GenerateApiKeyRes {
            key: key_value,
            name: name_clone,
            role,
        })
    }

    #[local]
    #[http]
    async fn revoke_api_key(&mut self, request: RevokeApiKeyReq) -> Result<String, String> {
        self.validate_api_key(request.api_key, true)?;

        let key_to_revoke = request.key_to_revoke.as_str();

        // Don't allow revoking the initial admin key
        if key_to_revoke == self.admin_key {
            return Err("Cannot revoke initial admin key".to_string());
        }

        self.api_keys.retain(|k| k.key != key_to_revoke);

        Ok("API key revoked successfully".to_string())
    }

    #[local]
    #[http]
    async fn list_api_keys(&self, request: ListApiKeysReq) -> Result<Vec<ApiKeyInfo>, String> {
        self.validate_api_key(request.api_key, true)?;

        // Return keys without actual key values
        let safe_keys: Vec<ApiKeyInfo> = self
            .api_keys
            .iter()
            .map(|k| ApiKeyInfo {
                name: k.name.clone(),
                role: k.role.clone(),
                created_at: k.created_at.clone(),
                key_preview: format!("{}...", &k.key[..20.min(k.key.len())]),
            })
            .collect();

        Ok(safe_keys)
    }

    #[local]
    #[http]
    async fn get_history(&self, request: GetHistoryReq) -> Result<Vec<AudioTextPair>, String> {
        let limit = request.limit.unwrap_or(50) as usize;
        let offset = request.offset.unwrap_or(0) as usize;

        // Load from VFS
        let pairs = self.load_audio_text_pairs(limit, offset).await?;

        Ok(pairs)
    }

    #[local]
    #[http]
    async fn get_audio_text_pair(
        &self,
        request: GetAudioTextPairReq,
    ) -> Result<AudioTextPair, String> {
        // Load from VFS
        let pair = self.load_audio_text_pair_by_id(&request.id).await?;

        Ok(pair)
    }

    #[http]
    async fn get_admin_key(&self) -> Result<GetAdminKeyRes, String> {
        // Only return if no other admin keys exist
        let admin_count = self
            .api_keys
            .iter()
            .filter(|k| matches!(k.role, ApiKeyRole::Admin))
            .count();

        if admin_count == 1 {
            Ok(GetAdminKeyRes {
                admin_key: self.admin_key.clone(),
                message: "Save this key! It will not be shown again.".to_string(),
            })
        } else {
            Err("Admin key already retrieved".to_string())
        }
    }
}
