//! What the hub tells when the team server refused a new resource only after the form was gone (it waited in the
//! outbox, and was sent once the server answered): the resource is taken back whole, as one refused at once, and each
//! says what it was and why, until the person dismisses it.

use std::path::{Path, PathBuf};

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::sharing::{TakenBack, team::TeamConfiguration};

use crate::{
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// One notice for the new resources of the project at `root` that were taken back; none while there are none.
pub(super) fn notices(ctx: &mut Ctx, team: &TeamConfiguration, root: &Path) -> Vec<Element> {
  if team.taken_back.is_empty() {
    return Vec::new();
  }
  let services = Services::of(ctx);
  let root: PathBuf = root.to_path_buf();
  team
    .taken_back
    .iter()
    .enumerate()
    .map(|(index, taken)| {
      let (servers, root) = (services.servers.clone(), root.clone());
      notice(ctx, index, taken, move || servers.dismiss_taken_back(&root))
    })
    .collect()
}

fn notice(ctx: &Ctx, index: usize, taken: &TakenBack, dismiss: impl Fn() + Send + Sync + 'static) -> Element {
  Notice::new(
    format!("team-hub-taken-back-{index}"),
    NoticeTone::Warning,
    ctx.t_args("team_hub.takenBack.title", [("name", taken.name.clone())]),
  )
  .icon(Icon::CircleAlert)
  .body(message_text(ctx, &taken.reason))
  .action(
    ActionButton::new(
      format!("team-hub-taken-back-dismiss-{index}"),
      ButtonKind::Secondary,
      ctx.t("team_hub.takenBack.dismiss"),
    )
    .on_click(dismiss),
  )
  .into()
}
