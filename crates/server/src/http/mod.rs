mod admin;
mod auth;
mod claims;
mod events;
mod extract;
mod projects;
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
use orchester_api::{ErrorCode, ServerInfo};
use orchester_core::id::UserId;

use crate::{Server, error::ServerError, tls::PeerAddr};

pub(crate) fn router(server: Server) -> Router {
  let protected = Router::new()
    .route("/auth/me", get(auth::me))
    .route("/auth/sessions", get(auth::sessions))
    .route("/auth/sessions/current", delete(auth::sign_out))
    .route("/auth/sessions/{id}", delete(auth::revoke))
    .route("/auth/password", post(auth::password))
    .route("/admin/users", get(admin::users).post(admin::create))
    .route("/admin/users/{id}", axum::routing::patch(admin::update))
    .route("/admin/users/{id}/password", post(admin::reset))
    .route("/admin/users/{id}/sessions", delete(admin::revoke))
    .merge(projects::routes())
    .merge(tasks::routes())
    .merge(claims::routes())
    .route("/projects/{project}/events", get(events::subscribe))
    .route_layer(middleware::from_fn_with_state(server.clone(), authenticated));
  let api = protected
    .route("/server", get(info))
    .route("/auth/sessions", post(auth::sign_in));
  Router::new()
    .nest("/api/v1", api)
    .fallback(|| async { ServerError::from(ErrorCode::NotFound) })
    .method_not_allowed_fallback(|| async { ServerError::from(ErrorCode::MethodNotAllowed) })
    .layer(middleware::from_fn(body_limit))
    .layer(middleware::from_fn_with_state(server.clone(), request_log))
    .with_state(server)
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
  let needs_bootstrap = server
    .inner
    .db
    .read(|db| {
      Ok(db.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM users WHERE admin=1 AND disabled_at IS NULL)",
        [],
        |row| row.get(0),
      )?)
    })
    .await?;
  Ok(Json(ServerInfo {
    name: server.inner.config.name.clone(),
    api_version: 1,
    needs_bootstrap,
  }))
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
