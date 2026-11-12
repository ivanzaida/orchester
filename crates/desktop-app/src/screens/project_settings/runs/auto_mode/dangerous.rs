//! Choosing *Dangerous auto-approve* as the Approve mode asks first (`docs/design/approval-modes.md`, the person's
//! decision of 2026-11-11; `Dialog` `T4bbC` in `Cuj66` of `Orchestrator WIP`): the app's confirmation dialog, 480 wide,
//! its tile `triangle-alert` in `danger`, what will happen, the four things an agent can then do (each a 13 glyph in
//! `danger` and its words in 12.5/500, in one `border` outline on `surface_panel`, rows 7 by 12 and ruled from each
//! other), and that it stays on until it is turned off here. Enter is bound to nothing, so a stray key cannot choose
//! it; Esc, *Cancel* (which has the focus) and the scrim cancel. Only the danger button turns it on, and only a
//! person's press of it: an MCP client's press is refused and said in the dialog. Moving the mode down asks nothing.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::{Answerer, ApproveMode};
use orchester_workspace::project_settings::person_only as person_only_message;

use crate::{
  mcp::input_gate::{Guarded, InputGate, answerer},
  screens::project_settings::work::{Change, Save},
  theme::{DialogText, ExtraSpacing, ExtraTypography, IconSize, ProjectSettingsSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone},
    icon::{Icon, icon},
    message::message_text,
    tinted_note::person_only,
  },
};

/// The dialog's width (`Dialog` `T4bbC`).
const DIALOG_WIDTH: f32 = 480.0;

/// What an agent can do once every request is approved, in the design's order (`V0Gmt8`).
const RISKS: [(Icon, &str); 4] = [
  (Icon::GitBranch, "project_settings.dangerousDialog.push"),
  (Icon::Trash2, "project_settings.dangerousDialog.delete"),
  (Icon::KeyRound, "project_settings.dangerousDialog.credentials"),
  (Icon::Terminal, "project_settings.dangerousDialog.run"),
];

/// The dialog's state: whether it shows, *Cancel* to focus as it opens, and whether an MCP client's press of the
/// danger button was refused.
#[derive(Clone)]
pub(super) struct Confirmation {
  pub(super) open: Signal<bool>,
  cancel: ElementRef,
  refused: Signal<bool>,
  /// Whether *Cancel* was given the focus since the dialog opened.
  focused: Signal<bool>,
}

impl Confirmation {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      cancel: ctx.element_ref(),
      refused: ctx.signal(false),
      focused: ctx.signal(false),
    }
  }

  /// Opens the dialog; nothing is saved until its danger button is pressed.
  pub(super) fn ask(&self) {
    self.refused.set(false);
    self.focused.set(false);
    self.open.set(true);
  }

  /// The dialog over the window while it is open, for the project named `project`; the danger button saves through
  /// `save` in the project at `root` as whoever pressed it, if that is a person.
  pub(super) fn render(
    &self,
    ctx: &mut Ctx,
    project: &str,
    (save, root): (&Save, &Path),
    gate: Option<&InputGate>,
  ) -> Option<Element> {
    if !self.open.get() {
      return None;
    }
    if !self.focused.get_untracked() {
      self.focused.set(true);
      ctx.focus(&self.cancel);
    }
    let confirm = {
      let (this, save, root, gate) = (self.clone(), save.clone(), root.to_path_buf(), gate.cloned());
      move || this.confirm(gate.as_ref(), &save, &root)
    };
    let refusal = self.refused.get().then(|| {
      person_only(
        "settings-dangerous-person-only",
        &message_text(ctx, &person_only_message(ApproveMode::DangerousAutoApprove)),
      )
      .into()
    });
    let dialog = Dialog::new(
      "settings-dangerous-dialog",
      Icon::TriangleAlert,
      ctx.t("project_settings.dangerousDialog.title"),
      ctx.t("project_settings.dangerousDialog.cancel"),
    )
    .tone(Tone::Danger)
    .width(DIALOG_WIDTH)
    .part(body(ctx, project))
    .cancel_ref(self.cancel.clone())
    .action(
      ActionButton::new(
        "settings-dangerous-dialog-confirm",
        ButtonKind::Danger,
        ctx.t("project_settings.dangerousDialog.confirm"),
      )
      .icon(Icon::TriangleAlert)
      .on_click(confirm),
    )
    .under(refusal)
    .render(ctx, &self.open);
    Some(dialog)
  }

  /// The danger button's press: a person's turns the mode on and closes the dialog; any other input's is refused and
  /// said, and the dialog stays.
  fn confirm(&self, gate: Option<&InputGate>, save: &Save, root: &Path) {
    let by = answerer(gate, Guarded::ApprovalWidening).unwrap_or(Answerer::Automation);
    if by != Answerer::Person {
      tracing::warn!("synthetic input pressed Turn on Dangerous auto-approve mode; not taken");
      self.refused.set(true);
      return;
    }
    if save.state().get_untracked().is_pending() {
      return;
    }
    save.run((
      root.to_path_buf(),
      Change::Mode {
        to: ApproveMode::DangerousAutoApprove,
        by,
      },
    ));
    self.open.set(false);
  }
}

/// What will happen, what an agent can then do, and that it stays on (`In use` `q3vme`), 10 apart.
fn body(ctx: &Ctx, project: &str) -> Column {
  let words = |id: &str, text: Arc<str>| {
    Text::new(&text)
      .id(id.to_owned())
      .variant(DialogText::Line.role())
      .width(Dimension::Pct(100.0))
  };
  let risks = RISKS.iter().enumerate().map(|(index, (glyph, key))| {
    let row = Row::new()
      .id(format!("settings-dangerous-dialog-risk-{index}"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        SpacingSize::Md,
        ProjectSettingsSpacing::RiskY.role(),
      ))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .child(icon(*glyph, IconSize::Base, PaletteColor::Danger))
      .child(
        Text::new(&ctx.t(key))
          .variant(ExtraTypography::OptionTitle.role())
          .color(PaletteColor::TextPrimary)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      );
    if index == 0 {
      row
    } else {
      row.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    }
  });
  let list = Column::new()
    .id("settings-dangerous-dialog-risks")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Xs).bottom(SpacingSize::Xs))
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .with_children(risks);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(words(
      "settings-dangerous-dialog-text",
      ctx.t_args("project_settings.dangerousDialog.text", [("project", project)]),
    ))
    .child(list)
    .child(words(
      "settings-dangerous-dialog-after",
      ctx.t("project_settings.dangerousDialog.after"),
    ))
}

/// Under the group while Dangerous auto-approve is chosen: an `info` glyph and that nothing under the Approve mode is
/// used, 11.5 `text_muted`, 8 under the box and 2 in at its sides (`Note` `fYUkp`).
pub(super) fn unused_note(ctx: &Ctx) -> Element {
  Row::new()
    .id("settings-auto-dangerous-note")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(ExtraSpacing::Tight.role())
        .right(ExtraSpacing::Tight.role()),
    )
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("project_settings.dangerousFoot"))
        .id("settings-auto-dangerous-note-text")
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
