mod admin;
mod admin_proxy;
mod auth;
mod claims;
mod cloud;
mod events;
mod extract;
mod internal;
mod invites;
mod managed;
mod projects;
mod settings;
mod tasks;

use std::{future::poll_fn, pin::Pin, time::Duration};

use axum::{
  Json, Router,
  body::Body,
  extract::{ConnectInfo, MatchedPath, Request, State},
  http::HeaderValue,
  middleware::{self, Next},
  response::{IntoResponse, Response},
  routing::{delete, get, post},
};
use hyper::body::Body as HttpBody;
use orchester_api::{DeviceKind, ErrorCode, ServerInfo};
use orchester_core::id::UserId;

use crate::{Server, cloud_link::Link, db, error::ServerError, tls::PeerAddr};

/// The public listener: the API, and every other path proxied to the server admin (`admin_proxy`).
pub(crate) fn router(server: Server) -> Router {
  Router::new()
    .nest("/api/v1", api(&server))
    .fallback(admin_proxy::forward)
    .method_not_allowed_fallback(|| async { ServerError::from(ErrorCode::MethodNotAllowed) })
    .layer(middleware::from_fn(body_limit))
    .layer(middleware::from_fn_with_state(server.clone(), request_log))
    .with_state(server)
}

/// The private listener the server admin's own server side calls: the API only, with the client's address and host
/// as the admin forwards them (`internal`).
pub(crate) fn internal_router(server: Server) -> Router {
  Router::new()
    .nest("/api/v1", api(&server))
    .fallback(|| async { ServerError::from(ErrorCode::NotFound) })
    .method_not_allowed_fallback(|| async { ServerError::from(ErrorCode::MethodNotAllowed) })
    .layer(middleware::from_fn(body_limit))
    .layer(middleware::from_fn_with_state(server.clone(), request_log))
    .layer(middleware::from_fn(internal::forwarded))
    .with_state(server)
}

fn api(server: &Server) -> Router<Server> {
  // What only the app does: work on tasks, claims and events, link checkouts, pair its next device. A browser signed
  // in to the server admin reaches none of it.
  let app = Router::new()
    .route("/auth/sessions", get(auth::sessions))
    .route("/auth/sessions/{id}", delete(auth::revoke))
    .route("/auth/pairings", post(auth::pairing))
    .merge(projects::app_routes())
    .merge(tasks::routes())
    .merge(claims::routes())
    .route("/projects/{project}/events", get(events::subscribe))
    .route_layer(middleware::from_fn(app_only));
  let protected = Router::new()
    .route("/auth/me", get(auth::me))
    .route("/auth/sessions/current", delete(auth::sign_out))
    .route("/auth/approvals", get(auth::approvals))
    .route("/auth/approvals/{id}", post(auth::decide))
    .route("/admin/users", get(admin::users))
    .route("/admin/users/{id}", axum::routing::patch(admin::update))
    .route("/admin/users/{id}/sessions", delete(admin::revoke))
    .route("/admin/sessions", get(admin::sessions))
    .route("/admin/sessions/{id}", delete(admin::revoke_session))
    .route("/admin/browsers", get(admin::browsers))
    .route("/admin/browsers/{id}", delete(admin::revoke_browser))
    .route("/admin/projects", get(managed::list))
    .route("/admin/projects/{project}", get(managed::show))
    .route("/invites", get(invites::list).post(invites::create))
    .route("/invites/{id}", delete(invites::revoke))
    .route("/server/cloud", get(cloud::show_cloud).patch(cloud::change_cloud))
    .route("/server/settings", get(settings::show).patch(settings::change))
    .merge(projects::routes())
    .merge(app)
    .route_layer(middleware::from_fn_with_state(server.clone(), authenticated));
  protected
    .route("/server", get(info))
    .route("/auth/setup", post(auth::check_setup))
    .route("/auth/code", post(auth::join_with_code))
    .route("/auth/cloud", post(auth::join_with_cloud))
}

/// Refuses a browser's admin sign-in on what only the app does.
async fn app_only(request: Request, next: Next) -> Response {
  let admin_web = request
    .extensions()
    .get::<db::Authenticated>()
    .is_some_and(|auth| auth.kind == DeviceKind::AdminWeb);
  if admin_web {
    return ServerError::from(ErrorCode::Forbidden).into_response();
  }
  next.run(request).await
}

async fn body_limit(request: Request, next: Next) -> Response {
  // Enforce the contract even on routes that do not extract a JSON body, including chunked requests.
  let (parts, body) = request.into_parts();
  match tokio::time::timeout(Duration::from_secs(10), read_body(body)).await {
    Ok(Ok(bytes)) => next.run(Request::from_parts(parts, Body::from(bytes))).await,
    Ok(Err(code)) => ServerError::from(code).into_response(),
    Err(_) => {
      let mut response = ServerError::from(ErrorCode::RequestTimeout).into_response();
      response
        .headers_mut()
        .insert("Connection", HeaderValue::from_static("close"));
      response
    }
  }
}

async fn read_body(mut body: Body) -> Result<Vec<u8>, ErrorCode> {
  let mut bytes = Vec::new();
  while let Some(frame) = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
    let frame = frame.map_err(|_| ErrorCode::BadRequest)?;
    if let Ok(data) = frame.into_data() {
      if bytes.len().saturating_add(data.len()) > 1024 * 1024 {
        return Err(ErrorCode::PayloadTooLarge);
      }
      bytes.extend_from_slice(&data);
    }
  }
  Ok(bytes)
}

async fn info(State(server): State<Server>) -> Result<Json<ServerInfo>, ServerError> {
  let state = server.inner.clone();
  let (name, needs_setup, cloud) = server
    .inner
    .db
    .read(move |db| {
      Ok((
        crate::settings::name(db, &state.config.name)?,
        !db::has_admin(db)?,
        Link::load(db)?.map(|link| link.status(&state.keys)),
      ))
    })
    .await?;
  Ok(Json(ServerInfo {
    name,
    // 3: device tokens from cloud identities and codes, no passwords; projects by slug and address.
    api_version: 3,
    needs_bootstrap: needs_setup,
    cloud,
    certificate: server.certificate(),
    version: env!("CARGO_PKG_VERSION").to_owned(),
  }))
}

/// The server's base address: the configured `address`, or the scheme this server serves and the `Host` the request
/// was sent to. A project's address is this and its slug.
pub(super) fn base_address(server: &Server, headers: &axum::http::HeaderMap) -> String {
  if let Some(address) = &server.inner.config.address {
    return address.clone();
  }
  let scheme = if server.inner.tls.is_some() { "https" } else { "http" };
  let host = headers
    .get(axum::http::header::HOST)
    .and_then(|value| value.to_str().ok())
    .filter(|host| !host.is_empty() && !host.contains(['/', '?', '#', '@']) && !host.contains(char::is_whitespace))
    .unwrap_or("localhost");
  format!("{scheme}://{host}")
}

async fn authenticated(State(server): State<Server>, mut request: Request, next: Next) -> Response {
  let auth = match extract::bearer(request.headers()) {
    Ok(token) => server.authenticate(token).await,
    Err(error) => Err(error),
  };
  let auth = match auth {
    Ok(auth) => auth,
    Err(error) => return error.into_response(),
  };
  let id = auth.user.id;
  request.extensions_mut().insert(auth);
  let mut response = next.run(request).await;
  response.extensions_mut().insert(id);
  response
}

async fn request_log(State(server): State<Server>, request: Request, next: Next) -> Response {
  let started = std::time::Instant::now();
  let method = request.method().clone();
  let route = request
    .extensions()
    .get::<MatchedPath>()
    .map_or("<unmatched>", MatchedPath::as_str)
    .to_owned();
  let mut response = next.run(request).await;
  response.headers_mut().insert(
    "Orchester-Server-Time",
    HeaderValue::from_str(&server.now().to_string()).expect("integer time header"),
  );
  response
    .headers_mut()
    .insert("Cache-Control", HeaderValue::from_static("no-store"));
  let user = response.extensions().get::<UserId>().map(ToString::to_string);
  tracing::info!(method = %method, route = %route, status = response.status().as_u16(), user_id = user,
    duration_ms = started.elapsed().as_millis() as u64, "request");
  response
}

pub(super) fn address(
  server: &Server,
  peer: ConnectInfo<PeerAddr>,
  headers: &axum::http::HeaderMap,
) -> std::net::IpAddr {
  crate::addresses::client_address(peer.0.0.ip(), &server.inner.config.trusted_proxies, headers)
}
