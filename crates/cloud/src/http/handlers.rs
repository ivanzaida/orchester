use std::str::FromStr;

use actix_web::{HttpMessage, HttpRequest, HttpResponse, web};
use orchester_api::{
  PasswordChange,
  cloud::{
    AccountChanges, AccountDeletion, CloudErrorCode, CloudSignIn, ConfirmEmail, DevicesResponse, IdentityTokenRequest,
    KeySetResponse, LinkInspect, ResendConfirmation, ResetFinish, ResetRequest, SignUp, TwoFactorCode, TwoFactorOff,
    TwoFactorSignIn, TwoFactorStart,
  },
};

use super::extract::{ClientAddress, Device, SafeJson};
use crate::{Cloud, accounts::SignInAnswer, error::CloudError};

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
  let (account, answer) = cloud.sign_in(input.0, address.0).await?;
  request.extensions_mut().insert(account);
  Ok(match answer {
    SignInAnswer::SignedIn(signed_in) => HttpResponse::Created().json(signed_in),
    SignInAnswer::Challenged(challenge) => HttpResponse::Accepted().json(challenge),
  })
}

pub(super) async fn finish_sign_in(
  cloud: web::Data<Cloud>,
  request: HttpRequest,
  address: ClientAddress,
  input: SafeJson<TwoFactorSignIn>,
) -> Answer {
  let signed_in = cloud.finish_sign_in(input.0, address.0).await?;
  request.extensions_mut().insert(signed_in.signed_in.account.id);
  Ok(HttpResponse::Created().json(signed_in))
}

pub(super) async fn step_up(cloud: web::Data<Cloud>, device: Device, input: SafeJson<TwoFactorCode>) -> Answer {
  let stepped = cloud.step_up(device.0, input.0).await?;
  Ok(HttpResponse::Ok().json(stepped))
}

pub(super) async fn start_two_factor(
  cloud: web::Data<Cloud>,
  device: Device,
  address: ClientAddress,
  input: SafeJson<TwoFactorStart>,
) -> Answer {
  let setup = cloud.start_two_factor(device.0, input.0, address.0).await?;
  Ok(HttpResponse::Created().json(setup))
}

pub(super) async fn confirm_two_factor(
  cloud: web::Data<Cloud>,
  device: Device,
  input: SafeJson<TwoFactorCode>,
) -> Answer {
  let codes = cloud.confirm_two_factor(device.0, input.0).await?;
  Ok(HttpResponse::Ok().json(codes))
}

pub(super) async fn turn_off_two_factor(
  cloud: web::Data<Cloud>,
  device: Device,
  address: ClientAddress,
  input: SafeJson<TwoFactorOff>,
) -> Answer {
  cloud.turn_off_two_factor(device.0, input.0, address.0).await?;
  Ok(HttpResponse::NoContent().finish())
}

pub(super) async fn new_recovery_codes(
  cloud: web::Data<Cloud>,
  device: Device,
  input: SafeJson<TwoFactorCode>,
) -> Answer {
  let codes = cloud.new_recovery_codes(device.0, input.0).await?;
  Ok(HttpResponse::Ok().json(codes))
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
  Ok(HttpResponse::Ok().json(cloud.me(device.0).await?))
}

pub(super) async fn change_me(cloud: web::Data<Cloud>, device: Device, input: SafeJson<AccountChanges>) -> Answer {
  Ok(HttpResponse::Ok().json(cloud.change_account(device.0, input.0).await?))
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
