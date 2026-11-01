//! What a join came to, in place under what was pressed: joined, with *See your projects there*, which opens the
//! team's shared projects, the step's next page ([`super::shared`]); waiting for another computer's or an admin's
//! approval; or no place on the server for the account.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  node::Element,
};
use orchester_workspace::sharing::team_join::{JoinOutcome, JoinedServer};

use crate::{
  navigation::Navigation,
  routes::{JoinWay, Route},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    icon::Icon,
  },
};

/// How a join ended, with the server's name as the person knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
  Joined(JoinedServer),
  /// The server holds the join until another computer of the person's, or an admin, approves it.
  Waiting {
    server: String,
  },
  /// The server has no place for the account.
  NotInvited {
    server: String,
  },
}

impl DevtoolsInspectable for Outcome {}

impl Outcome {
  /// `outcome` of a join to the server named `server`.
  pub(super) fn of(outcome: JoinOutcome, server: &str) -> Self {
    match outcome {
      JoinOutcome::Joined(joined) => Self::Joined(joined),
      JoinOutcome::Waiting(_) => Self::Waiting {
        server: server.to_owned(),
      },
      JoinOutcome::NotInvited => Self::NotInvited {
        server: server.to_owned(),
      },
    }
  }
}

/// `outcome` as a callout whose ids start with `id`; `email` is the Orchester Cloud account's, if one is signed in.
pub(super) fn callout(ctx: &mut Ctx, id: &str, outcome: &Outcome, email: Option<&str>) -> Element {
  match outcome {
    Outcome::Joined(JoinedServer {
      id: server,
      name,
      username,
    }) => {
      let open = ActionButton::new(
        format!("{id}-open"),
        ButtonKind::Secondary,
        ctx.t("startup.join.outcome.joined.open"),
      )
      .icon(Icon::ArrowRight)
      .opens(Navigation::of(ctx).to(Route::JoinTeam(JoinWay::Projects(*server))));
      Callout::new(
        format!("{id}-joined"),
        CalloutTone::Success,
        ctx.t_args(
          "startup.join.outcome.joined.title",
          [("server", name.as_str()), ("username", username.as_str())],
        ),
      )
      .body(ctx.t("startup.join.outcome.joined.body"))
      .action(open)
      .into()
    }
    Outcome::Waiting { server } => Callout::new(
      format!("{id}-waiting"),
      CalloutTone::Info,
      ctx.t("startup.join.outcome.waiting.title"),
    )
    .icon(Icon::Hourglass)
    .body(ctx.t_args("startup.join.outcome.waiting.body", [("server", server.as_str())]))
    .into(),
    Outcome::NotInvited { server } => Callout::new(
      format!("{id}-not-invited"),
      CalloutTone::Warning,
      ctx.t_args(
        "startup.join.outcome.notInvited.title",
        [("server", server.as_str()), ("email", email.unwrap_or_default())],
      ),
    )
    .icon(Icon::UserX)
    .body(ctx.t("startup.join.outcome.notInvited.body"))
    .into(),
  }
}
