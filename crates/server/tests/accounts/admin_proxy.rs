//! The team server as the front door of the server admin: `/api/*` is its own, `/` goes to `/admin`, and every
//! other path is forwarded to the admin app with the client's address and host in headers the server sets itself.

use orchester_api::ErrorCode;
use serde_json::json;

use crate::{
  cloud::refused,
  support::{TestServer, admin_api, admin_app::AdminApp},
};

#[tokio::test]
async fn sa_admin_paths_are_forwarded_with_the_servers_own_forwarded_headers() {
  let admin = AdminApp::start().await;
  let url = admin.url.clone();
  let server = TestServer::start(move |config| config.admin_url = url).await;
  let page = server
    .page(
      "GET",
      "/admin/projects?tab=members",
      &[
        ("X-Forwarded-For", "203.0.113.9"),
        ("X-Forwarded-Host", "evil.example"),
        ("X-Forwarded-Proto", "https"),
        ("Forwarded", "for=203.0.113.9"),
        ("Cookie", "__Host-orchester_admin=sealed"),
      ],
    )
    .await;
  assert_eq!(page.status, 200);
  assert_eq!(page.text(), "admin page");
  assert_eq!(page.headers.get("x-admin-app").map(String::as_str), Some("yes"));
  assert!(
    !page.headers.contains_key("keep-alive"),
    "hop-by-hop headers stay on their connection"
  );
  let heads = admin.heads();
  assert_eq!(heads.len(), 1);
  let head = heads[0].to_ascii_lowercase();
  assert!(head.starts_with("get /admin/projects?tab=members http/1.1"), "{head}");
  assert!(head.contains("x-forwarded-for: 127.0.0.1\r\n"), "{head}");
  assert!(
    head.contains("x-forwarded-proto: http\r\n"),
    "a client that is no trusted proxy cannot say https: {head}"
  );
  assert!(head.contains("x-forwarded-host: localhost\r\n"), "{head}");
  assert!(head.contains("cookie: __host-orchester_admin=sealed"), "{head}");
  assert!(
    !head.contains("203.0.113.9") && !head.contains("evil.example"),
    "{head}"
  );
  for path in ["/kontur", "/_next/static/app.js", "/admin/auth/callback"] {
    assert_eq!(server.page("GET", path, &[]).await.status, 200, "{path}");
  }
  server.finish().await;
}

#[tokio::test]
async fn sa_the_root_goes_to_the_admin_and_api_paths_are_never_forwarded() {
  let admin = AdminApp::start().await;
  let url = admin.url.clone();
  let server = TestServer::start(move |config| config.admin_url = url).await;
  let root = server.page("GET", "/", &[]).await;
  assert_eq!(root.status, 303);
  assert_eq!(root.headers.get("location").map(String::as_str), Some("/admin"));
  for path in ["/api", "/api/v2/server", "/api/v1/nothing-here"] {
    let response = server.page("GET", path, &[]).await;
    refused(&response, 404, ErrorCode::NotFound);
  }
  let posted = server.page("POST", "/api/v1/server", &[]).await;
  refused(&posted, 405, ErrorCode::MethodNotAllowed);
  assert!(admin.heads().is_empty(), "the API never reaches the admin app");
  server.finish().await;
}

#[tokio::test]
async fn sa_without_the_admin_app_admin_paths_say_it_is_not_running() {
  let server = TestServer::start(|config| config.admin_url = "http://127.0.0.1:9".into()).await;
  let page = server.page("GET", "/admin", &[]).await;
  assert_eq!(page.status, 502);
  assert!(page.contains("The admin app is not running"));
  assert!(page.headers["content-security-policy"].contains("default-src 'none'"));
  let api = server.request("GET", "/server", None, None).await;
  assert_eq!(api.status, 200, "the API works without the admin app");
  server.finish().await;
}

#[tokio::test]
async fn sa_reserved_slugs_are_refused_for_projects() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  for slug in ["admin", "api", "auth"] {
    let body = json!({"idempotency_key": orchester_core::id::IdempotencyKey::generate(), "name": "Mine",
      "slug": slug});
    let response = server
      .request("POST", "/projects", Some(&admin.token), Some(body))
      .await;
    refused(&response, 409, ErrorCode::SlugTaken);
  }
  for slug in ["_next", ".well-known", "Kontur Two", ""] {
    let body = json!({"idempotency_key": orchester_core::id::IdempotencyKey::generate(), "name": "Mine",
      "slug": slug});
    let response = server
      .request("POST", "/projects", Some(&admin.token), Some(body))
      .await;
    refused(&response, 422, ErrorCode::Invalid);
  }
  server.finish().await;
}

#[tokio::test]
async fn sa_the_admin_api_listener_takes_the_browsers_address_and_host_from_the_admin() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let api = server.start_admin_api().await;
  let headers = [
    ("X-Forwarded-For", "198.51.100.7"),
    ("X-Forwarded-Host", "orchester.example.test"),
    ("X-Forwarded-Proto", "https"),
  ];
  let body = json!({"idempotency_key": orchester_core::id::IdempotencyKey::generate(), "name": "Kontur",
    "slug": "kontur"});
  let created = admin_api(api, "POST", "/projects", Some(&admin.token), Some(body), &headers).await;
  assert_eq!(created.status, 201);
  let address = created.json::<serde_json::Value>()["project"]["address"]
    .as_str()
    .expect("address")
    .to_owned();
  assert_eq!(address, "http://orchester.example.test/kontur");
  for _ in 0..10 {
    let wrong = json!({"code": "ZZZZ-ZZZZ", "device": "browser", "kind": "admin_web"});
    let response = admin_api(api, "POST", "/auth/code", None, Some(wrong), &headers).await;
    assert_eq!(response.status, 401);
  }
  let limited = json!({"code": "ZZZZ-ZZZZ", "device": "browser", "kind": "admin_web"});
  let response = admin_api(api, "POST", "/auth/code", None, Some(limited.clone()), &headers).await;
  assert_eq!(
    response.status, 429,
    "the browser's address is limited, not the admin app's"
  );
  let other = admin_api(
    api,
    "POST",
    "/auth/code",
    None,
    Some(limited),
    &[("X-Forwarded-For", "198.51.100.8")],
  )
  .await;
  assert_eq!(other.status, 401, "another browser is not");
  server.finish().await;
}

/// The scheme the admin app is told, from one request through the server's front door.
async fn forwarded_proto(configure: impl FnOnce(&mut orchester_server::config::Config), proto: Option<&str>) -> String {
  let admin = AdminApp::start().await;
  let url = admin.url.clone();
  let server = TestServer::start(move |config| {
    config.admin_url = url;
    configure(config);
  })
  .await;
  let headers: Vec<(&str, &str)> = proto.map(|proto| ("X-Forwarded-Proto", proto)).into_iter().collect();
  assert_eq!(server.page("GET", "/admin", &headers).await.status, 200);
  let head = admin.heads().pop().expect("a forwarded request").to_ascii_lowercase();
  server.finish().await;
  head
    .lines()
    .find_map(|line| line.strip_prefix("x-forwarded-proto: "))
    .expect("the forwarded scheme")
    .trim()
    .to_owned()
}

/// Security review 1, M1: behind Caddy (`tls = "off"`), the admin learns the browser used https from the trusted proxy,
/// or from the configured address, so its pages do not say *This page needs HTTPS*.
#[tokio::test]
async fn sa_behind_a_trusted_proxy_the_admin_is_told_the_browsers_scheme() {
  let trusted = |config: &mut orchester_server::config::Config| {
    config.trusted_proxies = vec!["127.0.0.1".parse().expect("loopback")];
  };
  assert_eq!(forwarded_proto(trusted, Some("https")).await, "https");
  assert_eq!(forwarded_proto(trusted, Some("HTTP")).await, "http");
  assert_eq!(
    forwarded_proto(trusted, Some("javascript")).await,
    "http",
    "only http or https"
  );
  assert_eq!(forwarded_proto(trusted, None).await, "http");
  assert_eq!(
    forwarded_proto(|_| {}, Some("https")).await,
    "http",
    "an untrusted peer is not believed"
  );
  let addressed = |config: &mut orchester_server::config::Config| {
    config.address = Some("https://orchester.example.test".into());
  };
  assert_eq!(
    forwarded_proto(addressed, None).await,
    "https",
    "the configured address's scheme"
  );
}

/// Security review 1, minor 4: what `Connection` names stays on its connection, and a client's proxy headers (one of
/// them would let a request skip Next.js' middleware) never reach the admin.
#[tokio::test]
async fn sa_connection_named_and_proxy_headers_from_the_client_are_dropped() {
  let admin = AdminApp::start().await;
  let url = admin.url.clone();
  let server = TestServer::start(move |config| config.admin_url = url).await;
  let page = server
    .page(
      "GET",
      "/admin",
      &[
        ("Connection", "close, X-Secret-Hop"),
        ("X-Secret-Hop", "hop-value"),
        ("X-Forwarded-Port", "8443"),
        ("X-Forwarded-Prefix", "/elsewhere"),
        ("X-Real-IP", "198.51.100.77"),
        ("X-Middleware-Subrequest", "middleware:middleware:middleware"),
        ("X-Kept", "kept-value"),
      ],
    )
    .await;
  assert_eq!(page.status, 200);
  let head = admin.heads().pop().expect("a forwarded request").to_ascii_lowercase();
  for gone in [
    "hop-value",
    "8443",
    "/elsewhere",
    "198.51.100.77",
    "x-middleware-subrequest",
  ] {
    assert!(!head.contains(gone), "{gone} reached the admin: {head}");
  }
  assert!(head.contains("x-kept: kept-value"), "{head}");
  server.finish().await;
}
