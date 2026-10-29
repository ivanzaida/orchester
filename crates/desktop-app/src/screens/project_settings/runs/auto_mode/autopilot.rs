//! *Allow Autopilot for agents in this project* (`docs/design/approval-modes.md`, `m0eUB` / `tvmsJ`), drawn as the
//! group's other switches are: its label, what On or Off means, the long words in its (i), and a switch. Allowing
//! Autopilot lets agents set to it approve their own requests, so only a person's input turns it on, whatever the
//! input gate lets automation do; turning it off stays open. A refused press says so under the row.

use std::{path::Path, sync::Arc};

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::run::Answerer;
use orchester_workspace::project_settings::{Settings, autopilot_person_only};

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
  let on = settings.allow_autopilot.is_on();
  let toggle = Toggle::new(
    "settings-allow-autopilot",
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
      tab.autopilot_refused.clone(),
      save.clone(),
      props.tab.project.open.repo_root.clone(),
    );
    toggle.on_change(move |chosen| press(chosen, (gate.as_ref(), &refused), &save, &root))
  };
  let refusal = tab.autopilot_refused.get().then(|| {
    person_only(
      "settings-allow-autopilot-person-only",
      &message_text(ctx, &autopilot_person_only()),
    )
  });
  SettingRow::new(
    "settings-auto-allow-autopilot",
    ctx.t("project_settings.allowAutopilot"),
  )
  .info(ctx.t("project_settings.allowAutopilotDetail"))
  .hint(hint(ctx, on, &settings.autopilot_profiles))
  .control(toggle)
  .under_all(refusal.map(Element::from))
  .render(ctx)
}

/// What the switch's state means; off, it names the profiles set to Autopilot (`tvmsJ`), which then run as Manual.
fn hint(ctx: &Ctx, on: bool, profiles: &[String]) -> Arc<str> {
  match profiles {
    _ if on => ctx.t("project_settings.allowAutopilotOn"),
    [] => ctx.t("project_settings.allowAutopilotOff"),
    [one] => ctx.t_args(
      "project_settings.allowAutopilotOffProfiles.one",
      [("profiles", one.as_str())],
    ),
    [rest @ .., last] => {
      let rest = rest.join(&ctx.t("project_settings.listSeparator"));
      let listed = ctx.t_args(
        "project_settings.listAnd",
        [("rest", rest.as_str()), ("last", last.as_str())],
      );
      ctx.t_args(
        "project_settings.allowAutopilotOffProfiles.other",
        [("profiles", &*listed)],
      )
    }
  }
}

/// Saves the switch turned `on` or off as whoever gives the input: turning it off is anyone's, turning it on only a
/// person's.
fn press(on: bool, (gate, refused): (Option<&InputGate>, &Signal<bool>), save: &Save, root: &Path) {
  let by = answerer(gate, Guarded::ApprovalWidening).unwrap_or(Answerer::Automation);
  let taken = !on || by == Answerer::Person;
  refused.set(!taken);
  if !taken {
    tracing::warn!("synthetic input turned on Allow Autopilot; not taken");
    return;
  }
  if !save.state().get_untracked().is_pending() {
    save.run((root.to_path_buf(), Change::Autopilot { on, by }));
  }
}
