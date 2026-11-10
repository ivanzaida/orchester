//! What the form that creates a resource says when the team server answered otherwise than by taking it (`Create
//! refused · name taken in the team` A15 `dLqHC`, and as the hub's edit says them): one notice in place, over the form,
//! which keeps everything typed. A team server that cannot be reached creates nothing, and the notice says why.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::sharing::team_edit::Refusal;

use super::work::Failed;
use crate::ui::{
  icon::Icon,
  message::message_text,
  notice::{Notice, NoticeTone},
  tinted_note::person_only,
};

/// Why the team server did not create the resource, or this computer did not send it.
pub(super) fn refused(ctx: &Ctx, failed: &Failed, admins: &str) -> Element {
  let danger = |id: &str, title: String| {
    Notice::new(id.to_owned(), NoticeTone::Danger, title)
      .icon(Icon::CircleX)
      .stacked()
  };
  match failed {
    Failed::PersonOnly => person_only("team-new-person-only", &ctx.t("team_new.personOnly")).into(),
    Failed::Refused(Refusal::NameTaken, name) => danger(
      "team-new-refused",
      ctx
        .t_args("team_new.refused.nameTaken.title", [("name", name.as_str())])
        .to_string(),
    )
    .body(ctx.t("team_new.refused.nameTaken.body"))
    .into(),
    Failed::Refused(Refusal::NotAdmin, _) => Notice::new(
      "team-new-refused",
      NoticeTone::Danger,
      ctx.t("team_new.refused.notAdmin.title"),
    )
    .icon(Icon::ShieldX)
    .body(ctx.t_args("team_new.refused.notAdmin.body", [("admins", admins)]))
    .stacked()
    .into(),
    Failed::Refused(refusal, name) => danger(
      "team-new-refused",
      ctx
        .t_args("team_new.refused.rejected.title", [("name", name.as_str())])
        .to_string(),
    )
    .body(ctx.t_args(
      "team_new.refused.rejected.body",
      [("reason", message_text(ctx, &refusal.message()).to_string())],
    ))
    .into(),
    Failed::Said(reason) => danger("team-new-refused", ctx.t("team_new.refused.failed.title").to_string())
      .body(message_text(ctx, reason))
      .into(),
  }
}
