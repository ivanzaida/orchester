//! *Join a team*, a step of the start page (`docs/design/startup.md` decision 4; `Startup · redesign` `qzzYw`): this
//! computer gets its own sign-in on a team's server, with Orchester Cloud or with a code, and nothing local is
//! uploaded. The start page itself (ORC-293) has not landed: All projects' welcome opens the step, and its frame
//! ([`frame`]) is the one the start page's steps share.
//!
//! - Signed in to Orchester Cloud, `/join-team` shows the account's teams and invites (`04` `drCQd`, [`teams`]), and
//!   *Use a code* (`/join-team?way=code`) the two ways side by side, the account in the Cloud column's place.
//! - Signed out, both show the two ways (`02` `B914L`): the Cloud column signs in ([`cloud`]), and the code column
//!   waits, faded, meanwhile (`03` `BD519`). A server joined with a code while signed out is this computer's.
//! - The code column ([`code`]) joins with an address and a code; a wrong or expired code (`10` `aWsVx`) and a server
//!   that does not answer (`11` `oOn6j`) say so in place.
//! - Once joined, the team's shared projects and linking one to a folder ([`shared`], decisions 5–6).
//!
//! Nothing opens a dialog: what a join came to shows where it was asked. Settings › Orchester joins through the same
//! service ([`orchester_workspace::sharing::team_join`]).

mod cloud;
mod code;
mod code_join;
pub(crate) mod frame;
mod outcome;
mod rows;
mod shared;
mod teams;
mod ways;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_workspace::sharing::cloud::SignIn;

use self::{
  cloud::CloudSignIn,
  code_join::CodeJoin,
  frame::{StartStep, StepFrame, note, step_frame},
  teams::TeamJoins,
};
use crate::{root::follow_sign_in, routes::JoinWay, services::Services};

/// The step's screen on the way `way` names.
pub(super) fn screen(ctx: &mut Ctx, way: JoinWay) -> Element {
  ctx.mount_keyed::<JoinTeam>("join-team", Props { way })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Props {
  way: JoinWay,
}

impl DevtoolsInspectable for Props {}

/// The step. What was typed and asked stays while the person goes between the teams and the code.
struct JoinTeam {
  code: CodeJoin,
  teams: TeamJoins,
  sign_in: CloudSignIn,
}

impl Component for JoinTeam {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let email = match Services::of(ctx).cloud.sign_in_state() {
      SignIn::SignedOut { ended: Some(ended), .. } => ended.email,
      SignIn::SignedOut { ended: None, .. } | SignIn::SignedIn { .. } => String::new(),
    };
    Self {
      code: CodeJoin::new(ctx),
      teams: TeamJoins::new(ctx),
      sign_in: CloudSignIn::new(ctx, &email),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let way = ctx.props::<Props>().way.clone();
    let cloud = Services::of(ctx).cloud;
    match (follow_sign_in(ctx, &cloud), way) {
      // A team joined with a code has no Orchester Cloud account behind it: its projects show signed out too.
      (_, JoinWay::Projects(server)) => shared::projects_screen(ctx, server),
      (_, JoinWay::Folder(target)) => shared::link_screen(ctx, target),
      (SignIn::SignedIn { account, email }, JoinWay::Cloud) => teams::page(ctx, &self.teams, (&email, account)),
      (SignIn::SignedIn { account, email }, JoinWay::Code) => {
        let left = cloud::signed_in(ctx, &email, account);
        let right = code::column(ctx, &self.code, true);
        ways_page(ctx, ways::columns(left, right, false))
      }
      (SignIn::SignedOut { .. }, _) => {
        let waiting = self.sign_in.pending();
        let left = cloud::signed_out(ctx, &self.sign_in);
        let right = code::column(ctx, &self.code, false);
        ways_page(ctx, ways::columns(left, right, waiting))
      }
    }
  }
}

/// *Join a team* with the two ways in `columns` (`B914L`).
fn ways_page(ctx: &mut Ctx, columns: Element) -> Element {
  let frame = StepFrame {
    id: "join_team",
    ancestors: Vec::new(),
    crumb: ctx.t("startup.join.crumb").to_string(),
    step: StartStep::Join,
    title: ctx.t("startup.join.title").to_string(),
    lead: Some(ctx.t("startup.join.lead").to_string()),
    body: columns,
    note: Some(note("join-team-note", &ctx.t("startup.join.note"), None)),
  };
  step_frame(ctx, frame)
}
