use std::time::Duration;

use luna_protocol::{
    ApiError, Bootstrap, DevicePlatform, PairingCodeRequestResponse, PairingExchangeRequest,
    PairingExchangeResponse,
};
use reqwest::{Client, Method, StatusCode, Url, redirect::Policy};
use serde::{Serialize, de::DeserializeOwned};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerOrigin(Url);

impl ServerOrigin {
    pub fn parse(value: &str) -> Result<Self, ServerOriginError> {
        let mut url = Url::parse(value).map_err(ServerOriginError::InvalidUrl)?;
        let host = url
            .host_str()
            .ok_or(ServerOriginError::MissingHost)?
            .to_ascii_lowercase();
        let loopback = matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1" | "[::1]");
        if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
            return Err(ServerOriginError::InsecureScheme);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(ServerOriginError::EmbeddedCredentials);
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(ServerOriginError::UnexpectedComponents);
        }
        if url.path() != "/" && !url.path().is_empty() {
            return Err(ServerOriginError::UnexpectedPath);
        }
        url.set_path("/");
        Ok(Self(url))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str().trim_end_matches('/')
    }

    pub fn endpoint(&self, path: &str) -> Result<Url, ServerOriginError> {
        self.0
            .join(path.trim_start_matches('/'))
            .map_err(ServerOriginError::InvalidUrl)
    }
}

impl std::fmt::Display for ServerOrigin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServerOriginError {
    #[error("the server URL is invalid: {0}")]
    InvalidUrl(url::ParseError),
    #[error("the server URL must include a host")]
    MissingHost,
    #[error("the server URL must use HTTPS; HTTP is allowed only for loopback development")]
    InsecureScheme,
    #[error("the server URL must not contain a username or password")]
    EmbeddedCredentials,
    #[error("the server URL must not contain a query or fragment")]
    UnexpectedComponents,
    #[error("the server URL must be an origin without a path")]
    UnexpectedPath,
}

#[derive(Clone)]
pub struct LunaApi {
    origin: ServerOrigin,
    client: Client,
    token: Option<String>,
}

impl LunaApi {
    pub fn new(origin: ServerOrigin, token: Option<String>) -> Result<Self, ApiClientError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(60))
            .user_agent(concat!("luna-tui/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            origin,
            client,
            token,
        })
    }

    #[must_use]
    pub fn origin(&self) -> &ServerOrigin {
        &self.origin
    }

    pub async fn request_pairing_code(&self) -> Result<PairingCodeRequestResponse, ApiClientError> {
        self.request(Method::POST, "/v1/pairing/request", Option::<&()>::None)
            .await
    }

    pub async fn exchange_pairing_code(
        &self,
        code: &str,
        device_name: &str,
    ) -> Result<PairingExchangeResponse, ApiClientError> {
        self.request(
            Method::POST,
            "/v1/pairing/exchange",
            Some(&PairingExchangeRequest {
                code: code.into(),
                device_name: device_name.into(),
                platform: DevicePlatform::Tui,
            }),
        )
        .await
    }

    pub async fn bootstrap(&self) -> Result<Bootstrap, ApiClientError> {
        self.request(Method::GET, "/v1/bootstrap", Option::<&()>::None)
            .await
    }

    async fn request<Response, Body>(
        &self,
        method: Method,
        path: &str,
        body: Option<&Body>,
    ) -> Result<Response, ApiClientError>
    where
        Response: DeserializeOwned,
        Body: Serialize + ?Sized,
    {
        let mut request = self.client.request(method, self.origin.endpoint(path)?);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await?;
        let status = response.status();
        if status.is_redirection() {
            return Err(ApiClientError::RedirectRejected(status.as_u16()));
        }
        let bytes = response.bytes().await?;
        if !status.is_success() {
            let message = serde_json::from_slice::<ApiError>(&bytes)
                .map(|error| error.message)
                .unwrap_or_else(|_| fallback_status_message(status));
            return Err(ApiClientError::Server {
                status: status.as_u16(),
                message,
            });
        }
        serde_json::from_slice(&bytes).map_err(ApiClientError::Decode)
    }
}

fn fallback_status_message(status: StatusCode) -> String {
    status
        .canonical_reason()
        .unwrap_or("Luna could not complete the request")
        .into()
}

#[derive(Debug, thiserror::Error)]
pub enum ApiClientError {
    #[error(transparent)]
    InvalidOrigin(#[from] ServerOriginError),
    #[error("Luna returned HTTP {status}: {message}")]
    Server { status: u16, message: String },
    #[error("Luna refused an HTTP redirect ({0}) to protect the device credential")]
    RedirectRejected(u16),
    #[error("Luna returned an invalid response: {0}")]
    Decode(serde_json::Error),
    #[error("Luna could not be reached: {0}")]
    Transport(#[from] reqwest::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_private_https_and_loopback_http_origins() {
        for value in [
            "https://luna.example.ts.net:8447",
            "http://127.0.0.1:9870",
            "http://localhost:9870",
            "http://[::1]:9870",
        ] {
            assert!(ServerOrigin::parse(value).is_ok(), "{value}");
        }
    }

    #[test]
    fn rejects_origins_that_could_leak_credentials() {
        for value in [
            "http://luna.example.ts.net:8447",
            "https://user:pass@luna.example.ts.net",
            "https://luna.example.ts.net/path",
            "https://luna.example.ts.net?redirect=evil",
            "https://luna.example.ts.net#fragment",
        ] {
            assert!(ServerOrigin::parse(value).is_err(), "{value}");
        }
    }
}
