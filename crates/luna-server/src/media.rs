use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path as AxumPath, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use image::{ImageFormat, ImageReader, Limits, codecs::jpeg::JpegEncoder, imageops::FilterType};
use luna_protocol::{AttachmentResponse, ServerEvent};
use luna_storage::NewAttachment;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{auth::now, error::AppError, extract::AuthenticatedDevice, state::AppState};

const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_DECODE_BYTES: u64 = 256 * 1024 * 1024;

pub async fn upload_attachment(
    State(state): State<AppState>,
    AuthenticatedDevice(device): AuthenticatedDevice,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<AttachmentResponse>), AppError> {
    let mut file: Option<(String, Vec<u8>)> = None;
    let mut conversation_id = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::InvalidRequest("The image upload is invalid.".into()))?
    {
        match field.name() {
            Some("file") => {
                let file_name = field
                    .file_name()
                    .map(safe_file_name)
                    .unwrap_or_else(|| "image".into());
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::InvalidRequest("The image upload is invalid.".into()))?;
                if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
                    return Err(AppError::InvalidRequest(
                        "Images must be between 1 byte and 20 MB.".into(),
                    ));
                }
                file = Some((file_name, bytes.to_vec()));
            }
            Some("conversationId") => {
                let value = field.text().await.map_err(|_| {
                    AppError::InvalidRequest("The conversation identifier is invalid.".into())
                })?;
                conversation_id = Some(Uuid::parse_str(&value).map_err(|_| {
                    AppError::InvalidRequest("The conversation identifier is invalid.".into())
                })?);
            }
            _ => {}
        }
    }
    let (file_name, bytes) =
        file.ok_or_else(|| AppError::InvalidRequest("An image file is required.".into()))?;
    let bytes = if is_heif(&bytes) {
        convert_heif(&state.config.attachment_directory, &bytes).await?
    } else {
        bytes
    };
    if let Some(id) = conversation_id
        && state.database.conversation(id).await?.is_none()
    {
        return Err(AppError::NotFound);
    }
    let format = image::guess_format(&bytes).map_err(|_| {
        AppError::InvalidRequest(
            "Only PNG, JPEG, GIF, WebP, HEIC, and HEIF images are supported.".into(),
        )
    })?;
    let (mime_type, extension) = match format {
        ImageFormat::Png => ("image/png", "png"),
        ImageFormat::Jpeg => ("image/jpeg", "jpg"),
        ImageFormat::Gif => ("image/gif", "gif"),
        ImageFormat::WebP => ("image/webp", "webp"),
        _ => {
            return Err(AppError::InvalidRequest(
                "Only PNG, JPEG, GIF, WebP, HEIC, and HEIF images are supported.".into(),
            ));
        }
    };
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let image = reader.decode().map_err(|_| {
        AppError::InvalidRequest("The image is corrupt or exceeds Luna's image limits.".into())
    })?;
    let width = image.width();
    let height = image.height();
    let thumbnail = image.resize(512, 512, FilterType::Lanczos3).to_rgb8();
    let mut thumbnail_bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut thumbnail_bytes, 82)
        .encode_image(&thumbnail)
        .map_err(|_| {
            AppError::InvalidRequest("Luna could not create an image thumbnail.".into())
        })?;

    let id = Uuid::now_v7();
    let original_key = format!("originals/{id}.{extension}");
    let thumbnail_key = format!("thumbnails/{id}.jpg");
    let original_path = state.config.attachment_directory.join(&original_key);
    let thumbnail_path = state.config.attachment_directory.join(&thumbnail_key);
    write_private_file(&original_path, &bytes).await?;
    if let Err(error) = write_private_file(&thumbnail_path, &thumbnail_bytes).await {
        let _ = tokio::fs::remove_file(&original_path).await;
        return Err(error);
    }
    let created_at = now()?;
    let stored = state
        .database
        .create_attachment(NewAttachment {
            id,
            conversation_id,
            uploaded_by_device_id: device.id,
            storage_key: &original_key,
            thumbnail_storage_key: &thumbnail_key,
            original_name: &file_name,
            mime_type,
            byte_size: i64::try_from(bytes.len()).unwrap_or(i64::MAX),
            sha256: &format!("{:x}", Sha256::digest(&bytes)),
            width,
            height,
            created_at: &created_at,
        })
        .await;
    let stored = match stored {
        Ok(stored) => stored,
        Err(error) => {
            let _ = tokio::fs::remove_file(&original_path).await;
            let _ = tokio::fs::remove_file(&thumbnail_path).await;
            return Err(error.into());
        }
    };
    state
        .events
        .append(
            conversation_id,
            Some(id),
            &ServerEvent::AttachmentUpdated(stored.attachment.clone()),
            &created_at,
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(AttachmentResponse {
            attachment: stored.attachment,
        }),
    ))
}

pub async fn attachment_content(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Response, AppError> {
    serve_attachment(&state, id, false).await
}

pub async fn repository_icon(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Response, AppError> {
    let icon = state
        .database
        .repository_icon_file(id)
        .await?
        .ok_or(AppError::NotFound)?;
    let path = state
        .config
        .repository_icon_directory
        .join(&icon.storage_key);
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound
        } else {
            AppError::Storage(luna_storage::StorageError::Io(error))
        }
    })?;
    let content_type = match std::path::Path::new(&icon.storage_key)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    };
    let mut response = Body::from(bytes).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=86400"),
    );
    Ok(response)
}

pub async fn attachment_thumbnail(
    State(state): State<AppState>,
    AuthenticatedDevice(_device): AuthenticatedDevice,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Response, AppError> {
    serve_attachment(&state, id, true).await
}

async fn serve_attachment(
    state: &AppState,
    id: Uuid,
    thumbnail: bool,
) -> Result<Response, AppError> {
    let stored = state
        .database
        .stored_attachment(id)
        .await?
        .ok_or(AppError::NotFound)?;
    let key = if thumbnail {
        &stored.thumbnail_storage_key
    } else {
        &stored.storage_key
    };
    let path = state.config.attachment_directory.join(key);
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound
        } else {
            AppError::Storage(luna_storage::StorageError::Io(error))
        }
    })?;
    let mut response = Body::from(bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(if thumbnail {
            "image/jpeg"
        } else {
            match stored.attachment.mime_type.as_str() {
                "image/png" => "image/png",
                "image/jpeg" => "image/jpeg",
                "image/gif" => "image/gif",
                "image/webp" => "image/webp",
                _ => "application/octet-stream",
            }
        }),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=86400"),
    );
    Ok(response)
}

fn is_heif(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || &bytes[4..8] != b"ftyp" {
        return false;
    }
    matches!(
        &bytes[8..12],
        b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"mif1" | b"msf1"
    )
}

#[cfg(target_os = "macos")]
async fn convert_heif(directory: &Path, bytes: &[u8]) -> Result<Vec<u8>, AppError> {
    let id = Uuid::new_v4();
    let input = directory.join(format!("staging/{id}.heic"));
    let output = directory.join(format!("staging/{id}.jpg"));
    write_private_file(&input, bytes).await?;
    let result = tokio::process::Command::new("/usr/bin/sips")
        .args(["-s", "format", "jpeg"])
        .arg(&input)
        .arg("--out")
        .arg(&output)
        .output()
        .await;
    let converted = match result {
        Ok(result) if result.status.success() => tokio::fs::read(&output).await.ok(),
        _ => None,
    };
    let _ = tokio::fs::remove_file(input).await;
    let _ = tokio::fs::remove_file(output).await;
    converted.filter(|value| !value.is_empty()).ok_or_else(|| {
        AppError::InvalidRequest("The HEIC or HEIF image could not be converted.".into())
    })
}

#[cfg(not(target_os = "macos"))]
async fn convert_heif(_directory: &Path, _bytes: &[u8]) -> Result<Vec<u8>, AppError> {
    Err(AppError::InvalidRequest(
        "HEIC and HEIF conversion is unavailable on this server.".into(),
    ))
}

async fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(|| {
        AppError::InvalidRequest("The attachment storage path is invalid.".into())
    })?;
    tokio::fs::create_dir_all(parent).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).await?;
    }
    tokio::fs::write(path, bytes).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await?;
    }
    Ok(())
}

fn safe_file_name(value: &str) -> String {
    PathBuf::from(value)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image")
        .chars()
        .filter(|character| !character.is_control())
        .take(200)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{is_heif, safe_file_name};

    #[test]
    fn recognizes_apple_image_container_brands() {
        assert!(is_heif(b"\0\0\0\x18ftypheic\0\0\0\0"));
        assert!(is_heif(b"\0\0\0\x18ftypmif1\0\0\0\0"));
        assert!(!is_heif(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn removes_directories_from_uploaded_names() {
        assert_eq!(safe_file_name("../../private/photo.heic"), "photo.heic");
    }
}
