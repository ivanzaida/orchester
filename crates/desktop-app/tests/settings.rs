//! Settings › Automation's *What automation may do*: *Look at the window*, always on; *Agents can control this
//! window*, the build's default until a person chooses, applying from the next start; and *Let automation answer
//! approvals*, *Let automation change agent permissions* and the Orchestrator's switches (*publish drafts*, *answer
//! agents*, *merge runs*): each off until a person turns it on in the window, where it reads the stored value and
//! saves what the person chooses; synthetic input never flips one, and MCP clients' window input is refused meanwhile.

mod support;

use lurq::app::events::{MouseButton, ScrollPhase};
use orchester_app::routes::Route;
use orchester_workspace::workspace::OrchestratorConsent;
use support::Window;

const SWITCH: &str = "settings-change-permissions";
const PUBLISH: &str = "settings-publish-drafts";

/// Whether the switch shows on: its knob at the end of its track.
fn on(window: &mut Window) -> bool {
  switch_on(window, SWITCH)
}

/// Whether the switch `switch` shows on.
fn switch_on(window: &mut Window, switch: &str) -> bool {
  let mut bounds = |id: String| {
    window
      .tree
      .get_element_by_id_mut(&id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (track, knob) = (bounds(format!("{switch}-mark")), bounds(format!("{switch}-knob")));
  knob.x - track.x > track.width / 2.0 - knob.width / 2.0
}

/// Presses the switch until the stored setting is `wanted`; a press while it is read or saved is not
/// taken, and the setting is read before each press, so it never flips back.
fn turn(window: &mut Window, wanted: bool) {
  let workspace = window.launch.workspace().expect("the session started").clone();
  window.until("the choice to be saved", |window| {
    let stored = workspace.automation_changes_permissions().expect("the setting reads");
    if stored != wanted {
      click(window, SWITCH);
    }
    stored == wanted
  });
  window.until("the switch to show it", |window| on(window) == wanted);
}

fn click(window: &mut Window, id: &str) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
}

/// A window tall enough to show *Let automation change agent permissions* under the MCP server's section and the
/// rows above it, so a pointer reaches its switch where it is drawn.
fn tall() -> Window {
  let mut window = Window::open();
  window.tree.resize(1440, 2400);
  window
}

#[test]
fn settings_a_person_turns_on_automation_changing_agent_permissions() {
  let mut window = tall();
  window.navigation().go(Route::Settings);
  window.until("the setting's row", |window| {
    window.texts("settings-automation").is_some()
  });
  let texts = window.texts("settings-automation").expect("the section shows");
  assert!(
    texts
      .iter()
      .any(|text| text == "Let automation change agent permissions"),
    "{texts:?}"
  );
  assert!(!on(&mut window), "off until turned on");
  turn(&mut window, true);
  turn(&mut window, false);
}

#[test]
fn settings_human_revocation_survives_leaving_before_save_dispatch() {
  let mut window = tall();
  let workspace = window.launch.workspace().expect("session").clone();
  workspace
    .set_automation_changes_permissions(true)
    .expect("private human opt-in");
  window.navigation().go(Route::Settings);
  window.until("enabled switch", on);
  let bounds = window
    .tree
    .get_element_by_id_mut(SWITCH)
    .and_then(|element| element.bounds())
    .expect("switch bounds");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  // Do not tick the future: leave immediately after the actual human pointer event.
  assert!(
    !workspace.automation_permission_consent(),
    "human event revokes synchronously"
  );
  window.navigation().go(Route::AllProjects);
  window.pass();
  assert!(!window.link.input_gate().protecting(), "Settings has unmounted");
  assert!(
    !workspace
      .automation_changes_permissions()
      .expect("stored read after leaving")
  );
  assert!(
    !workspace.automation_permission_consent(),
    "leaving cannot restore the old grant"
  );
}

#[test]
fn settings_synthetic_pointer_cannot_change_its_own_opt_in() {
  use lurq::app::events::with_synthetic_input;
  let mut window = tall();
  let workspace = window.launch.workspace().expect("session").clone();
  window.navigation().go(Route::Settings);
  window.until("loaded Settings switch", |window| {
    window.texts("settings-automation").is_some()
  });
  for chosen in [false, true] {
    workspace
      .set_automation_changes_permissions(chosen)
      .expect("private human choice");
    // Remount to show the stored human choice, with no MCP caller in flight.
    window.navigation().go(Route::AllProjects);
    window.pass();
    window.navigation().go(Route::Settings);
    window.until("stored switch", |window| on(window) == chosen);
    window.until("synthetic input rejected", |window| {
      with_synthetic_input(|| click(window, SWITCH));
      window.texts("settings-change-permissions-blocked").is_some()
    });
    assert_eq!(
      workspace.automation_changes_permissions().expect("actual setting"),
      chosen
    );
    assert!(
      window.texts("settings-change-permissions-blocked").is_some(),
      "synthetic input is visibly rejected"
    );
  }
}

/// Clicks the element `id` where it is, laid out below the window's fold or not.
fn press_element(window: &mut Window, id: &str) {
  window
    .tree
    .get_element_by_id_mut(id)
    .unwrap_or_else(|| panic!("{id} exists"))
    .click();
  window.pass();
}

/// The switch of *Let the Orchestrator continue interrupted runs*, the section's last.
const CONTINUE: &str = "settings-continue-interrupted";

/// The Orchestrator's switches: each one's element id, its title, and the consent it saves.
const ORCHESTRATOR_SWITCHES: [(&str, &str, OrchestratorConsent); 6] = [
  (
    PUBLISH,
    "Let the Orchestrator publish drafts",
    OrchestratorConsent::PublishDrafts,
  ),
  (
    "settings-answer-agents",
    "Let the Orchestrator answer agents",
    OrchestratorConsent::AnswerAgents,
  ),
  (
    "settings-merge-runs",
    "Let the Orchestrator merge runs",
    OrchestratorConsent::MergeRuns,
  ),
  (
    "settings-merge-finished",
    "Let the Orchestrator merge finished runs on its own",
    OrchestratorConsent::MergeFinished,
  ),
  (
    CONTINUE,
    "Let the Orchestrator continue interrupted runs",
    OrchestratorConsent::ContinueInterrupted,
  ),
  (
    "settings-write-git",
    "Let the Orchestrator write with git",
    OrchestratorConsent::WriteGit,
  ),
];

#[test]
fn settings_only_a_person_lets_the_orchestrator_publish_answer_merge_or_continue() {
  use lurq::app::events::with_synthetic_input;
  let mut window = Window::open();
  let workspace = window.launch.workspace().expect("session").clone();
  window.navigation().go(Route::Settings);
  for (switch, title, consent) in ORCHESTRATOR_SWITCHES {
    window.until("the setting's row", |window| {
      window.texts(&format!("{switch}-row")).is_some()
    });
    let texts = window.texts(&format!("{switch}-row")).expect("the row shows");
    assert!(texts.iter().any(|text| text == title), "{texts:?}");
    assert!(!switch_on(&mut window, switch), "{title} is off until turned on");
    window.until("synthetic input rejected", |window| {
      with_synthetic_input(|| press_element(window, switch));
      window.texts(&format!("{switch}-blocked")).is_some()
    });
    assert!(
      !workspace.orchestrator_allows(consent).expect("the setting reads"),
      "synthetic input does not turn {title} on"
    );
    window.until("the person's choice to be saved", |window| {
      let stored = workspace.orchestrator_allows(consent).expect("the setting reads");
      if !stored {
        press_element(window, switch);
      }
      stored
    });
    window.until("the switch to show it", |window| switch_on(window, switch));
  }
  assert!(
    !workspace
      .automation_changes_permissions()
      .expect("the other setting reads"),
    "the Orchestrator's switches turn nothing else on"
  );
}

#[test]
fn settings_turning_off_an_orchestrator_switch_holds_from_the_click() {
  let mut window = Window::open();
  let workspace = window.launch.workspace().expect("session").clone();
  for (_, _, consent) in ORCHESTRATOR_SWITCHES {
    workspace
      .set_orchestrator_allows(consent, true)
      .expect("the person turned it on");
  }
  window.navigation().go(Route::Settings);
  for (switch, title, consent) in ORCHESTRATOR_SWITCHES {
    window.until("the switch on", |window| {
      window.texts(&format!("{switch}-row")).is_some() && switch_on(window, switch)
    });
    // The person's click; its save has not run yet.
    window.tree.get_element_by_id_mut(switch).expect("the switch").click();
    assert!(
      !workspace.orchestrator_allows(consent).expect("read"),
      "{title} holds off from the click, before the save"
    );
  }
}

/// The bottom of `id` as laid out.
fn bottom(window: &mut Window, id: &str) -> f32 {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  bounds.y + bounds.height
}

#[test]
fn settings_automation_scrolls_to_its_last_switch_in_a_short_window() {
  let mut window = Window::open();
  window.tree.resize(1440, 600);
  window.navigation().go(Route::Settings);
  window.until("the last switch", |window| {
    window.tree.get_element_by_id(CONTINUE).is_some()
  });
  assert!(
    bottom(&mut window, CONTINUE) > 600.0,
    "below the window's height before scrolling"
  );
  let bounds = window
    .tree
    .get_element_by_id_mut("settings-automation-scroll")
    .and_then(|element| element.bounds())
    .expect("the tab scrolls");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.scroll(x, y, 0.0, -2000.0, ScrollPhase::Scroll);
  window.pass();
  window.pass();
  assert!(bottom(&mut window, CONTINUE) <= 600.0, "scrolled into the window");
}

/// The switch of the Orchestrator's daily report, the section's last row.
const DAILY_REPORT: &str = "settings-daily-report";

#[test]
fn settings_daily_report_is_off_until_turned_on_and_comes_at_six_by_default() {
  let mut window = Window::open();
  let workspace = window.launch.workspace().expect("session").clone();
  window.navigation().go(Route::Settings);
  window.until("the report's row", |window| {
    window.texts(&format!("{DAILY_REPORT}-row")).is_some()
  });
  assert!(!switch_on(&mut window, DAILY_REPORT), "off until turned on");
  assert!(
    window.tree.get_element_by_id(&format!("{DAILY_REPORT}-time")).is_none(),
    "no time while it is off"
  );
  window.until("the person's choice to be saved", |window| {
    let stored = workspace.daily_report_schedule().expect("the setting reads");
    if stored.is_none() {
      press_element(window, DAILY_REPORT);
    }
    stored.is_some()
  });
  let schedule = workspace.daily_report_schedule().expect("read").expect("on");
  assert_eq!(schedule.time.minute(), 18 * 60, "at 18:00 unless the person chooses");
  window.until("its time to show", |window| {
    window.tree.get_element_by_id(&format!("{DAILY_REPORT}-time")).is_some()
  });
  let texts = window.texts(&format!("{DAILY_REPORT}-at")).expect("the time's row");
  assert!(texts.iter().any(|text| text == "Deliver at"), "{texts:?}");
}

const LOOK: &str = "settings-look-at-window";
const WINDOW_CONTROL: &str = "settings-window-control";
const APPROVALS: &str = "settings-answer-approvals";

#[test]
fn settings_automation_opens_with_what_automation_may_always_do_and_locks_window_input() {
  let mut window = Window::open();
  window.navigation().go(Route::Settings);
  window.until("the section's rows", |window| {
    window.texts(&format!("{APPROVALS}-row")).is_some()
  });
  let texts = window.texts(&format!("{LOOK}-row")).expect("the row shows");
  for words in ["Look at the window", "Always on"] {
    assert!(texts.iter().any(|text| text == words), "{words} in {texts:?}");
  }
  assert!(switch_on(&mut window, LOOK), "looking is always on");
  // The section's rows come in the design's order.
  let top = |window: &mut Window, id: &str| {
    window
      .tree
      .get_element_by_id_mut(&format!("{id}-row"))
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
      .y
  };
  let order = [LOOK, WINDOW_CONTROL, APPROVALS, SWITCH, PUBLISH];
  let tops: Vec<f32> = order.iter().map(|id| top(&mut window, id)).collect();
  assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{order:?} at {tops:?}");
  assert!(
    window.link.input_gate().protecting(),
    "while the person's switches show, MCP clients' window input is refused"
  );
}

#[test]
fn settings_window_control_shows_the_next_start_and_only_a_person_changes_it() {
  use lurq::app::events::with_synthetic_input;
  let mut window = Window::open();
  let workspace = window.launch.workspace().expect("session").clone();
  window.navigation().go(Route::Settings);
  window.until("the switch", |window| {
    window.texts(&format!("{WINDOW_CONTROL}-hint")).is_some()
  });
  let default = cfg!(debug_assertions);
  assert_eq!(
    switch_on(&mut window, WINDOW_CONTROL),
    default,
    "the build's default until set"
  );
  let build = if default {
    "Build default: on in development builds like this one"
  } else {
    "Build default: off in release builds like this one"
  };
  assert!(window.text(&format!("{WINDOW_CONTROL}-hint")).starts_with(build));
  window.until("synthetic input rejected", |window| {
    with_synthetic_input(|| press_element(window, WINDOW_CONTROL));
    window.texts(&format!("{WINDOW_CONTROL}-blocked")).is_some()
  });
  assert_eq!(workspace.mcp_window_control().expect("read"), None, "nothing stored");
  window.until("the person's choice to be saved", |window| {
    let stored = workspace.mcp_window_control().expect("read");
    if stored.is_none() {
      press_element(window, WINDOW_CONTROL);
    }
    stored.is_some()
  });
  assert_eq!(workspace.mcp_window_control().expect("read"), Some(!default));
  let next = if default {
    "Off from the next start."
  } else {
    "On from the next start."
  };
  window.until("the hint to say when it applies", |window| {
    window.text(&format!("{WINDOW_CONTROL}-hint")).starts_with(next)
  });
}

#[test]
fn settings_only_a_person_lets_automation_answer_approvals_and_off_holds_from_the_click() {
  use lurq::app::events::with_synthetic_input;
  let mut window = Window::open();
  let workspace = window.launch.workspace().expect("session").clone();
  window.navigation().go(Route::Settings);
  window.until("the switch", |window| {
    window.texts(&format!("{APPROVALS}-row")).is_some()
  });
  assert!(!switch_on(&mut window, APPROVALS), "off until turned on");
  window.until("synthetic input rejected", |window| {
    with_synthetic_input(|| press_element(window, APPROVALS));
    window.texts(&format!("{APPROVALS}-blocked")).is_some()
  });
  assert!(!workspace.automation_answers_approvals().expect("read"));
  window.until("the person's choice to be saved", |window| {
    let stored = workspace.automation_answers_approvals().expect("read");
    if !stored {
      press_element(window, APPROVALS);
    }
    stored
  });
  window.until("the switch and its note", |window| {
    switch_on(window, APPROVALS) && window.texts(&format!("{APPROVALS}-on")).is_some()
  });
  assert!(workspace.automation_approval_consent(), "automation may answer now");
  // Shown again from what is stored, so the person's next click is taken.
  window.navigation().go(Route::AllProjects);
  window.pass();
  window.navigation().go(Route::Settings);
  window.until("the stored switch", |window| {
    window.texts(&format!("{APPROVALS}-row")).is_some() && switch_on(window, APPROVALS)
  });
  window
    .tree
    .get_element_by_id_mut(APPROVALS)
    .expect("the switch")
    .click();
  assert!(
    !workspace.automation_approval_consent(),
    "off holds from the person's click, before the save"
  );
  window.until("the choice to be saved", |_| {
    !workspace.automation_answers_approvals().expect("read")
  });
  assert!(
    !workspace.automation_changes_permissions().expect("read"),
    "answering approvals turns nothing else on"
  );
}
