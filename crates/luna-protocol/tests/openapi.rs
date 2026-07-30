use luna_protocol::openapi;

#[test]
fn documents_the_supported_http_surface() {
    let document = openapi();
    let paths = document.paths.paths;
    for path in [
        "/v1/health/live",
        "/v1/health/ready",
        "/v1/pairing/exchange",
        "/v1/bootstrap",
        "/v1/sync",
        "/v1/conversations",
        "/v1/conversations/{id}",
        "/v1/conversations/{id}/messages",
        "/v1/conversations/{id}/abort",
        "/v1/conversations/{id}/archive",
        "/v1/attachments",
        "/v1/repositories/{id}/icon",
        "/v1/transcriptions",
    ] {
        assert!(paths.contains_key(path), "missing OpenAPI path {path}");
    }
    assert!(
        document
            .components
            .expect("components")
            .security_schemes
            .contains_key("deviceToken")
    );
}
