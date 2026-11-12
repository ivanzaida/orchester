//! Which of its two states the panel of a team MCP server is in, and what each puts in the panel
//! (`docs/design/team-configuration-live.md`, rule 6): the team's part as values with the fix of the first missing part
//! pinned at the bottom (plain, as a member reads it), or, for an admin, the form that makes one, edited in place as
//! every other team kind is (A26–A31): *Unsaved changes* after *Team · vN* while the fields differ from the version
//! they are made on, the fixed Save/Cancel bar with what Save publishes at its end, and what the team server answered
//! to the last Save over the form. While nothing is changed the fix of the first missing part keeps its pinned place.
//! While the team server cannot be reached the form stays and Save says why it waits.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row},
};
use orchester_workspace::{agents::team_mcp_edit::McpEdit, app_mcp::setup::NewTeamMcpServer};

use super::{
  McpPanel, Props,
  edit::SaveAsked,
  edit_view,
  fix::{Bar, Step},
  from_team, words,
};
use crate::{
  mcp::input_gate::{Guarded, may_press},
  screens::{
    local_hub::save_bar::{self, Save},
    team_hub::{actions::Busy, create::EditView, edit::hint, panel, rows},
  },
  ui::{chip::BadgeTone, message::message_text, pressable::Press},
};

/// What the panel is doing with the team's part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
  /// The part is values: a member's panel, or a server the form cannot hold.
  Plain,
  /// An admin edits it in place.
  Editing,
}

/// The chips after *Team · vN*: their words and what stands at their end.
pub(super) struct Chips {
  pub badges: Vec<(Arc<str>, BadgeTone)>,
  pub end: Option<Arc<str>>,
}

/// What a state puts in the panel besides its head.
pub(super) struct Parts {
  pub chips: Chips,
  /// What stands over *From the team*: what the team server answered to the last Save.
  pub above: Vec<lurq::node::Element>,
  pub from_team: Column,
  pub bar: Option<lurq::node::Element>,
}

impl McpPanel {
  /// An admin edits in place, also once the team server cannot be reached, where Save then waits.
  pub(super) fn mode(&self, props: &Props) -> Mode {
    if props.admin && props.sheet.edit.is_some() && self.edit.fields.is_some() {
      Mode::Editing
    } else {
      Mode::Plain
    }
  }

  /// What the state adds to the panel: the fix of the first missing part is pinned while nothing is edited.
  pub(super) fn parts(&self, ctx: &mut Ctx, props: &Props, (busy, press): (Busy, &dyn Fn(&Step) -> Press)) -> Parts {
    if self.mode(props) == Mode::Editing
      && let Some(parts) = self.editing(ctx, props, fix_bar(ctx, props, (busy, press)))
    {
      return parts;
    }
    self.plain_parts(ctx, props, (busy, press))
  }

  fn editing(&self, ctx: &mut Ctx, props: &Props, fix: Option<lurq::node::Element>) -> Option<Parts> {
    let fields = self.edit.fields.as_ref()?;
    let base = self.edit.base()?;
    let pending = self.edit.save.state().get().is_pending();
    let form = fields.edit(
      ctx,
      &EditView {
        listing: &props.listing,
        published: &base.published,
        locked: pending,
      },
    );
    let changed = !form.changed.is_empty();
    self.edit.keep_up(props.sheet.edit.as_ref(), changed);
    let save = if pending {
      Save::Saving
    } else if let Some(unreached) = &props.unreached {
      Save::Waits(message_text(ctx, unreached))
    } else if !changed {
      Save::Unchanged
    } else if !form.valid {
      Save::Waits(ctx.t("team_hub.edit.fixFields"))
    } else {
      Save::Ready(self.save_press(fields.input(), &base))
    };
    let cancel = (changed && !pending).then(|| {
      let edit = self.edit.clone();
      Press::run(move || edit.cancel())
    });
    let refused = self.edit.refused.get();
    let said = hint(ctx, base.number, changed, props.unreached.is_some());
    let save_bar = save_bar::bar(ctx, "team-hub-edit", (save, cancel), said);
    let holds = changed || refused.is_some();
    let bar = if holds { Some(save_bar) } else { fix.or(Some(save_bar)) };
    let mut badges = Vec::new();
    if changed {
      badges.push((ctx.t("team_hub.edit.unsaved"), BadgeTone::Warning));
    }
    badges.push(readiness_chip(ctx, props));
    Some(Parts {
      chips: Chips {
        badges,
        end: panel::used_by(ctx, &props.sheet.used_by),
      },
      above: refused
        .map(|(refused, _)| edit_view::refusal(ctx, &refused, &props.entry))
        .into_iter()
        .collect(),
      from_team: section(ctx, form.rows),
      bar,
    })
  }

  /// Save: a person's press (or automation's where they let it change what agents may do) sends the fields as typed,
  /// made on the version the form was opened on; automation's press otherwise is refused to it, and the panel stays as
  /// it was.
  fn save_press(&self, input: NewTeamMcpServer, base: &McpEdit) -> Press {
    let (save, gate, number) = (self.edit.save.clone(), self.gate.clone(), base.number);
    Press::run(move || {
      if may_press(
        gate.as_ref(),
        Guarded::PermissionWidening,
        "publish a new version of a team MCP server",
      ) {
        save.run(SaveAsked {
          input: input.clone(),
          base: number,
        });
      }
    })
  }

  /// The panel that edits nothing: the readiness chip, the team's part as values and the fix of the first missing part
  /// pinned at the bottom.
  fn plain_parts(&self, ctx: &mut Ctx, props: &Props, (busy, press): (Busy, &dyn Fn(&Step) -> Press)) -> Parts {
    let Props {
      listing, entry, sheet, ..
    } = props;
    let missing = entry.standing.as_ref().map_or(&[][..], |standing| standing.missing());
    Parts {
      chips: Chips {
        badges: vec![words::chip(ctx, listing, entry, missing)],
        end: panel::used_by(ctx, &sheet.used_by),
      },
      above: Vec::new(),
      from_team: from_team::section(ctx, sheet, rows::FromTeam::Shared),
      bar: fix_bar(ctx, props, (busy, press)),
    }
  }
}

/// The fix of the first missing part, pinned at the bottom; `None` when nothing is missing and nothing is offered.
fn fix_bar(ctx: &Ctx, props: &Props, (busy, press): (Busy, &dyn Fn(&Step) -> Press)) -> Option<lurq::node::Element> {
  let missing = props
    .entry
    .standing
    .as_ref()
    .map_or(&[][..], |standing| standing.missing());
  Bar {
    missing,
    changes_path: props.sheet.program.is_some().then_some(props.entry.id),
    busy,
    press,
  }
  .render(ctx)
}

/// The panel's readiness chip.
pub(super) fn readiness_chip(ctx: &Ctx, props: &Props) -> (Arc<str>, BadgeTone) {
  let missing = props
    .entry
    .standing
    .as_ref()
    .map_or(&[][..], |standing| standing.missing());
  words::chip(ctx, &props.listing, &props.entry, missing)
}

/// *From the team* over `rows`.
pub(super) fn section(ctx: &Ctx, rows: Vec<Row>) -> Column {
  rows::section(
    "team-hub-from-team",
    &ctx.t("team_hub.profile.fromTeam"),
    Vec::new(),
    rows,
  )
}
