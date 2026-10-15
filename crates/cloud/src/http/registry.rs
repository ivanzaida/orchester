//! The team server registry and invites in flight: `/servers` and `/invites`.

use actix_web::{HttpRequest, HttpResponse, web};
use orchester_api::cloud::{
  CloudErrorCode, CloudInvites, InviteDelivered, InviteWithdrawn, ServerLink, ServerResponse, ServersResponse,
};
use orchester_core::id::{InviteId, TeamServerId};

use super::extract::{Device, SafeJson};
use crate::{Cloud, error::CloudError};

type Answer = Result<HttpResponse, CloudError>;

pub(super) async fn link(cloud: web::Data<Cloud>, device: Device, input: SafeJson<ServerLink>) -> Answer {
  let linked = cloud.link_server(device.0, input.0).await?;
  Ok(HttpResponse::Created().json(linked))
}

pub(super) async fn server(cloud: web::Data<Cloud>, _device: Device, request: HttpRequest) -> Answer {
  let id: TeamServerId = path_id(&request)?;
  let server = cloud.server(id).await?;
  Ok(HttpResponse::Ok().json(ServerResponse { server }))
}

pub(super) async fn servers(cloud: web::Data<Cloud>, device: Device) -> Answer {
  let servers = cloud.servers(device.0).await?;
  Ok(HttpResponse::Ok().json(ServersResponse { servers }))
}

pub(super) async fn receive_invite(cloud: web::Data<Cloud>, input: SafeJson<InviteDelivered>) -> Answer {
  cloud.receive_invite(input.0).await?;
  Ok(HttpResponse::Accepted().finish())
}

pub(super) async fn withdraw_invite(
  cloud: web::Data<Cloud>,
  request: HttpRequest,
  input: SafeJson<InviteWithdrawn>,
) -> Answer {
  let id: InviteId = path_id(&request)?;
  cloud.withdraw_invite(id, input.0).await?;
  Ok(HttpResponse::NoContent().finish())
}

pub(super) async fn invites(cloud: web::Data<Cloud>, device: Device) -> Answer {
  let invites = cloud.invites(device.0).await?;
  Ok(HttpResponse::Ok().json(CloudInvites { invites }))
}

fn path_id<T: std::str::FromStr>(request: &HttpRequest) -> Result<T, CloudError> {
  request
    .match_info()
    .get("id")
    .and_then(|id| id.parse().ok())
    .ok_or_else(|| CloudErrorCode::NotFound.into())
}
