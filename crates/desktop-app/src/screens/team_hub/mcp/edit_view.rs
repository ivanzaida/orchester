//! What the panel of a team MCP server says of the last Save of an admin's edit
//! (`docs/design/team-configuration-live.md`, rule 6): one notice for what the team server answered, in place over the
//! form. What was typed is kept, except after a version someone published first, which the form then shows.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::resource::VersionNumber;
use orchester_workspace::{agents::team_mcp_edit::EditError, sharing::team_edit::EditRefused};

use crate::{
  screens::team_hub::model::Entry,
  theme::HubText,
  ui::{
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// What the team server answered to Save, in one notice: the title says what was refused, the words under it why and
/// what to do.
pub(super) fn refusal(ctx: &Ctx, refused: &EditError, entry: &Entry) -> Element {
  let version = entry
    .number
    .and_then(VersionNumber::next)
    .map_or_else(String::new, |next| next.to_string());
  let (title, body) = match refused {
    EditError::Invalid(invalid) => (
      ctx.t_args("team_hub.edit.failed.title", [("version", version)]),
      message_text(ctx, &(**invalid).clone().message()),
    ),
    EditError::Refused(EditRefused::Refused(refusal)) => (
      ctx.t_args(
        if refusal.is_local() {
          "team_hub.edit.rejectedHere.title"
        } else {
          "team_hub.edit.rejected.title"
        },
        [("version", version)],
      ),
      ctx.t_args(
        "team_hub.edit.rejected.body",
        [("reason", message_text(ctx, &refusal.message()).to_string())],
      ),
    ),
    EditError::Refused(changed @ EditRefused::Changed { latest }) => (
      ctx.t_args("team_hub.edit.superseded.title", [("version", latest.to_string())]),
      message_text(ctx, &changed.message()),
    ),
    EditError::Refused(EditRefused::Unreachable(reason)) => {
      (ctx.t("team_hub.edit.unreachable.title"), message_text(ctx, reason))
    }
    EditError::Refused(EditRefused::Failed(reason)) => (
      ctx.t_args("team_hub.edit.failed.title", [("version", version)]),
      message_text(ctx, reason),
    ),
  };
  Notice::new("team-hub-edit-refused", NoticeTone::Danger, title)
    .icon(Icon::CircleX)
    .title_style(HubText::NoticeTitle.role())
    .body(body)
    .stacked()
    .flat()
    .into()
}
