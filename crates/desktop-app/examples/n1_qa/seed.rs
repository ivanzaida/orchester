//! Each state's seed: what the person would have done in the window (signing in, accepting an invite), done through
//! the workspace before the window opens, and what time would have done to the invite kept here, written to the
//! application database.

use std::{error::Error, path::Path};

use orchester_api::{
  Credential,
  client::{CloudAddress, ServerAddress},
};
use orchester_app::{
  routes::{OrchesterPanel, OrchesterView, Route},
  services::Services,
};
use orchester_core::time::Timestamp;
use orchester_store::{AcceptedInvite, AppDb};
use orchester_workspace::{sharing::CloudJoined, workspace::Workspace};
use tokio::runtime::Runtime;

use super::{
  State,
  account_cloud::{EMAIL, OTHER_EMAIL, PASSWORD},
  backends::Backends,
  project,
};

/// Seeds `state` for the window of `services` and `workspace`, whose data is in `folder`; where it opens.
pub fn seed(
  state: State,
  (backends, services, workspace): (&Backends, &Services, &Workspace),
  (runtime, folder): (&Runtime, &Path),
) -> Result<Route, Box<dyn Error>> {
  sign_in(backends, services, runtime)?;
  let cloud = &services.cloud;
  let settings = |panel| Route::Orchester(OrchesterView { server: None, panel });
  if matches!(state, State::Project | State::LinkConfirm | State::Linked) {
    return project::seed(state, (backends, services, workspace), (runtime, folder));
  }
  if state == State::OtherAccount {
    project::seed(State::Linked, (backends, services, workspace), (runtime, folder))?;
    runtime.block_on(async {
      cloud.sign_out().await?;
      cloud
        .sign_in(OTHER_EMAIL.into(), Credential::new(PASSWORD.into()))
        .await?;
      Ok::<_, Box<dyn Error>>(())
    })?;
    return Ok(settings(OrchesterPanel::Account));
  }
  if state == State::SignedIn {
    backends.invite_to_billing();
    return Ok(settings(OrchesterPanel::Account));
  }
  let database = folder.join("orchester.db");
  let database = database.as_path();
  if matches!(state, State::Pending | State::Failed) {
    failed_join(backends, services, runtime)?;
    if state == State::Pending {
      // Accepted, and nothing asked for it yet: as a computer that closed before its first ask leaves it.
      rekeep(database, |kept| AcceptedInvite { asks: 0, ..kept })?;
    }
    return Ok(settings(OrchesterPanel::Invites));
  }
  waiting_join(backends, services, runtime, &folder.join("other-computer"))?;
  let due = Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 1000);
  match state {
    State::Unanswered => rekeep(database, |kept| AcceptedInvite {
      next_check_at: None,
      ..kept
    })?,
    State::TooMany => rekeep(database, |kept| AcceptedInvite {
      asks: orchester_workspace::sharing::cloud::MAX_ASKS,
      next_check_at: None,
      ..kept
    })?,
    State::Approved | State::Refused => {
      let answered = if state == State::Approved {
        backends.server.approve_waiting()
      } else {
        backends.server.refuse_waiting()
      };
      if answered != 1 {
        return Err(format!("{answered} joins waited for approval, not one").into());
      }
      // The check asks again as Settings › Orchester opens.
      rekeep(database, |kept| AcceptedInvite {
        next_check_at: Some(due),
        ..kept
      })?;
    }
    _ => {}
  }
  Ok(settings(OrchesterPanel::Invites))
}

/// Signs the window in to the stand-in cloud of `backends`, as the person would at the gate.
pub fn sign_in(backends: &Backends, services: &Services, runtime: &Runtime) -> Result<(), Box<dyn Error>> {
  let cloud = &services.cloud;
  runtime.block_on(async {
    // The stand-in is the cloud's web and API address, its API at `/v1` as the public cloud's is.
    let address = ServerAddress::parse(&backends.cloud.url())?;
    cloud
      .set_address(Some(CloudAddress::new(address.clone(), Some(address))))
      .await?;
    cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())).await?;
    Ok::<_, Box<dyn Error>>(())
  })
}

/// Accepts the Billing invite while the cloud names an address where nothing listens: the join fails after the
/// cloud issued its token, and the invite stays here to try again, which then reaches the server.
fn failed_join(backends: &Backends, services: &Services, runtime: &Runtime) -> Result<(), Box<dyn Error>> {
  let invite = backends.invite_to_billing();
  backends.named_at(Some("http://127.0.0.1:9"));
  let accepted = runtime.block_on(async {
    let listed = services.cloud.invites().await?;
    let invite = listed
      .iter()
      .find(|listed| listed.id.to_string() == invite)
      .ok_or("the invite is listed")?;
    Ok::<_, Box<dyn Error>>(services.cloud.accept(invite).await)
  })?;
  backends.named_at(None);
  match accepted {
    Err(_) => Ok(()),
    Ok(accepted) => Err(format!("the join was meant to fail: {:?}", accepted.joined).into()),
  }
}

/// Accepts the Billing invite on a server that asks for approval: the join waits.
fn waiting_join(
  backends: &Backends,
  services: &Services,
  runtime: &Runtime,
  other: &Path,
) -> Result<(), Box<dyn Error>> {
  backends.ask_for_approval(other);
  let invite = backends.invite_to_billing();
  let accepted = runtime.block_on(async {
    let listed = services.cloud.invites().await?;
    let invite = listed
      .iter()
      .find(|listed| listed.id.to_string() == invite)
      .ok_or("the invite is listed")?;
    Ok::<_, Box<dyn Error>>(services.cloud.accept(invite).await?)
  })?;
  match accepted.joined {
    CloudJoined::Waiting(_) => Ok(()),
    CloudJoined::Joined(_) => Err("the join was meant to wait for approval".into()),
  }
}

/// Changes what the database keeps of its one accepted invite, as time would have.
fn rekeep(database: &Path, change: impl FnOnce(AcceptedInvite) -> AcceptedInvite) -> Result<(), Box<dyn Error>> {
  let mut db = AppDb::open(database)?;
  let kept = db
    .accepted_invites(Timestamp::now())?
    .pop()
    .ok_or("an invite is kept")?;
  let invite = kept.invite;
  // Asks never go down when kept again; forgotten first, the change stands as it is.
  db.forget_accepted_invite(invite)?;
  db.keep_accepted_invite(&change(kept))?;
  Ok(())
}
