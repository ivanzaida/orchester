//! *With auto mode on, approve everything in each run* (ORC-199), drawn as *New runs use auto mode* is (`q0c4Ib` in
//! `WSQqT`): its label, a short hint, the long words in its (i), and a switch. Turning it on lets every auto mode run
//! answer its own requests, so an MCP client's input turns it on only while a person lets automation answer approvals
//! and change agent permissions, as `set_auto_mode` takes it; turning it off stays open. A refused press says so under
//! the row.

use std::path::Path;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::run::Answerer;
use orchester_workspace::project_settings::{Settings, approve_everything_person_only};

use super::{AutoMode, Props};
use crate::{
  mcp::input_gate::{Guarded, InputGate, answerer},
  screens::project_settings::{
    kit::SettingRow,
    work::{Change, Save},
  },
  ui::{
    message::message_text,
    tinted_note::person_only,
    toggle::{Toggle, ToggleKind},
  },
};

pub(super) fn row(
  ctx: &mut Ctx,
  (tab, props): (&AutoMode, &Props),
  settings: &Settings,
  (save, busy): (&Save, bool),
) -> Element {
  let on = settings.approve_everything.is_on();
  let toggle = Toggle::new(
    "settings-auto-everything",
    ToggleKind::Switch,
    on,
    ctx.t(if on {
      "project_settings.on"
    } else {
      "project_settings.off"
    }),
  )
  .hide_label();
  let toggle = if busy {
    toggle
  } else {
    let (gate, refused, save, root) = (
      tab.gate.clone(),
      tab.everything_refused.clone(),
      save.clone(),
      props.tab.project.open.repo_root.clone(),
    );
    toggle.on_change(move |chosen| press(chosen, (gate.as_ref(), &refused), &save, &root))
  };
  let refusal = tab.everything_refused.get().then(|| {
    person_only(
      "settings-auto-everything-person-only",
      &message_text(ctx, &approve_everything_person_only()),
    )
  });
  SettingRow::new(
    "settings-auto-approve-everything",
    ctx.t("project_settings.approveEverything"),
  )
  .info(ctx.t("project_settings.approveEverythingDetail"))
  .hint(ctx.t("project_settings.approveEverythingHint"))
  .control(toggle)
  .under_all(refusal.map(Element::from))
  .render(ctx)
}

/// Saves the switch turned `on` or off as whoever gives the input: turning it off is anyone's, turning it on a
/// person's or, under the input gate, automation's.
fn press(on: bool, (gate, refused): (Option<&InputGate>, &Signal<bool>), save: &Save, root: &Path) {
  let given = answerer(gate, Guarded::ApprovalWidening);
  let by = if on {
    given
  } else {
    Some(given.unwrap_or(Answerer::Automation))
  };
  refused.set(by.is_none());
  let Some(by) = by else {
    tracing::warn!("synthetic input turned on approving everything in each run; not taken");
    return;
  };
  if !save.state().get_untracked().is_pending() {
    save.run((root.to_path_buf(), Change::Everything { on, by }));
  }
}
