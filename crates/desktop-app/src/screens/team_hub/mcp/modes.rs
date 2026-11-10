//! Which of its two states the panel of a team MCP server is in, and what each puts in the panel
//! (`docs/design/team-configuration-live.md`, rule 6): the team's part as values with the fix of the first missing part
//! pinned at the bottom (plain), or an admin's form over it under the fixed Save/Cancel bar the local panels have
//! (editing), with what the team server answered to the last Save over it. While the team server cannot be reached the
//! form stays and Save says why it waits.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row, Text},
  node::Element,
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
    team_hub::{actions::Busy, create::EditView, panel, rows},
  },
  theme::HubText,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    icon::Icon,
    message::message_text,
    pressable::Press,
  },
};

/// What the panel is doing with the team's part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
  /// Nothing is edited: the part is values, and an admin has *Edit*.
  Plain,
  /// An admin edits it as a form.
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
  pub above: Vec<Element>,
  pub from_team: Column,
  pub bar: Option<Element>,
}

impl McpPanel {
  /// An admin who pressed *Edit* edits, also once the team server cannot be reached, where Save then waits.
  pub(super) fn mode(&self, props: &Props) -> Mode {
    if props.admin && props.sheet.edit.is_some() && self.edit.editing.get() {
      Mode::Editing
    } else {
      Mode::Plain
    }
  }

  /// What the state adds to the panel; `None` for the plain panel.
  pub(super) fn parts(&self, ctx: &mut Ctx, props: &Props, mode: Mode) -> Option<Parts> {
    match mode {
      Mode::Plain => None,
      Mode::Editing => self.editing(ctx, props),
    }
  }

  fn editing(&self, ctx: &mut Ctx, props: &Props) -> Option<Parts> {
    let fields = self.edit.fields.as_ref()?;
    let base = self.edit.base.lock().clone()?;
    let pending = self.edit.save.state().get().is_pending();
    let form = fields.edit(
      ctx,
      &EditView {
        listing: &props.listing,
        published: &base.published,
        locked: pending,
      },
    );
    let save = if pending {
      Save::Saving
    } else if let Some(unreached) = &props.unreached {
      Save::Waits(message_text(ctx, unreached))
    } else if form.changed.is_empty() {
      Save::Unchanged
    } else if !form.valid {
      Save::Waits(ctx.t("team_hub.edit.fixFields"))
    } else {
      Save::Ready(self.save_press(fields.input(), &base))
    };
    let cancel = (!pending).then(|| {
      let edit = self.edit.clone();
      Press::run(move || edit.cancel())
    });
    let bar = save_bar::bar(
      ctx,
      "team-hub-edit",
      (save, cancel),
      save_bar::makes(ctx, Some(base.number)),
    );
    let above = self
      .edit
      .refused
      .get()
      .map(|(refused, _)| edit_view::refusal(ctx, &refused, &props.entry))
      .into_iter()
      .collect();
    let note = ctx.t("team_hub.edit.fromTeamEditing");
    Some(Parts {
      chips: Chips {
        badges: vec![readiness_chip(ctx, props)],
        end: panel::used_by(ctx, &props.sheet.used_by),
      },
      above,
      from_team: section(ctx, vec![note_text("team-hub-from-team-note", &note).into()], form.rows),
      bar: Some(bar),
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
  pub(super) fn plain_parts(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    (busy, press): (Busy, &dyn Fn(&Step) -> Press),
  ) -> Parts {
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
      from_team: from_team::section(ctx, sheet),
      bar: Bar {
        missing,
        changes_path: sheet.program.is_some().then_some(entry.id),
        busy,
        press,
      }
      .render(ctx),
    }
  }

  /// *Edit* in the head of an admin's panel: the form opens on the version the panel shows.
  pub(super) fn edit_button(&self, ctx: &Ctx, props: &Props) -> Option<Element> {
    props.team.may_change.then_some(())?;
    let latest = props.sheet.edit.clone()?;
    let edit = self.edit.clone();
    Some(
      ActionButton::new("team-hub-edit", ButtonKind::Ghost, ctx.t("team_hub.panel.edit"))
        .icon(Icon::Pencil)
        .on_click(move || edit.open(&latest))
        .into(),
    )
  }
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

pub(super) fn note_text(id: &str, words: &str) -> Text {
  Text::new(words)
    .id(id.to_owned())
    .variant(HubText::PanelHint.role())
    .nowrap()
}

/// *From the team* over `rows`, with `after` on its line.
pub(super) fn section(ctx: &Ctx, after: Vec<Element>, rows: Vec<Row>) -> Column {
  rows::section("team-hub-from-team", &ctx.t("team_hub.profile.fromTeam"), after, rows)
}
