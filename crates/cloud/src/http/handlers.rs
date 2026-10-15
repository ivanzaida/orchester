use actix_web::{HttpMessage, HttpRequest, HttpResponse, web};
use orchester_api::{
  PasswordChange,
  cloud::{
    AccountChanges, AccountDeletion, AccountResponse, CloudErrorCode, CloudSignIn, ConfirmEmail, DevicesResponse,
    IdentityTokenRequest, InvitesResponse, KeySetResponse, LinkInspect, ResendConfirmation, ResetFinish, ResetRequest,
    ServerLink, ServerResponse, ServersResponse, SignUp, SignedInvite,
  },
};
use std::str::FromStr;

use super::extract::{ClientAddress, Device, SafeJson};
use crate::{Cloud, error::CloudError};

type Answer = Result<HttpResponse, CloudError>;

pub(super) async fn sign_up(cloud: web::Data<Cloud>, address: ClientAddress, input: SafeJson<SignUp>) -> Answer {
  cloud.sign_up(input.0, address.0).await?;
  Ok(HttpResponse::Accepted().finish())
}

pub(super) async fn confirm(cloud: web::Data<Cloud>, address: ClientAddress, input: SafeJson<ConfirmEmail>) -> Answer {
  cloud.confirm(input.0, address.0).await?;
  Ok(HttpResponse::NoContent().finish())
}

pub(super) async fn resend_confirmation(
  cloud: web::Data<Cloud>,
  address: ClientAddress,
  input: SafeJson<ResendConfirmation>,
) -> Answer {
  cloud.resend_confirmation(input.0, address.0).await?;
  Ok(HttpResponse::Accepted().finish())
}

pub(super) async fn inspect_link(
  cloud: web::Data<Cloud>,
  address: ClientAddress,
  input: SafeJson<LinkInspect>,
) -> Answer {
  let inspected = cloud.inspect_link(input.0, address.0).await?;
  Ok(HttpResponse::Ok().json(inspected))
}

pub(super) async fn sign_in(
  cloud: web::Data<Cloud>,
  request: HttpRequest,
  address: ClientAddress,
  input: SafeJson<CloudSignIn>,
) -> Answer {
  let signed_in = cloud.sign_in(input.0, address.0).await?;
  request.extensions_mut().insert(signed_in.account.id);
  Ok(HttpResponse::Created().json(signed_in))
}

pub(super) async fn request_reset(
  cloud: web::Data<Cloud>,
  address: ClientAddress,
  input: SafeJson<ResetRequest>,
) -> Answer {
  cloud.request_reset(input.0, address.0).await?;
  Ok(HttpResponse::Accepted().finish())
}

pub(super) async fn finish_reset(
  cloud: web::Data<Cloud>,
  address: ClientAddress,
  input: SafeJson<ResetFinish>,
) -> Answer {
  cloud.finish_reset(input.0, address.0).await?;
  Ok(HttpResponse::NoContent().finish())
}

pub(super) async fn change_password(
  cloud: web::Data<Cloud>,
  device: Device,
  address: ClientAddress,
  input: SafeJson<PasswordChange>,
) -> Answer {
  cloud.change_password(device.0, input.0, address.0).await?;
  Ok(HttpResponse::NoContent().finish())
}

pub(super) async fn sign_out(cloud: web::Data<Cloud>, device: Device) -> Answer {
  cloud.revoke_device(device.0, None).await?;
  Ok(HttpResponse::NoContent().finish())
}

pub(super) async fn me(cloud: web::Data<Cloud>, device: Device) -> Answer {
  let account = cloud.me(device.0).await?;
  Ok(HttpResponse::Ok().json(AccountResponse { account }))
}

pub(super) async fn change_me(cloud: web::Data<Cloud>, device: Device, input: SafeJson<AccountChanges>) -> Answer {
  let account = cloud.change_account(device.0, input.0).await?;
  Ok(HttpResponse::Ok().json(AccountResponse { account }))
}

pub(super) async fn devices(cloud: web::Data<Cloud>, device: Device) -> Answer {
  let devices = cloud.devices(device.0).await?;
  Ok(HttpResponse::Ok().json(DevicesResponse { devices }))
}

pub(super) async fn delete_me(
  cloud: web::Data<Cloud>,
  device: Device,
  address: ClientAddress,
  input: SafeJson<AccountDeletion>,
) -> Answer {
  cloud.delete_account(device.0, input.0, address.0).await?;
  Ok(HttpResponse::NoContent().finish())
}

pub(super) async fn revoke_device(cloud: web::Data<Cloud>, device: Device, request: HttpRequest) -> Answer {
  cloud.revoke_device(device.0, Some(path_id(&request)?)).await?;
  Ok(HttpResponse::NoContent().finish())
}

pub(super) async fn servers(cloud: web::Data<Cloud>, device: Device) -> Answer {
  let servers = cloud.servers(device.0).await?;
  Ok(HttpResponse::Ok().json(ServersResponse { servers }))
}

pub(super) async fn link_server(cloud: web::Data<Cloud>, device: Device, input: SafeJson<ServerLink>) -> Answer {
  let server = cloud.link_server(device.0, input.0).await?;
  Ok(HttpResponse::Created().json(ServerResponse { server }))
}

pub(super) async fn invites(cloud: web::Data<Cloud>, device: Device) -> Answer {
  let invites = cloud.invites(device.0).await?;
  Ok(HttpResponse::Ok().json(InvitesResponse { invites }))
}

/// Signed by a registered team server; whoever carries it needs no token.
pub(super) async fn receive_invite(cloud: web::Data<Cloud>, input: SafeJson<SignedInvite>) -> Answer {
  cloud.receive_invite(input.0).await?;
  Ok(HttpResponse::Accepted().finish())
}

pub(super) async fn decline_invite(cloud: web::Data<Cloud>, device: Device, request: HttpRequest) -> Answer {
  cloud.decline_invite(device.0, path_id(&request)?).await?;
  Ok(HttpResponse::NoContent().finish())
}

pub(super) async fn accept_invite(cloud: web::Data<Cloud>, device: Device, request: HttpRequest) -> Answer {
  let accepted = cloud.accept_invite(device.0, path_id(&request)?).await?;
  Ok(HttpResponse::Created().json(accepted))
}

/// The path's `{id}`; one that cannot be an id names nothing there (`404`).
fn path_id<T: FromStr>(request: &HttpRequest) -> Result<T, CloudError> {
  request
    .match_info()
    .get("id")
    .and_then(|id| id.parse().ok())
    .ok_or_else(|| CloudErrorCode::NotFound.into())
}

pub(super) async fn keys(cloud: web::Data<Cloud>) -> Answer {
  let key_set = cloud.key_set(&cloud.client().await?).await?;
  Ok(HttpResponse::Ok().json(KeySetResponse { key_set }))
}

pub(super) async fn identity_token(
  cloud: web::Data<Cloud>,
  device: Device,
  input: SafeJson<IdentityTokenRequest>,
) -> Answer {
  let issued = cloud.issue_identity_token(device.0, input.0).await?;
  Ok(HttpResponse::Created().json(issued))
}

pub(super) async fn not_found() -> Answer {
  Err(CloudErrorCode::NotFound.into())
}

pub(super) async fn method_not_allowed() -> Answer {
  Err(CloudErrorCode::MethodNotAllowed.into())
}
