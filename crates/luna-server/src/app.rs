use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    body::Body,
    http::{HeaderName, HeaderValue, Request, header},
    middleware::{Next, from_fn},
    response::Response,
};
use luna_pi::SessionRuntimeConfig;
use luna_storage::Database;
use tower_http::{
    catch_panic::CatchPanicLayer,
    compression::CompressionLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    services::ServeDir,
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

use crate::{
    auth::AuthService, config::Config, error::AppError, events::EventHub, maintenance::Maintenance,
    routes, runtime::ConversationRuntime, state::AppState, transcription::TranscriptionService,
};

pub struct BuiltApp {
    pub router: Router,
    pub pairing_code: String,
    pub database: Database,
    pub runtime: Arc<ConversationRuntime>,
    pub maintenance: Maintenance,
}

pub async fn build(config: Config) -> Result<BuiltApp, AppError> {
    let web_directory = config.web_directory.clone();
    let database = Database::connect(&config.database_path).await?;
    let recovered_at = crate::auth::now()?;
    database.recover_inflight_dispatches(&recovered_at).await?;
    for message in database.recover_streaming_messages(&recovered_at).await? {
        database
            .append_event(
                Some(message.conversation_id),
                Some(message.id),
                &luna_protocol::ServerEvent::MessageUpserted(message),
                &recovered_at,
            )
            .await?;
    }
    for conversation_id in database
        .recover_interrupted_conversations(&recovered_at)
        .await?
    {
        database
            .reset_agent_activities(conversation_id, &recovered_at)
            .await?;
        database
            .append_event(
                Some(conversation_id),
                Some(conversation_id),
                &luna_protocol::ServerEvent::SessionStateChanged {
                    state: luna_protocol::SessionState::Crashed,
                },
                &recovered_at,
            )
            .await?;
    }
    let maintenance = Maintenance::spawn(
        database.clone(),
        config.event_retention_days,
        config.attachment_directory.clone(),
        config.attachment_retention_days,
    );
    let events = EventHub::new(database.clone());
    let runtime = Arc::new(ConversationRuntime::new(
        SessionRuntimeConfig {
            pi_executable: config.pi_executable.clone(),
            bridge_extension: config.pi_bridge_path.clone(),
            session_directory: config.pi_session_directory.clone(),
            bridge_directory: config.bridge_directory.clone(),
            request_timeout: Duration::from_secs(15),
        },
        database.clone(),
        events.clone(),
        config.attachment_directory.clone(),
        config.repository_icon_directory.clone(),
        config.title_model.clone(),
    ));
    let transcription = TranscriptionService::new(
        config.transcription_api_key.clone(),
        config.transcription_base_url.clone(),
        config.transcription_model.clone(),
    )?;
    let state = AppState::new(
        config,
        database.clone(),
        events,
        runtime.clone(),
        transcription,
    );
    let pairing_code = AuthService::new(database.clone())
        .create_pairing_code()
        .await?
        .value;
    let request_id = HeaderName::from_static("x-request-id");
    let static_files = ServeDir::new(web_directory).append_index_html_on_directories(true);
    let router = routes::router(state)
        .fallback_service(static_files)
        .layer(from_fn(static_cache_policy))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; connect-src 'self' ws: wss:; img-src 'self' blob: data:; media-src 'self' blob:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; font-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
            ),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(self), microphone=(self)"),
        ))
        .layer(PropagateRequestIdLayer::new(request_id.clone()))
        .layer(SetRequestIdLayer::new(request_id, MakeRequestUuid))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(CatchPanicLayer::new());
    Ok(BuiltApp {
        router,
        pairing_code,
        database,
        runtime,
        maintenance,
    })
}

async fn static_cache_policy(request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    let mut response = next.run(request).await;
    if response.status().is_success() && !path.starts_with("/v1/") {
        let policy = if path.starts_with("/_next/static/") {
            "public, max-age=31536000, immutable"
        } else if path == "/" || path == "/sw.js" || path == "/manifest.webmanifest" {
            "no-cache"
        } else {
            "public, max-age=86400"
        };
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    }
    response
}
