use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, Method, request::Parts},
};
use luna_protocol::Device;

use crate::{error::AppError, state::AppState};

pub struct AuthenticatedDevice(pub Device);

impl FromRequestParts<AppState> for AuthenticatedDevice {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        validate_tailnet(&parts.headers, state)?;
        validate_origin(&parts.method, &parts.headers, state)?;
        let token = bearer_token(&parts.headers).ok_or(AppError::AuthenticationRequired)?;
        let device = state
            .auth
            .authenticate(token)
            .await?
            .ok_or(AppError::AuthenticationRequired)?;
        Ok(Self(device))
    }
}

pub fn validate_tailnet(headers: &HeaderMap, state: &AppState) -> Result<(), AppError> {
    if state.config.allowed_tailnet_logins.is_empty() {
        return Ok(());
    }
    let login = headers
        .get("tailscale-user-login")
        .and_then(|value| value.to_str().ok())
        .map(str::to_lowercase);
    if login
        .as_ref()
        .is_some_and(|value| state.config.allowed_tailnet_logins.contains(value))
    {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

pub fn validate_origin(
    method: &Method,
    headers: &HeaderMap,
    state: &AppState,
) -> Result<(), AppError> {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return Ok(());
    }
    let Some(expected) = &state.config.public_origin else {
        return Ok(());
    };
    match headers.get("origin").and_then(|value| value.to_str().ok()) {
        None => Ok(()),
        Some(origin) if origin == expected => Ok(()),
        Some(_) => Err(AppError::Forbidden),
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    if let Some(token) = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    {
        return Some(token);
    }
    headers
        .get("cookie")
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == "luna_device").then_some(value)
            })
        })
}
