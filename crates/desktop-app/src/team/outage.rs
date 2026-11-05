//! A team project's page while its team data cannot be read (`06d · Module · team data unavailable` `B7Mbe`, its card
//! `State · team data unavailable` `dmgtg`): the card in the page's body, a 36 tile with the 18 `cloud-off`, what
//! cannot be read now in 15/600, why in 12.5 `text_secondary` and *Try again*, which opens the team's reads anew and
//! asks the server at once. The same card says why when this computer is signed out of the server, the person is no
//! longer a member, or the server refused the read. Nothing read before is shown as current, and nothing is kept to
//! show.

use lurq::app::ctx::Ctx;
use orchester_core::id::ServerId;

use super::TeamFailure;
use crate::{
  screens::settings::orchester::moment,
  services::{Services, updates},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    state::EmptyState,
  },
};

/// Where the project lives: its server's id and name; `None` while it is not read.
pub type Place = Option<(ServerId, String)>;

/// What a page cannot read: the card's element `id`, its title, and the key of why while the server does not answer
/// (with `{{server}}` and `{{time}}`).
pub struct Unread<'a> {
  pub id: &'a str,
  pub title: std::sync::Arc<str>,
  pub unavailable: &'a str,
}

/// The card for `failure` in the project `project` at `place`.
pub fn card(ctx: &mut Ctx, unread: &Unread<'_>, (failure, place, project): (&TeamFailure, &Place, &str)) -> EmptyState {
  let server = place.as_ref().map_or("", |(_, name)| name.as_str());
  let body = match failure {
    TeamFailure::Unavailable { since } => ctx.t_args(
      unread.unavailable,
      [("server", server.to_owned()), ("time", moment(ctx, *since))],
    ),
    TeamFailure::SignedOut => ctx.t_args("tasks.team.signedOut", [("server", server), ("project", project)]),
    TeamFailure::NotMember => ctx.t_args("tasks.team.notMember", [("server", server), ("project", project)]),
    TeamFailure::Unsupported { required } => ctx.t_args(
      "tasks.team.unsupported.body",
      [
        ("server", server),
        ("version", required.as_str()),
        ("current", updates::version().as_str()),
      ],
    ),
    TeamFailure::Refused => ctx.t_args("tasks.team.refused.body", [("server", server)]),
  };
  let services = Services::of(ctx);
  let (team, servers) = (services.team, services.servers);
  let server = place.as_ref().map(|(id, _)| *id);
  let again = ActionButton::new(
    format!("{}-try-again", unread.id),
    ButtonKind::Primary,
    ctx.t("common.tryAgain"),
  )
  .icon(Icon::RefreshCw)
  .on_click(move || {
    team.renew();
    if let Some(server) = server {
      servers.retry_now(server);
    }
  });
  EmptyState::new(unread.id, Icon::CloudOff, unread.title.clone(), body)
    .wide()
    .action(again)
}
