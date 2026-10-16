//! The cloud's HTTP API on actix-web (`/api/v1`, JSON; `docs/cloud-api.md`). The desktop and the web front's server
//! call it alike; there are no CORS headers, since browsers never call it directly.

mod extract;
mod handlers;
mod registry;

use std::{
  future::Future,
  net::TcpListener,
  time::{Duration, Instant},
};

use actix_web::{
  App, HttpMessage, HttpServer, Resource,
  body::MessageBody,
  dev::{ServiceRequest, ServiceResponse},
  http::header::{CACHE_CONTROL, HeaderValue},
  middleware::{Next, from_fn},
  web,
};
use orchester_core::id::CloudAccountId;

use self::handlers as h;
use crate::{Cloud, error::CloudError, sweep};

/// Serves until `shutdown` resolves, then lets requests in flight finish for up to 15 seconds. Connections are
/// bounded and must send their request head within 10 seconds; keep-alive connections idle out after 10.
pub async fn serve(
  cloud: Cloud,
  listener: TcpListener,
  shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), CloudError> {
  let data = web::Data::new(cloud.clone());
  let server = HttpServer::new(move || {
    App::new()
      .app_data(data.clone())
      .service(web::scope("/api/v1").configure(routes))
      .default_service(web::to(h::not_found))
      .wrap(from_fn(request_log))
  })
  .listen(listener)?
  .max_connections(256)
  .backlog(256)
  .client_request_timeout(Duration::from_secs(10))
  .client_disconnect_timeout(Duration::from_secs(2))
  .keep_alive(Duration::from_secs(10))
  .shutdown_timeout(15)
  .disable_signals()
  .shutdown_signal(shutdown)
  .run();
  let sweeper = sweep::spawn(cloud);
  let result = server.await;
  sweeper.abort();
  Ok(result?)
}

fn routes(config: &mut web::ServiceConfig) {
  config
    .service(resource("/auth/signup").route(web::post().to(h::sign_up)))
    .service(resource("/auth/confirm").route(web::post().to(h::confirm)))
    .service(resource("/auth/confirm/resend").route(web::post().to(h::resend_confirmation)))
    .service(resource("/auth/links/inspect").route(web::post().to(h::inspect_link)))
    .service(resource("/auth/sessions").route(web::post().to(h::sign_in)))
    .service(resource("/auth/sessions/current").route(web::delete().to(h::sign_out)))
    .service(resource("/auth/reset").route(web::post().to(h::request_reset)))
    .service(resource("/auth/reset/finish").route(web::post().to(h::finish_reset)))
    .service(resource("/auth/password").route(web::post().to(h::change_password)))
    .service(
      resource("/me")
        .route(web::get().to(h::me))
        .route(web::patch().to(h::change_me))
        .route(web::delete().to(h::delete_me)),
    )
    .service(resource("/devices").route(web::get().to(h::devices)))
    .service(resource("/devices/{id}").route(web::delete().to(h::revoke_device)))
    .service(resource("/keys").route(web::get().to(h::keys)))
    .service(resource("/identity-tokens").route(web::post().to(h::identity_token)))
    .service(
      resource("/servers")
        .route(web::get().to(registry::servers))
        .route(web::post().to(registry::link)),
    )
    .service(resource("/servers/{id}").route(web::get().to(registry::server)))
    .service(
      resource("/invites")
        .route(web::get().to(registry::invites))
        .route(web::post().to(registry::receive_invite)),
    )
    .service(resource("/invites/{id}").route(web::delete().to(registry::withdraw_invite)))
    .service(resource("/invites/{id}/decline").route(web::post().to(registry::decline_invite)));
}

fn resource(path: &str) -> Resource {
  web::resource(path).default_service(web::to(h::method_not_allowed))
}

/// One record per request: method, route template, status, account id and duration; never headers, bodies or the
/// path's own values.
async fn request_log(
  request: ServiceRequest,
  next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
  let started = Instant::now();
  let method = request.method().clone();
  let route = request.match_pattern().unwrap_or_else(|| "<unmatched>".into());
  let mut response = next.call(request).await?;
  response
    .headers_mut()
    .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
  let account = response
    .request()
    .extensions()
    .get::<CloudAccountId>()
    .map(ToString::to_string);
  tracing::info!(method = %method, route = %route, status = response.status().as_u16(), account_id = account,
    duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX), "request");
  Ok(response)
}
