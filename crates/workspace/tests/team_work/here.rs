//! What this computer holds of a team project's claims and runs (`team_work::team_here`), as the screens read it to say
//! whether a claim is this computer's: a claim it took and has not released, the claim its team run started under, and
//! a run the server fenced, with why.

use orchester_api::v2::{AuthorityRef, Subject};
use orchester_store::TeamFence;
use orchester_workspace::{
  actions::{Action, ActionOutcome, ClaimAction, Intent, TeamOutcome},
  team_work::{TeamHere, team_here},
};

use crate::{
  Checkout, started,
  team_server::{MEMBER, TeamServer},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_here_holds_what_this_computer_took_until_it_lets_go_or_loses_it() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let here = || team_here(checkout.root()).expect("read");
  assert_eq!(here(), TeamHere::default(), "nothing taken yet");

  let claim = checkout.claim(&server, MEMBER).await;
  assert!(here().holds(claim), "the claim it took");

  let (dispatcher, _generation) = checkout.dispatcher(&server, MEMBER);
  let release = Action::Claim(ClaimAction::Release {
    authority: AuthorityRef {
      subject: Subject::Task { id: server.task },
      claim_id: claim,
      work_id: None,
    },
  });
  let outcome = dispatcher.dispatch(&Intent::new(release)).await.expect("dispatched");
  assert!(
    matches!(outcome, ActionOutcome::Team(TeamOutcome::Completed(_))),
    "{outcome:?}"
  );
  assert!(!here().holds(claim), "released");

  let (work, run, _generation) = started(&checkout, &server).await;
  let held = server.state().claim.clone().expect("held").id;
  assert!(here().holds(held), "the claim its run started under");
  assert_eq!(here().fence(run), None);

  crate::take_over(&server, held).await;
  work.heartbeat().await.expect("sent");
  assert_eq!(here().fence(run), Some(TeamFence::ClaimLost), "taken over");
  assert_eq!(here().unsent, [(run, 0)], "every report it queued was acknowledged");
}
