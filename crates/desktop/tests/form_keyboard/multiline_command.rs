//! Command previews and details through the real Activity tab, using stored fixture events.

use lurq::{
  clipboard::{copy_to_clipboard, read_from_clipboard},
  node::element::ElementRef,
};
use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, ToolCommandResult},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{create_and_run, finished_on_page, has, init_repository, latest_run, programs, seed},
};

fn exact_text(element: ElementRef<'_>, text: &str) -> bool {
  element.text_content() == Some(text) || element.children().into_iter().any(|child| exact_text(child, text))
}

fn append(desktop: &Desktop, events: Vec<AgentEvent>) -> u64 {
  let mut db = ProjectDb::open(desktop.repo()).expect("open fixture project");
  let run = latest_run(desktop.repo()).expect("fixture run");
  let invocation = db.invocations(run.id()).expect("invocations").remove(0);
  let count = events.len() as u64;
  let events: Vec<_> = events
    .into_iter()
    .enumerate()
    .map(|(i, event)| (Timestamp::from_unix_millis(10_000 + i as i64 * 1_000), event))
    .collect();
  db.append_activity(&invocation, &events).expect("store fixture events") - count + 1
}

fn assert_header_geometry(desktop: &mut Desktop, sequence: u64) {
  let header = desktop.bounds(&format!("activity-entry-header-{sequence}"));
  let title = desktop.bounds(&format!("activity-entry-title-{sequence}"));
  let meta = desktop.bounds(&format!("activity-entry-meta-{sequence}"));
  let time = desktop.bounds(&format!("activity-entry-time-{sequence}"));
  let icon = desktop.bounds(&format!("activity-entry-icon-{sequence}"));
  assert!(title.height <= header.height);
  assert!(title.x + title.width <= meta.x);
  assert!(meta.x + meta.width <= header.x + header.width);
  assert_eq!(time.width, 36.0);
  assert_eq!(icon.width, 24.0);
  assert_eq!(time.y, header.y);
  assert_eq!(icon.y, header.y);
}

fn check_entry((command, preview): (&str, &str), events: &[AgentEvent], meta: &str, failed: bool) {
  let has_output = events
    .iter()
    .any(|event| matches!(event, AgentEvent::CommandFinished { .. }));
  for width in [1440, 960] {
    let (runner, fake) = programs();
    let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
    create_and_run(&mut desktop, "Append one line.", "1400000");
    finished_on_page(&mut desktop);
    let sequence = append(&desktop, events.to_vec());
    check_at_width(
      &mut desktop,
      sequence,
      (command, preview),
      meta,
      (failed, has_output),
      width,
    );
  }
}

fn check_at_width(
  desktop: &mut Desktop,
  sequence: u64,
  (command, preview): (&str, &str),
  meta: &str,
  (failed, has_output): (bool, bool),
  width: u32,
) {
  desktop.tree.resize(width, 960);
  desktop.click("task-tab-activity");
  desktop.wait_for("activity-step-header");
  desktop.click("activity-step-header");
  let header = format!("activity-entry-header-{sequence}");
  let toggle = format!("activity-entry-toggle-{sequence}");
  let detail_id = format!("activity-entry-details-{sequence}");
  desktop.wait_for(&header);
  assert!(shows(desktop, &header, preview));
  assert!(shows(desktop, &header, meta));
  if failed {
    assert!(has(desktop, &detail_id), "failed commands start expanded");
    desktop.click(&toggle);
  }
  desktop.wait_until("the collapsed header layout", |desktop| {
    desktop
      .tree
      .get_element_by_id_mut(&header)
      .and_then(|element| element.bounds())
      .is_some()
  });
  assert_header_geometry(desktop, sequence);
  assert!(!has(desktop, &detail_id));
  desktop.click(&toggle);
  desktop.wait_for(&detail_id);
  let detail = desktop.tree.get_element_by_id(&detail_id).expect("command detail");
  assert!(exact_text(detail, command), "details retain the original command");
  if has_output {
    assert!(exact_text(detail, "unchanged output\nsecond line"));
    assert!(shows(desktop, &header, "1.0 s"));
  }
  assert!(
    has(desktop, &format!("activity-command-detail-{sequence}")),
    "ids are the entry's own"
  );
  assert!(has(desktop, &format!("activity-command-copy-{sequence}")));
  let detail_bounds = desktop.bounds(&detail_id);
  assert!(detail_bounds.height > 26.0, "details grow for multiline commands");
  desktop.click(&toggle);
  assert!(!has(desktop, &detail_id));
  desktop.click("task-tab-summary");
  finished_on_page(desktop);
}

#[test]
fn multiline_command_preview_and_details_keep_states_and_output() {
  for ending in ["\n", "\r\n"] {
    let command = format!(
      "python -c \"{ending}  print('quoted') {ending}{ending}  # {}{ending}\"",
      "long ".repeat(20)
    );
    let preview = format!("python -c \" print('quoted') # {} \"", "long ".repeat(20).trim_end());
    let shown = (command.as_str(), preview.as_str());
    for status in [None, Some(CommandStatus::Completed), Some(CommandStatus::Failed)] {
      let mut events = vec![AgentEvent::CommandStarted {
        item: "multiline".into(),
        command: command.clone(),
      }];
      if let Some(status) = status {
        events.push(AgentEvent::CommandFinished {
          item: "multiline".into(),
          command: command.clone(),
          output: "unchanged output\nsecond line".into(),
          exit_code: Some(if status == CommandStatus::Failed { 1 } else { 0 }),
          status,
        });
      }
      let meta = match status {
        None => "running",
        Some(CommandStatus::Failed) => "exit 1",
        _ => "exit 0",
      };
      check_entry(shown, &events, meta, status == Some(CommandStatus::Failed));
    }
    let events = vec![
      AgentEvent::ToolCommandStarted {
        tool: "fixture".into(),
        command: command.clone(),
      },
      AgentEvent::ToolCommandFinished {
        tool: "fixture".into(),
        command: command.clone(),
        result: ToolCommandResult::Passed,
        exit_code: Some(0),
        output: "".into(),
        omitted_bytes: 0,
      },
    ];
    check_entry(shown, &events, "exit 0", false);
  }
}

/// Two failed commands, both open, in the Activity tab at the supported minimum width; returns the
/// app and the first entry's sequence.
fn two_open_commands() -> (Desktop, u64) {
  let command = format!("pwsh -Command \"\n  # {}\n\"", "long ".repeat(60));
  let mut events = Vec::new();
  for item in ["first", "second"] {
    events.push(AgentEvent::CommandStarted {
      item: item.into(),
      command: command.clone(),
    });
    events.push(AgentEvent::CommandFinished {
      item: item.into(),
      command: command.clone(),
      output: "".into(),
      exit_code: Some(1),
      status: CommandStatus::Failed,
    });
  }
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Append one line.", "1400000");
  finished_on_page(&mut desktop);
  let first = append(&desktop, events);
  desktop.tree.resize(960, 960);
  desktop.click("task-tab-activity");
  desktop.wait_for("activity-step-header");
  desktop.click("activity-step-header");
  desktop.wait_for(&format!("activity-entry-details-{first}"));
  (desktop, first)
}

#[test]
fn multiline_command_text_pressed_in_activity_leaves_the_entry_open() {
  let (mut desktop, first) = two_open_commands();
  let second = first + 2;
  for sequence in [first, second] {
    assert!(has(&mut desktop, &format!("activity-command-detail-{sequence}")));
    assert!(has(&mut desktop, &format!("activity-command-copy-{sequence}")));
  }
  let details = format!("activity-entry-details-{first}");
  // The middle of the scroll area is on the command's text, which runs past it.
  desktop.press(&format!("activity-command-detail-{first}-scroll"));
  assert!(
    has(&mut desktop, &details),
    "pressing the command text keeps the entry open"
  );
  desktop.press(&format!("activity-command-detail-{first}"));
  assert!(
    has(&mut desktop, &details),
    "pressing the command box keeps the entry open"
  );
}

#[test]
#[ignore = "writes the command to the clipboard and restores its previous text; run separately"]
fn multiline_command_copy_pressed_in_activity_leaves_the_entry_open() {
  let previous = read_from_clipboard().expect("a text clipboard whose previous contents can be restored");
  let (mut desktop, first) = two_open_commands();
  let copy = format!("activity-command-copy-{first}");
  desktop.press(&copy);
  let copied = shows(&mut desktop, &copy, "Copied");
  let open = has(&mut desktop, &format!("activity-entry-details-{first}"));
  assert!(copy_to_clipboard(&previous), "restore the previous clipboard text");
  assert!(copied, "Copy took the command");
  assert!(open, "pressing Copy keeps the entry open");
}
