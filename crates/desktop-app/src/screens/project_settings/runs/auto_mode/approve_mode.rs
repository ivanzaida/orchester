//! *Approve mode* (`docs/design/approval-modes.md`, the person's decision of 2026-11-11; `Approve mode` `aHLLh`,
//! `oWUo9`, `O2Ml3` in `v5d5r` of `Orchestrator WIP`), the group's first row: its label and (i), and on the label's
//! line a segmented control of Manual, Autopilot and Dangerous auto-approve; under them what the chosen mode means,
//! across the row. In Dangerous auto-approve the row is on `danger_muted` and its hint in `text_secondary`.
//!
//! Moving the mode up widens what agents do without a person, so only a person's input does it, whatever the input
//! gate lets automation do: a refused press says so under the row. Choosing Dangerous auto-approve asks first, in its
//! confirmation ([`super::dangerous`]). Moving it down is anyone's, at once.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  core::Signal,
  node::Element,
};
use orchester_core::run::{Answerer, ApproveMode};
use orchester_workspace::project_settings::{Settings, person_only};

use super::{AutoMode, Props, dangerous::Confirmation};
use crate::{
  mcp::input_gate::{Guarded, InputGate, answerer},
  screens::project_settings::{
    kit::{SettingRow, Tone},
    work::{Change, Save},
  },
  ui::{
    message::message_text,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
    tinted_note::person_only as person_only_note,
  },
};

/// The modes in the control's order, with their segment's id and words.
const MODES: [(ApproveMode, &str, &str); 3] = [
  (
    ApproveMode::Manual,
    "settings-approve-mode-manual",
    "project_settings.approveModeManual",
  ),
  (
    ApproveMode::Autopilot,
    "settings-approve-mode-autopilot",
    "project_settings.approveModeAutopilot",
  ),
  (
    ApproveMode::DangerousAutoApprove,
    "settings-approve-mode-dangerous",
    "project_settings.approveModeDangerous",
  ),
];

/// What a press of a segment needs: who gave it, the dialog that asks first, the save and the refusal it may say.
#[derive(Clone)]
struct Choosing {
  gate: Option<InputGate>,
  confirmation: Confirmation,
  refused: Signal<bool>,
  save: Save,
  root: Arc<Path>,
}

impl Choosing {
  /// `to` chosen from `current`: Dangerous auto-approve asks first; another move up is a person's alone; down is
  /// anyone's.
  fn choose(&self, current: ApproveMode, to: ApproveMode) {
    if to == current {
      return;
    }
    if to == ApproveMode::DangerousAutoApprove {
      self.refused.set(false);
      self.confirmation.ask();
      return;
    }
    let by = answerer(self.gate.as_ref(), Guarded::ApprovalWidening).unwrap_or(Answerer::Automation);
    let taken = current.changed(to, by).is_ok();
    self.refused.set(!taken);
    if !taken {
      tracing::warn!("synthetic input moved the Approve mode up; not taken");
      return;
    }
    if !self.save.state().get_untracked().is_pending() {
      self.save.run((self.root.to_path_buf(), Change::Mode { to, by }));
    }
  }
}

pub(super) fn row(
  ctx: &mut Ctx,
  (tab, props): (&AutoMode, &Props),
  settings: &Settings,
  (save, busy): (&Save, bool),
) -> Element {
  let current = settings.approve_mode;
  let choosing = Choosing {
    gate: tab.gate.clone(),
    confirmation: tab.dangerous.clone(),
    refused: tab.autopilot_refused.clone(),
    save: save.clone(),
    root: Arc::from(props.tab.project.open.repo_root.as_path()),
  };
  let segments = MODES.map(|(mode, id, key)| {
    let label = ctx.t(key);
    let face = if mode == ApproveMode::DangerousAutoApprove {
      SegmentFace::Warned(label)
    } else {
      SegmentFace::Text(label)
    };
    let choosing = choosing.clone();
    Segment::new(id, face, mode == current, move || {
      if !busy {
        choosing.choose(current, mode);
      }
    })
  });
  let control = segmented("settings-approve-mode", segments, Focus::Keyboard);
  let refusal = tab.autopilot_refused.get().then(|| {
    person_only_note(
      "settings-approve-mode-person-only",
      &message_text(ctx, &person_only(ApproveMode::Autopilot)),
    )
  });
  let row = SettingRow::new("settings-auto-approve-mode", ctx.t("project_settings.approveMode"))
    .info(ctx.t("project_settings.approveModeDetail"))
    .on_label()
    .control(control)
    .under_all(refusal.map(Element::from));
  match current {
    ApproveMode::Manual => row.hint(manual_hint(ctx, &settings.autopilot_profiles)),
    ApproveMode::Autopilot => row.hint(ctx.t("project_settings.approveModeAutopilotHint")),
    ApproveMode::DangerousAutoApprove => row
      .fill(PaletteColor::DangerMuted)
      .toned(ctx.t("project_settings.approveModeDangerousHint"), Tone::Secondary),
  }
  .render(ctx)
}

/// What Manual means; with profiles set to Autopilot, which then run as Manual too, it names them (`tvmsJ`).
fn manual_hint(ctx: &Ctx, profiles: &[String]) -> Arc<str> {
  match profiles {
    [] => ctx.t("project_settings.approveModeManualHint"),
    [one] => ctx.t_args(
      "project_settings.approveModeManualHintProfiles.one",
      [("profiles", one.as_str())],
    ),
    [rest @ .., last] => {
      let rest = rest.join(&ctx.t("project_settings.listSeparator"));
      let listed = ctx.t_args(
        "project_settings.listAnd",
        [("rest", rest.as_str()), ("last", last.as_str())],
      );
      ctx.t_args(
        "project_settings.approveModeManualHintProfiles.other",
        [("profiles", &*listed)],
      )
    }
  }
}
