use actix_web::{HttpMessage, HttpRequest, HttpResponse, web};
use orchester_api::{
  PasswordChange,
  cloud::{
    AccountChanges, AccountResponse, CloudErrorCode, CloudSignIn, ConfirmEmail, DevicesResponse, IdentityTokenRequest,
    KeySetResponse, ResetFinish, ResetRequest, SignUp,
  },
};
use orchester_core::id::CloudDeviceId;

use super::extract::{ClientAddress, Device, SafeJson};
use crate::{Cloud, error::CloudError};

type Answer = Result<HttpResponse, CloudError>;

pub(super) async fn sign_up(cloud: web::Data<Cloud>, address: ClientAddress, input: SafeJson<SignUp>) -> Answer {
  cloud.sign_up(input.0, address.0).await?;
  Ok(HttpResponse::Accepted().finish())
}

pub(super) async fn confirm(cloud: web::Data<Cloud>, address: ClientAddress, input: SafeJson<ConfirmEmail>) -> Answer {
  cloud.confirm(input.0.link_token, address.0).await?;
  Ok(HttpResponse::NoContent().finish())
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

pub(super) async fn revoke_device(cloud: web::Data<Cloud>, device: Device, request: HttpRequest) -> Answer {
  let id = request
    .match_info()
    .get("id")
    .and_then(|id| id.parse::<CloudDeviceId>().ok())
    .ok_or(CloudErrorCode::NotFound)?;
  cloud.revoke_device(device.0, Some(id)).await?;
  Ok(HttpResponse::NoContent().finish())
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
