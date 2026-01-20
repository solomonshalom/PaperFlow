//! Groq Cloud Whisper transcription client.
//!
//! This module provides transcription via Groq's cloud API using their
//! hosted Whisper models (whisper-large-v3 and whisper-large-v3-turbo).

use log::{debug, error, info};
use reqwest::multipart;
use serde::Deserialize;
use std::io::Cursor;

const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

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
}

/// Transcribes audio using Groq's cloud Whisper API.
///
/// # Arguments
/// * `api_key` - Groq API key
/// * `model_id` - Model to use (e.g., "whisper-large-v3" or "whisper-large-v3-turbo")
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
        return Err(
            "Groq API key is not configured. Please add your API key in Settings.".to_string(),
        );
    }

    // Convert model_id to Groq's model name format
    let groq_model = match model_id {
        "groq-whisper-large-v3" => "whisper-large-v3",
        "groq-whisper-large-v3-turbo" => "whisper-large-v3-turbo",
        _ => {
            return Err(format!(
            "Unknown Groq model: {}. Use 'groq-whisper-large-v3' or 'groq-whisper-large-v3-turbo'",
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

    // Create multipart form
    let file_part = multipart::Part::bytes(wav_data)
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
    let client = reqwest::Client::new();
    let response = client
        .post(GROQ_API_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status();

    if !status.is_success() {
        let response_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());

        // Try to parse error response
        if let Ok(error_response) = serde_json::from_str::<GroqErrorResponse>(&response_text) {
            error!(
                "Groq API error: {} ({})",
                error_response.error.message,
                error_response.error.error_type.unwrap_or_default()
            );
            return Err(format!("Groq API error: {}", error_response.error.message));
        }
        return Err(format!(
            "Groq API request failed with status {}: {}",
            status, response_text
        ));
    }

    // Parse successful response
    let transcription: GroqTranscriptionResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

    info!(
        "Groq transcription completed: {} characters",
        transcription.text.len()
    );
    debug!("Transcription result: {}", transcription.text);

    Ok(transcription.text)
}

/// Transcribes audio using Groq's cloud Whisper API with multilingual support.
///
/// This function is optimized for code-switching scenarios where the user
/// speaks multiple languages in the same audio. It uses prompt hints to
/// help Groq's Whisper model better handle mixed-language content.
///
/// # Arguments
/// * `api_key` - Groq API key
/// * `model_id` - Model to use (e.g., "whisper-large-v3" or "whisper-large-v3-turbo")
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
        return Err(
            "Groq API key is not configured. Please add your API key in Settings.".to_string(),
        );
    }

    // Convert model_id to Groq's model name format
    let groq_model = match model_id {
        "groq-whisper-large-v3" => "whisper-large-v3",
        "groq-whisper-large-v3-turbo" => "whisper-large-v3-turbo",
        _ => {
            return Err(format!(
            "Unknown Groq model: {}. Use 'groq-whisper-large-v3' or 'groq-whisper-large-v3-turbo'",
            model_id
        ))
        }
    };

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

    // Create multipart form
    let file_part = multipart::Part::bytes(wav_data)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("Failed to create multipart: {}", e))?;

    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("model", groq_model.to_string())
        .text("response_format", "json");

    // Build a prompt hint for expected languages to help with code-switching
    if let (Some(primary), Some(secondary)) = (primary_language, secondary_language) {
        let primary_name = get_language_name(primary);
        let secondary_name = get_language_name(secondary);
        let prompt = format!(
            "This audio may contain {} and {}. Transcribe all languages as spoken without translation.",
            primary_name, secondary_name
        );
        debug!("Using multilingual prompt hint: {}", prompt);
        form = form.text("prompt", prompt);
    } else if let Some(primary) = primary_language {
        let primary_name = get_language_name(primary);
        let prompt = format!(
            "This audio may contain multiple languages including {}. Transcribe all languages as spoken without translation.",
            primary_name
        );
        debug!("Using single-language prompt hint: {}", prompt);
        form = form.text("prompt", prompt);
    }
    // Don't set a language parameter - let Groq auto-detect for code-switching

    // Send request to Groq API
    let client = reqwest::Client::new();
    let response = client
        .post(GROQ_API_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status();

    if !status.is_success() {
        let response_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());

        // Try to parse error response
        if let Ok(error_response) = serde_json::from_str::<GroqErrorResponse>(&response_text) {
            error!(
                "Groq API error: {} ({})",
                error_response.error.message,
                error_response.error.error_type.unwrap_or_default()
            );
            return Err(format!("Groq API error: {}", error_response.error.message));
        }
        return Err(format!(
            "Groq API request failed with status {}: {}",
            status, response_text
        ));
    }

    // Parse successful response
    let transcription: GroqTranscriptionResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

    info!(
        "Groq multilingual transcription completed: {} characters",
        transcription.text.len()
    );
    debug!("Transcription result: {}", transcription.text);

    Ok(transcription.text)
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
