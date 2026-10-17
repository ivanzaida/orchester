//! The two-factor states of Settings › Orchester and the All projects notice (layer `uyO5v`: `hC11x`, `rPG8h`, board
//! `qwDH9`), seeded on the stand-in cloud after the seed's sign-in: two-factor authentication off, this computer
//! signed in without a code, an accept the cloud refused for that (pressed by the example itself before the window
//! shows), and few recovery codes left after a sign-in with one (no frame).

use std::{error::Error, path::Path};

use lurq::app::{App, Tree};
use orchester_api::Credential;
use orchester_app::{
  routes::{OrchesterPanel, OrchesterView, Route},
  services::Services,
};
use orchester_core::time::Timestamp;
use orchester_store::{AcceptedInvite, AppDb};
use orchester_workspace::{
  sharing::cloud::{CodeKind, SignInStep},
  workspace::Workspace,
};
use tokio::runtime::Runtime;

use super::{
  State,
  account_cloud::{AccountCloud, EMAIL, PASSWORD, RECOVERY_CODES},
  backends::Backends,
  project,
};

/// A press the example makes before the window shows: once `button` shows, this computer's session loses its code (as
/// one signed in before two-factor authentication was turned on), `button` is pressed, and the window waits for
/// `shows`.
pub struct Press {
  pub button: String,
  pub shows: String,
}

/// Seeds the two-factor `state` in `folder`; where the window opens, and the press it needs. All projects shows its
/// notice beside a project, so the notice's states register Kontur.
pub fn seed(
  state: State,
  (backends, services, workspace): (&Backends, &Services, &Workspace),
  (runtime, folder): (&Runtime, &Path),
) -> Result<(Route, Option<Press>), Box<dyn Error>> {
  if matches!(state, State::NoticeOff | State::NoticeRefused) {
    project::register_kontur(workspace, folder)?;
  }
  let settings = Route::Orchester(OrchesterView {
    server: None,
    panel: OrchesterPanel::Account,
  });
  let cloud = &backends.cloud;
  if state == State::FewRecoveryCodes {
    cloud.require_codes();
    cloud.use_recovery_codes(RECOVERY_CODES.len() - 2);
    sign_in_with_a_recovery_code(services, runtime)?;
    return Ok((settings, None));
  }
  if matches!(
    state,
    State::KeptTwoFactorOff | State::KeptSignInAgain | State::KeptTryAgain
  ) {
    kept(state, (backends, services), (runtime, folder))?;
    return Ok((settings, None));
  }
  let invite = backends.invite_to_billing();
  let refused = |card: &str| Press {
    button: format!("{card}-accept"),
    shows: format!("{card}-sign-in-needed"),
  };
  Ok(match state {
    State::TwoFactorOff => {
      cloud.two_factor_off();
      (settings, None)
    }
    State::NoticeOff => {
      cloud.two_factor_off();
      (Route::AllProjects, None)
    }
    State::NotNow => {
      cloud.session_without_two_factor();
      (settings, None)
    }
    State::SignInAgain => (settings, Some(refused(&format!("orchester-invite-{invite}")))),
    State::NoticeRefused => (
      Route::AllProjects,
      Some(refused(&format!("all-projects-invite-{invite}"))),
    ),
    _ => return Err(format!("{state:?} is not a two-factor state").into()),
  })
}

/// Keeps the Billing invite as an earlier ask left it (its token taken, so the cloud lists it no more), then leaves
/// the account as `state` needs: two-factor authentication off, this computer signed in without a code, or signed in
/// again with one.
fn kept(
  state: State,
  (backends, services): (&Backends, &Services),
  (runtime, folder): (&Runtime, &Path),
) -> Result<(), Box<dyn Error>> {
  let invite = backends.invite_to_billing();
  let listed = runtime
    .block_on(services.cloud.invites())?
    .into_iter()
    .find(|listed| listed.id.to_string() == invite)
    .ok_or("the invite to Billing is listed")?;
  let kept = AcceptedInvite {
    asks: 1,
    // Its last ask was refused for want of two-factor authentication.
    needs_code: true,
    ..AcceptedInvite::new(
      (listed.id, listed.server.id, listed.server.name.clone()),
      listed.project.as_ref().map(|project| project.name.clone()),
      Timestamp::from_unix_millis(listed.expires_at),
      Timestamp::now(),
    )
  };
  AppDb::open(&folder.join("orchester.db"))?.keep_accepted_invite(&kept)?;
  let cloud = &backends.cloud;
  match state {
    State::KeptTwoFactorOff => cloud.two_factor_off(),
    State::KeptSignInAgain => cloud.session_without_two_factor(),
    _ => {
      cloud.require_codes();
      sign_in_with_the_apps_code(services, runtime, cloud)?;
    }
  }
  Ok(())
}

/// Signs out, then in again with the password and the code the authenticator app shows.
fn sign_in_with_the_apps_code(
  services: &Services,
  runtime: &Runtime,
  stand_in: &AccountCloud,
) -> Result<(), Box<dyn Error>> {
  let cloud = &services.cloud;
  runtime.block_on(async {
    cloud.sign_out_for_code(None).await?;
    let SignInStep::Code(challenge) = cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())).await? else {
      return Err("the password was meant to answer a challenge".into());
    };
    let code = Credential::new(stand_in.next_code());
    cloud.finish_sign_in(&challenge, (code, CodeKind::App)).await?;
    Ok::<_, Box<dyn Error>>(())
  })
}

/// Signs out, then in again with the password and one of the last recovery codes.
fn sign_in_with_a_recovery_code(services: &Services, runtime: &Runtime) -> Result<(), Box<dyn Error>> {
  let cloud = &services.cloud;
  runtime.block_on(async {
    cloud.sign_out().await?;
    let SignInStep::Code(challenge) = cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())).await? else {
      return Err("the password was meant to answer a challenge".into());
    };
    let code = Credential::new(RECOVERY_CODES[RECOVERY_CODES.len() - 2].to_owned());
    cloud.finish_sign_in(&challenge, (code, CodeKind::Recovery)).await?;
    Ok::<_, Box<dyn Error>>(())
  })
}

/// Makes `press` in the window before it shows: waits for its button, takes the code from this computer's session at
/// `cloud`, presses the button with the example's own pointer and waits for what it shows.
pub fn drive(app: &mut App, tree: &mut Tree, (press, cloud): (&Press, &AccountCloud)) -> Result<(), Box<dyn Error>> {
  project::until(app, tree, &press.button)?;
  cloud.session_without_two_factor();
  project::press(app, tree, &press.button)?;
  project::until(app, tree, &press.shows)
}
