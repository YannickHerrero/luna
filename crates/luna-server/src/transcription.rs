use std::time::Duration;

use axum::{
    Json,
    extract::{Multipart, State},
};
use luna_protocol::TranscriptionResponse;
use reqwest::multipart::{Form, Part};
use tracing::warn;

use crate::{error::AppError, extract::AuthenticatedDevice, state::AppState};

const MAX_AUDIO_BYTES: usize = 25 * 1024 * 1024;

#[derive(Clone)]
pub struct TranscriptionService {
    client: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
    model: String,
}

impl TranscriptionService {
    pub fn new(api_key: Option<String>, base_url: String, model: String) -> Result<Self, AppError> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .map_err(|error| AppError::TranscriptionFailed(error.to_string()))?,
            api_key,
            base_url: base_url.trim_end_matches('/').into(),
            model,
        })
    }

    async fn transcribe(
        &self,
        bytes: Vec<u8>,
        file_name: String,
        mime_type: String,
    ) -> Result<TranscriptionResponse, AppError> {
        let api_key = self.api_key.as_deref().ok_or_else(|| {
            AppError::TranscriptionFailed("Transcription is not configured.".into())
        })?;
        let part = Part::bytes(bytes)
            .file_name(file_name)
            .mime_str(&mime_type)
            .map_err(|_| AppError::InvalidRequest("The audio MIME type is invalid.".into()))?;
        let response = self
            .client
            .post(format!("{}/audio/transcriptions", self.base_url))
            .bearer_auth(api_key)
            .multipart(
                Form::new()
                    .text("model", self.model.clone())
                    .part("file", part),
            )
            .send()
            .await
            .map_err(|error| AppError::TranscriptionFailed(error.to_string()))?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::RateLimited);
        }
        if !response.status().is_success() {
            let status = response.status();
            warn!(%status, "Transcription provider rejected audio");
            return Err(AppError::TranscriptionFailed(format!(
                "Transcription provider returned {status}"
            )));
        }
        response
            .json::<TranscriptionResponse>()
            .await
            .map_err(|error| AppError::TranscriptionFailed(error.to_string()))
    }
}

pub async fn transcribe(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    mut multipart: Multipart,
) -> Result<Json<TranscriptionResponse>, AppError> {
    let mut audio = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::InvalidRequest("The audio upload is invalid.".into()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let file_name = field
            .file_name()
            .map(str::to_owned)
            .unwrap_or_else(|| "recording.webm".into());
        let mime_type = field
            .content_type()
            .map(str::to_owned)
            .unwrap_or_else(|| "audio/webm".into());
        if !is_supported_audio_type(&mime_type) {
            return Err(AppError::InvalidRequest(
                "The recording format is not supported.".into(),
            ));
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|_| AppError::InvalidRequest("The audio upload is invalid.".into()))?;
        if bytes.is_empty() || bytes.len() > MAX_AUDIO_BYTES {
            return Err(AppError::InvalidRequest(
                "Recordings must be between 1 byte and 25 MB.".into(),
            ));
        }
        audio = Some((bytes.to_vec(), file_name, mime_type));
    }
    let (bytes, file_name, mime_type) =
        audio.ok_or_else(|| AppError::InvalidRequest("An audio recording is required.".into()))?;
    Ok(Json(
        state
            .transcription
            .transcribe(bytes, file_name, mime_type)
            .await?,
    ))
}

fn is_supported_audio_type(value: &str) -> bool {
    matches!(
        value.split(';').next().unwrap_or(value).trim(),
        "audio/flac"
            | "audio/m4a"
            | "audio/mp4"
            | "audio/mpeg"
            | "audio/mp3"
            | "audio/mpga"
            | "audio/ogg"
            | "audio/wav"
            | "audio/x-wav"
            | "audio/webm"
            | "video/mp4"
    )
}
