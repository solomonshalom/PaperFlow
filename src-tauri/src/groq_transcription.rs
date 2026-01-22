//! Groq Cloud Whisper transcription client.
//!
//! This module provides transcription via Groq's cloud API using their
//! hosted Whisper models (whisper-large-v3, whisper-large-v3-turbo, and distil-whisper-large-v3-en).

use log::{debug, error, info, warn};
use reqwest::multipart;
use serde::Deserialize;
use std::io::Cursor;
use std::time::Duration;

const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

/// Base timeout for Groq API requests (30 seconds)
/// This prevents hanging on slow/unresponsive connections
const GROQ_BASE_TIMEOUT: Duration = Duration::from_secs(30);

/// Additional timeout per minute of audio (5 seconds per minute)
/// This accounts for longer audio files needing more processing time
const GROQ_TIMEOUT_PER_MINUTE: Duration = Duration::from_secs(5);

/// Maximum timeout for any request (120 seconds)
const GROQ_MAX_TIMEOUT: Duration = Duration::from_secs(120);

/// Maximum number of retry attempts for transient failures
const MAX_RETRIES: u32 = 3;

/// Initial delay between retries (doubles each attempt)
const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(500);

#[derive(Debug, Deserialize)]
struct GroqTranscriptionResponse {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GroqErrorResponse {
    error: GroqError,
}

#[derive(Debug, Deserialize)]
struct GroqError {
    message: String,
    #[serde(rename = "type")]
    error_type: Option<String>,
    code: Option<String>,
}

/// Categorized error types for better error handling
#[derive(Debug, Clone, PartialEq)]
pub enum GroqErrorKind {
    /// API key is missing or not configured
    MissingApiKey,
    /// API key is invalid or expired
    InvalidApiKey,
    /// Rate limit exceeded
    RateLimited,
    /// Request timeout
    Timeout,
    /// Network connectivity issue
    NetworkError,
    /// Server error (5xx)
    ServerError,
    /// Audio processing error
    AudioError,
    /// Unknown or other error
    Other,
}

impl GroqErrorKind {
    /// Returns true if this error type is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            GroqErrorKind::Timeout
                | GroqErrorKind::NetworkError
                | GroqErrorKind::ServerError
                | GroqErrorKind::RateLimited
        )
    }

    /// Returns a user-friendly message for this error type
    pub fn user_message(&self) -> &'static str {
        match self {
            GroqErrorKind::MissingApiKey => {
                "Groq API key is not configured. Please add your API key in Settings."
            }
            GroqErrorKind::InvalidApiKey => {
                "Groq API key is invalid or expired. Please check your API key in Settings."
            }
            GroqErrorKind::RateLimited => {
                "Groq API rate limit exceeded. Please wait a moment and try again."
            }
            GroqErrorKind::Timeout => {
                "Groq API request timed out. Please try again with shorter audio."
            }
            GroqErrorKind::NetworkError => {
                "Network error connecting to Groq. Please check your internet connection."
            }
            GroqErrorKind::ServerError => "Groq server error. Please try again in a moment.",
            GroqErrorKind::AudioError => "Error processing audio. Please try recording again.",
            GroqErrorKind::Other => "Groq transcription failed. Please try again.",
        }
    }
}

/// Calculate dynamic timeout based on audio length
fn calculate_timeout(audio_samples: &[f32]) -> Duration {
    // Assuming 16kHz sample rate
    let audio_duration_secs = audio_samples.len() as f64 / 16000.0;
    let audio_minutes = audio_duration_secs / 60.0;

    let dynamic_timeout = GROQ_BASE_TIMEOUT + GROQ_TIMEOUT_PER_MINUTE.mul_f64(audio_minutes);

    std::cmp::min(dynamic_timeout, GROQ_MAX_TIMEOUT)
}

/// Classify an error into a specific error kind
fn classify_error(
    status: Option<reqwest::StatusCode>,
    error_response: Option<&GroqErrorResponse>,
    req_error: Option<&reqwest::Error>,
) -> GroqErrorKind {
    // Check request error first
    if let Some(e) = req_error {
        if e.is_timeout() {
            return GroqErrorKind::Timeout;
        }
        if e.is_connect() {
            return GroqErrorKind::NetworkError;
        }
    }

    // Check HTTP status
    if let Some(status) = status {
        match status.as_u16() {
            401 => return GroqErrorKind::InvalidApiKey,
            429 => return GroqErrorKind::RateLimited,
            500..=599 => return GroqErrorKind::ServerError,
            _ => {}
        }
    }

    // Check error response content
    if let Some(err_resp) = error_response {
        let msg = err_resp.error.message.to_lowercase();
        let err_type = err_resp.error.error_type.as_deref().unwrap_or("");
        let code = err_resp.error.code.as_deref().unwrap_or("");

        if msg.contains("invalid api key")
            || msg.contains("invalid_api_key")
            || code == "invalid_api_key"
        {
            return GroqErrorKind::InvalidApiKey;
        }
        if msg.contains("rate limit") || err_type == "rate_limit_exceeded" {
            return GroqErrorKind::RateLimited;
        }
        if msg.contains("audio") || msg.contains("file") {
            return GroqErrorKind::AudioError;
        }
    }

    GroqErrorKind::Other
}

/// Transcribes audio using Groq's cloud Whisper API.
///
/// # Arguments
/// * `api_key` - Groq API key
/// * `model_id` - Model to use (e.g., "whisper-large-v3", "whisper-large-v3-turbo", or "distil-whisper-large-v3-en")
/// * `audio_samples` - Audio samples as f32 (mono, 16kHz expected)
/// * `language` - Optional language code (e.g., "en", "es", "auto")
///
/// # Returns
/// The transcribed text or an error.
pub async fn transcribe(
    api_key: &str,
    model_id: &str,
    audio_samples: &[f32],
    language: Option<&str>,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err(GroqErrorKind::MissingApiKey.user_message().to_string());
    }

    // Convert model_id to Groq's model name format
    let groq_model = match model_id {
        "groq-whisper-large-v3" => "whisper-large-v3",
        "groq-whisper-large-v3-turbo" => "whisper-large-v3-turbo",
        "groq-distil-whisper-large-v3-en" => "distil-whisper-large-v3-en",
        _ => {
            return Err(format!(
                "Unknown Groq model: {}. Available: groq-whisper-large-v3, groq-whisper-large-v3-turbo, groq-distil-whisper-large-v3-en",
                model_id
            ))
        }
    };

    info!(
        "Starting Groq cloud transcription with model: {}",
        groq_model
    );
    debug!("Audio samples: {} samples", audio_samples.len());

    // Convert f32 samples to WAV format
    let wav_data = samples_to_wav(audio_samples, 16000)?;
    debug!("WAV data size: {} bytes", wav_data.len());

    // Calculate dynamic timeout based on audio length
    let timeout = calculate_timeout(audio_samples);
    debug!("Using timeout: {:?}", timeout);

    // Create HTTP client with dynamic timeout
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Retry loop with exponential backoff
    let mut last_error: Option<(GroqErrorKind, String)> = None;
    let mut retry_delay = INITIAL_RETRY_DELAY;

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            warn!(
                "Retrying Groq API request (attempt {}/{}), waiting {:?}",
                attempt + 1,
                MAX_RETRIES + 1,
                retry_delay
            );
            tokio::time::sleep(retry_delay).await;
            retry_delay *= 2; // Exponential backoff
        }

        // Create multipart form (must be recreated for each attempt)
        let file_part = multipart::Part::bytes(wav_data.clone())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| format!("Failed to create multipart: {}", e))?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", groq_model.to_string())
            .text("response_format", "json");

        // Add language if specified and not "auto"
        if let Some(lang) = language {
            if lang != "auto" && !lang.is_empty() {
                // Groq uses ISO 639-1 language codes
                let lang_code = normalize_language_code(lang);
                form = form.text("language", lang_code);
            }
        }

        // Send request to Groq API
        let response = match client
            .post(GROQ_API_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let error_kind = classify_error(None, None, Some(&e));
                let error_msg = if e.is_timeout() {
                    error_kind.user_message().to_string()
                } else if e.is_connect() {
                    error_kind.user_message().to_string()
                } else {
                    format!("HTTP request failed: {}", e)
                };

                error!(
                    "Groq API request error: {} (kind: {:?})",
                    error_msg, error_kind
                );

                if error_kind.is_retryable() && attempt < MAX_RETRIES {
                    last_error = Some((error_kind, error_msg));
                    continue;
                }
                return Err(error_msg);
            }
        };

        let status = response.status();

        if status.is_success() {
            // Parse successful response
            let transcription: GroqTranscriptionResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

            info!(
                "Groq transcription completed: {} characters (attempt {})",
                transcription.text.len(),
                attempt + 1
            );
            debug!("Transcription result: {}", transcription.text);

            return Ok(transcription.text);
        }

        // Handle error response
        let response_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());

        let error_response = serde_json::from_str::<GroqErrorResponse>(&response_text).ok();
        let error_kind = classify_error(Some(status), error_response.as_ref(), None);

        let error_msg = if let Some(ref err_resp) = error_response {
            error!(
                "Groq API error: {} ({:?})",
                err_resp.error.message, error_kind
            );
            format!("{}", error_kind.user_message())
        } else {
            format!(
                "Groq API request failed with status {}: {}",
                status, response_text
            )
        };

        // Retry if the error is retryable
        if error_kind.is_retryable() && attempt < MAX_RETRIES {
            last_error = Some((error_kind, error_msg));
            continue;
        }

        return Err(error_msg);
    }

    // All retries exhausted
    if let Some((_, error_msg)) = last_error {
        Err(format!("{} (after {} retries)", error_msg, MAX_RETRIES))
    } else {
        Err("Groq transcription failed after all retries".to_string())
    }
}

/// Transcribes audio using Groq's cloud Whisper API with multilingual support.
///
/// This function is optimized for code-switching scenarios where the user
/// speaks multiple languages in the same audio. It uses prompt hints to
/// help Groq's Whisper model better handle mixed-language content.
///
/// # Arguments
/// * `api_key` - Groq API key
/// * `model_id` - Model to use (e.g., "whisper-large-v3", "whisper-large-v3-turbo", or "distil-whisper-large-v3-en")
/// * `audio_samples` - Audio samples as f32 (mono, 16kHz expected)
/// * `primary_language` - Optional primary language code (e.g., "en")
/// * `secondary_language` - Optional secondary language code (e.g., "es")
///
/// # Returns
/// The transcribed text or an error.
pub async fn transcribe_multilingual(
    api_key: &str,
    model_id: &str,
    audio_samples: &[f32],
    primary_language: Option<&str>,
    secondary_language: Option<&str>,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err(GroqErrorKind::MissingApiKey.user_message().to_string());
    }

    // Convert model_id to Groq's model name format
    let groq_model = match model_id {
        "groq-whisper-large-v3" => "whisper-large-v3",
        "groq-whisper-large-v3-turbo" => "whisper-large-v3-turbo",
        "groq-distil-whisper-large-v3-en" => "distil-whisper-large-v3-en",
        _ => {
            return Err(format!(
                "Unknown Groq model: {}. Available: groq-whisper-large-v3, groq-whisper-large-v3-turbo, groq-distil-whisper-large-v3-en",
                model_id
            ))
        }
    };

    // Note: distil-whisper is English-only, so multilingual prompts won't help
    if model_id == "groq-distil-whisper-large-v3-en"
        && (primary_language.is_some() || secondary_language.is_some())
    {
        warn!("distil-whisper-large-v3-en is English-only. Multilingual settings will be ignored.");
    }

    info!(
        "Starting Groq multilingual cloud transcription with model: {}",
        groq_model
    );
    debug!(
        "Audio samples: {} samples, primary_language: {:?}, secondary_language: {:?}",
        audio_samples.len(),
        primary_language,
        secondary_language
    );

    // Convert f32 samples to WAV format
    let wav_data = samples_to_wav(audio_samples, 16000)?;
    debug!("WAV data size: {} bytes", wav_data.len());

    // Calculate dynamic timeout based on audio length
    let timeout = calculate_timeout(audio_samples);
    debug!("Using timeout: {:?}", timeout);

    // Create HTTP client with dynamic timeout
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Build the prompt hint for multilingual transcription
    let prompt = if model_id != "groq-distil-whisper-large-v3-en" {
        if let (Some(primary), Some(secondary)) = (primary_language, secondary_language) {
            let primary_name = get_language_name(primary);
            let secondary_name = get_language_name(secondary);
            Some(format!(
                "This audio may contain {} and {}. Transcribe all languages as spoken without translation.",
                primary_name, secondary_name
            ))
        } else if let Some(primary) = primary_language {
            let primary_name = get_language_name(primary);
            Some(format!(
                "This audio may contain multiple languages including {}. Transcribe all languages as spoken without translation.",
                primary_name
            ))
        } else {
            None
        }
    } else {
        None
    };

    if let Some(ref p) = prompt {
        debug!("Using multilingual prompt hint: {}", p);
    }

    // Retry loop with exponential backoff
    let mut last_error: Option<(GroqErrorKind, String)> = None;
    let mut retry_delay = INITIAL_RETRY_DELAY;

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            warn!(
                "Retrying Groq multilingual API request (attempt {}/{}), waiting {:?}",
                attempt + 1,
                MAX_RETRIES + 1,
                retry_delay
            );
            tokio::time::sleep(retry_delay).await;
            retry_delay *= 2; // Exponential backoff
        }

        // Create multipart form (must be recreated for each attempt)
        let file_part = multipart::Part::bytes(wav_data.clone())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| format!("Failed to create multipart: {}", e))?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", groq_model.to_string())
            .text("response_format", "json");

        // Add prompt hint if available
        if let Some(ref p) = prompt {
            form = form.text("prompt", p.clone());
        }
        // Don't set a language parameter - let Groq auto-detect for code-switching

        // Send request to Groq API
        let response = match client
            .post(GROQ_API_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let error_kind = classify_error(None, None, Some(&e));
                let error_msg = error_kind.user_message().to_string();

                error!(
                    "Groq multilingual API request error: {} (kind: {:?})",
                    error_msg, error_kind
                );

                if error_kind.is_retryable() && attempt < MAX_RETRIES {
                    last_error = Some((error_kind, error_msg));
                    continue;
                }
                return Err(error_msg);
            }
        };

        let status = response.status();

        if status.is_success() {
            // Parse successful response
            let transcription: GroqTranscriptionResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

            info!(
                "Groq multilingual transcription completed: {} characters (attempt {})",
                transcription.text.len(),
                attempt + 1
            );
            debug!("Transcription result: {}", transcription.text);

            return Ok(transcription.text);
        }

        // Handle error response
        let response_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());

        let error_response = serde_json::from_str::<GroqErrorResponse>(&response_text).ok();
        let error_kind = classify_error(Some(status), error_response.as_ref(), None);

        let error_msg = if let Some(ref err_resp) = error_response {
            error!(
                "Groq multilingual API error: {} ({:?})",
                err_resp.error.message, error_kind
            );
            error_kind.user_message().to_string()
        } else {
            format!(
                "Groq API request failed with status {}: {}",
                status, response_text
            )
        };

        // Retry if the error is retryable
        if error_kind.is_retryable() && attempt < MAX_RETRIES {
            last_error = Some((error_kind, error_msg));
            continue;
        }

        return Err(error_msg);
    }

    // All retries exhausted
    if let Some((_, error_msg)) = last_error {
        Err(format!("{} (after {} retries)", error_msg, MAX_RETRIES))
    } else {
        Err("Groq multilingual transcription failed after all retries".to_string())
    }
}

/// Validates a Groq API key by making a minimal API call.
///
/// This function tests the API key by sending a tiny audio sample to check
/// if the key is valid and has the necessary permissions.
///
/// # Arguments
/// * `api_key` - The Groq API key to validate
///
/// # Returns
/// Ok(()) if the key is valid, Err with the specific error if not.
pub async fn validate_api_key(api_key: &str) -> Result<(), String> {
    if api_key.is_empty() {
        return Err(GroqErrorKind::MissingApiKey.user_message().to_string());
    }

    info!("Validating Groq API key");

    // Create a minimal WAV file with 0.1 seconds of silence
    // This is enough to test the API key without wasting credits
    let silence_samples: Vec<f32> = vec![0.0; 1600]; // 0.1 seconds at 16kHz
    let wav_data = samples_to_wav(&silence_samples, 16000)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let file_part = multipart::Part::bytes(wav_data)
        .file_name("test.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("Failed to create multipart: {}", e))?;

    let form = multipart::Form::new()
        .part("file", file_part)
        .text("model", "whisper-large-v3-turbo")
        .text("response_format", "json");

    let response = client
        .post(GROQ_API_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                GroqErrorKind::Timeout.user_message().to_string()
            } else if e.is_connect() {
                GroqErrorKind::NetworkError.user_message().to_string()
            } else {
                format!("Network error: {}", e)
            }
        })?;

    let status = response.status();

    // 401 means invalid API key
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(GroqErrorKind::InvalidApiKey.user_message().to_string());
    }

    // Any 2xx status means the key is valid (even if transcription might fail for other reasons)
    if status.is_success() {
        info!("Groq API key validated successfully");
        return Ok(());
    }

    // Parse error response for more details
    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Unknown error".to_string());

    if let Ok(error_response) = serde_json::from_str::<GroqErrorResponse>(&response_text) {
        let error_kind = classify_error(Some(status), Some(&error_response), None);
        return Err(error_kind.user_message().to_string());
    }

    // If we got a 4xx error that's not 401, it might still be a valid key
    // but with other issues (e.g., rate limit). Consider it valid for now.
    if status.is_client_error() && status != reqwest::StatusCode::UNAUTHORIZED {
        warn!(
            "Groq API returned {} during validation, but key may still be valid",
            status
        );
        return Ok(());
    }

    Err(format!("Groq API validation failed: HTTP {}", status))
}

/// Returns the human-readable name for a language code.
fn get_language_name(code: &str) -> &'static str {
    match code.to_lowercase().as_str() {
        "en" => "English",
        "es" => "Spanish",
        "fr" => "French",
        "de" => "German",
        "it" => "Italian",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "ja" => "Japanese",
        "ko" => "Korean",
        "zh" | "zh-hans" | "zh-hant" => "Chinese",
        "ar" => "Arabic",
        "hi" => "Hindi",
        "nl" => "Dutch",
        "pl" => "Polish",
        "tr" => "Turkish",
        "vi" => "Vietnamese",
        "th" => "Thai",
        "id" => "Indonesian",
        "uk" => "Ukrainian",
        "cs" => "Czech",
        "sv" => "Swedish",
        "el" => "Greek",
        "he" => "Hebrew",
        "da" => "Danish",
        "fi" => "Finnish",
        "no" => "Norwegian",
        "hu" => "Hungarian",
        "ro" => "Romanian",
        "ca" => "Catalan",
        "sk" => "Slovak",
        "bg" => "Bulgarian",
        _ => "the specified language",
    }
}

/// Converts f32 audio samples to WAV format.
fn samples_to_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|e| format!("Failed to create WAV writer: {}", e))?;
        for &sample in samples {
            // Convert f32 [-1.0, 1.0] to i16
            let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer
                .write_sample(sample_i16)
                .map_err(|e| format!("Failed to write WAV sample: {}", e))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("Failed to finalize WAV: {}", e))?;
    }

    Ok(cursor.into_inner())
}

/// Normalizes language codes to ISO 639-1 format expected by Groq.
fn normalize_language_code(lang: &str) -> String {
    match lang {
        // Handle Chinese variants
        "zh-Hans" | "zh-Hant" | "zh-CN" | "zh-TW" => "zh".to_string(),
        // Handle other common variants
        "en-US" | "en-GB" => "en".to_string(),
        "es-ES" | "es-MX" => "es".to_string(),
        "pt-BR" | "pt-PT" => "pt".to_string(),
        "fr-FR" | "fr-CA" => "fr".to_string(),
        // Return as-is for standard codes
        _ => {
            // Take just the first part if it contains a hyphen
            if let Some(code) = lang.split('-').next() {
                code.to_lowercase()
            } else {
                lang.to_lowercase()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_language_code() {
        assert_eq!(normalize_language_code("en"), "en");
        assert_eq!(normalize_language_code("en-US"), "en");
        assert_eq!(normalize_language_code("zh-Hans"), "zh");
        assert_eq!(normalize_language_code("zh-Hant"), "zh");
        assert_eq!(normalize_language_code("es-MX"), "es");
        assert_eq!(normalize_language_code("FR"), "fr");
    }

    #[test]
    fn test_samples_to_wav() {
        let samples = vec![0.0f32; 16000]; // 1 second of silence
        let wav = samples_to_wav(&samples, 16000).unwrap();
        // WAV header is 44 bytes, plus 16000 samples * 2 bytes = 32044 bytes
        assert_eq!(wav.len(), 32044);
    }
}
