//! What an agent wrote in a task's conversation is drawn as the Orchestrator draws its replies (`ui::chat::agent_text`,
//! `dcd0m`, `w262B`, `agv8r`): Markdown in 13 `text_primary`, a list of labelled links as their grid with the detail in
//! 12.5 `text_secondary`, any other list as a list, and a link to a place in the project with a tooltip that names it.

use std::time::{Duration, Instant};

use lurq::{
  layout::quad::{QuadContent, RichTextSpan},
  node::color::Color,
};
use orchester_app::ui::tooltip::TOOLTIP_DELAY;
use orchester_core::{
  connection::stream::AgentEvent,
  project::TaskKey,
  store::{ActivityStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{Seeded, Window, key_of, open, seeded, walk, with_text};

/// `text_primary` and `text_secondary` in the dark mode the window shows.
const PRIMARY: &str = "#EDEDF0";
const SECONDARY: &str = "#B3B3BA";

/// Records `text` as a message of the last agent of `key`'s latest run.
pub fn agent_says(seeded: &Seeded, key: &TaskKey, text: &str) {
  agent_event(seeded, key, AgentEvent::Message { text: text.to_owned() });
}

/// Records `event` in the activity of the last agent of `key`'s latest run.
pub fn agent_event(seeded: &Seeded, key: &TaskKey, event: AgentEvent) {
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's database");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key() == key)
    .expect("the task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .next_back()
    .expect("a run");
  let invocation = db
    .invocations(run.id())
    .expect("invocations")
    .pop()
    .expect("an invocation");
  db.append_activity(&invocation, &[(Timestamp::now(), event)])
    .expect("the event");
}

/// Every run of text the window draws, the invisible marks lurq draws a link's words between left out.
fn spans(window: &mut Window) -> Vec<RichTextSpan> {
  window.pass();
  let tree = &window.tree;
  let invisible = |ch: char| matches!(ch, '\u{200E}' | '\u{2060}' | '\u{2066}'..='\u{2069}');
  tree
    .resolve_quads(tree.last_layout().expect("a layout"))
    .into_iter()
    .flat_map(|quad| match quad.content {
      QuadContent::RichText { spans, .. } => spans,
      QuadContent::Text { text, style, .. } => vec![RichTextSpan { text, style }],
      _ => Vec::new(),
    })
    .map(|span| RichTextSpan {
      text: span.text.chars().filter(|ch| !invisible(*ch)).collect(),
      style: span.style,
    })
    .collect()
}

fn span<'a>(spans: &'a [RichTextSpan], start: &str) -> &'a RichTextSpan {
  spans
    .iter()
    .find(|span| span.text.trim().starts_with(start))
    .unwrap_or_else(|| panic!("no span starts {start:?}"))
}

fn has_text(window: &mut Window, text: &str) -> bool {
  window
    .tree
    .find_element(|element| element.text_content() == Some(text))
    .is_some()
}

/// Whether the tooltip of the link `label` says `words` once the pointer rests on it.
fn tooltip_shows(window: &mut Window, label: &str, words: &str) -> bool {
  let bounds = window
    .tree
    .find_element(|element| element.text_content() == Some(label))
    .unwrap_or_else(|| panic!("{label} is a link's words"))
    .bounds();
  window
    .tree
    .mouse_move(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.pass();
  window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  window.pass();
  let shown = has_text(window, words);
  window.tree.mouse_move(0.0, 0.0);
  window.pass();
  shown
}

#[test]
fn conversation_agent_text_draws_labelled_links_as_a_grid_and_names_places_as_the_orchestrator_does() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  let project = seeded.project;
  agent_says(
    &seeded,
    &key,
    &format!(
      "Where it stands:\n\n- Task: [{key} notes](orchester:/project/{project}/tasks/{key}/conversation) — ready for \
       review\n- Pull request: [#42 Delivery log](https://github.com/acme/kontur/pull/42)\n\nAnd:\n\n- one\n- two"
    ),
  );
  open(&mut window, &seeded, &key, "conversation-chat");
  let notes = format!("{key} notes");
  window.until("the message", |window| has_text(window, &notes));

  // The scenario's own message with links has a grid too; count this message's.
  let said = with_text(&mut window, "conversation-text-", &notes);
  let mut grids = 0;
  walk(
    window.tree.get_element_by_id(&said).expect("the message"),
    &mut |element| grids += usize::from(element.has_class("chat-link-grid")),
  );
  assert_eq!(grids, 1, "only the list of labelled links is a grid");
  for label in ["Task", "Pull request"] {
    assert!(has_text(&mut window, label), "{label} is a label of its own");
  }
  let drawn = spans(&mut window);
  let detail = span(&drawn, "ready for review");
  assert_eq!(
    (detail.style.font_size, detail.style.color),
    (12.5, Color::from_hex(SECONDARY)),
    "a detail in 12.5 `text_secondary`"
  );
  let body = span(&drawn, "Where it stands:");
  assert_eq!(
    (body.style.font_size, body.style.color),
    (13.0, Color::from_hex(PRIMARY)),
    "the message in 13 `text_primary`, as an Orchestrator's reply"
  );
  assert!(
    tooltip_shows(
      &mut window,
      &notes,
      &format!("{key} › Conversation · opens in this tab")
    ),
    "a link to a place in the project names it"
  );
}
