//! A team MCP server whose edit the team server refused and that waits in the outbox, as the person finds it after
//! they left the panel or a later send was refused (`Agents · Profile · admin · refused` A17 `rJbPm`): the refusal in
//! red with its reason, the form of the refused version, and the bar of an edit. *Publish for the team* takes the
//! version back as the draft and sends what is typed; *Discard draft* takes it back and drops it.

use orchester_core::{id::VersionId, time::Timestamp};
use orchester_store::PublishState;
use orchester_workspace::{
  agents::team_mcp_edit::McpEdit,
  sharing::team_edit::{EditRefused, Refusal},
};

use super::{
  McpPanel, Props,
  edit_view::{self, EditBar},
  modes::{Chips, Parts, note_text, readiness_chip, section},
};
use crate::{
  screens::team_hub::{create::EditView, panel},
  ui::chip::BadgeTone,
};

impl McpPanel {
  /// The panel of the refused version `version`, which the team server refused at `at`. `None` when the outbox does not
  /// hold it as refused any more or the form cannot hold the server.
  pub(super) fn refused(
    &self,
    ctx: &mut lurq::app::ctx::Ctx,
    props: &Props,
    edit: &McpEdit,
    (version, at): (VersionId, Timestamp),
  ) -> Option<Parts> {
    let fields = self.edit.fields.as_ref()?;
    let standing = props.team.standing(props.entry.id)?;
    let PublishState::Refused { code } = &standing.waiting.first()?.state else {
      return None;
    };
    let pending = self.edit.publish.state().get().is_pending();
    let form = fields.edit(
      ctx,
      &EditView {
        listing: &props.listing,
        published: &edit.published,
        locked: pending,
      },
    );
    // A press that failed before the version went back to being the draft says why in the notice's place.
    let said = match self.edit.refused.get() {
      Some((said, _)) => said,
      None => EditRefused::Refused(Refusal::of_code(code)),
    };
    let publishable = form.valid && !form.changed.is_empty() && !pending;
    let bar = EditBar {
      publish: self.publish_button(ctx, (fields.input(), Some(version)), publishable, pending),
      discard: self.discard_button(
        ctx,
        "team-hub-edit-discard",
        "team_hub.edit.bar.discard",
        (edit, Some(version)),
      ),
      refused_at: Some(at),
      team: &props.team,
    }
    .render(ctx);
    let number = props.entry.number.unwrap_or(edit.number);
    let note = ctx.t("team_hub.edit.fromTeamEditing");
    Some(Parts {
      chips: Chips {
        badges: vec![
          (
            ctx.t_args("team_hub.edit.chip.draft", [("version", number.to_string())]),
            BadgeTone::Warning,
          ),
          readiness_chip(ctx, props),
        ],
        end: panel::used_by(ctx, &props.sheet.used_by),
      },
      above: vec![edit_view::refusal(ctx, &said, &props.entry)],
      from_team: section(ctx, vec![note_text("team-hub-from-team-note", &note).into()], form.rows),
      computer: true,
      bar: Some(bar),
    })
  }
}
