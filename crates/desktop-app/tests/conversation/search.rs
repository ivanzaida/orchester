//! Searching the transcript (ORC-306, `Task page · redesign` `C3wC8` 13–13f) on KON-12's long transcript: Ctrl F (⌘ F
//! on macOS) opens the find bar beside the search button, the places are counted, the current one marked and brought
//! into view, next and previous go round them, a place in a closed tool group opens the group and the call with its
//! output around the place, nothing found says what was searched, Esc closes the bar and folds again what it opened,
//! and the query stays. A transcript 2,000 entries longer is searched off the UI thread: no frame waits for it.

use std::time::{Duration, Instant};

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{ids, key_of, long::at_work, open, retry_webhooks, seeded, support::Window};

/// The find field and the count in it.
const FIELD: &str = "conversation-search-field";
const COUNT: &str = "conversation-search-field-unit";

/// Ctrl F, ⌘ F on macOS.
fn find_keys(window: &mut Window) {
  let mac = cfg!(target_os = "macos");
  window
    .tree
    .key_down_with_meta("f".to_owned(), "KeyF".to_owned(), false, !mac, false, mac);
  window.pass();
}

/// Opens the search and looks for `query`, until the count says `count`.
fn search(window: &mut Window, query: &str, count: &str) {
  open_bar(window, query);
  window.until(&format!("{count} for {query}"), |window| {
    window.texts(COUNT).is_some_and(|texts| texts.concat() == count)
  });
}

/// Opens the search and types `query`.
fn open_bar(window: &mut Window, query: &str) {
  find_keys(window);
  window.until("the find bar", |window| {
    window.tree.get_element_by_id("conversation-search-bar").is_some()
  });
  window.set(FIELD, query);
}

/// How many places the count says the search found, once it found some.
fn found(window: &mut Window) -> usize {
  let mut count = String::new();
  window.until("the places counted", |window| {
    count = window.texts(COUNT).map(|texts| texts.concat()).unwrap_or_default();
    count.contains(" of ")
  });
  count
    .rsplit(" of ")
    .next()
    .and_then(|total| total.parse().ok())
    .unwrap_or_else(|| panic!("a count: {count}"))
}

fn exists(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

#[test]
fn search_finds_a_message_and_marks_its_row() {
  let (mut window, _) = retry_webhooks();
  assert!(exists(&mut window, "task-search"), "the search button ends the tabs");
  search(&mut window, "CONFIGURABLE", "1 of 1");
  window.until("the person's message marked as the current place", |window| {
    window
      .texts("conversation-search-current-row")
      .is_some_and(|texts| texts.concat().contains("retry budget configurable"))
  });
}

#[test]
fn search_opens_the_group_and_the_call_to_show_a_place_in_its_output() {
  let (mut window, _) = retry_webhooks();
  search(&mut window, "jitter_bound", "1 of 1");
  window.until("the current place in the output", |window| {
    window
      .texts("conversation-search-current")
      .is_some_and(|texts| texts.concat().contains("jitter_bound"))
  });
  let opened: Vec<String> = ids(&mut window, "conversation-")
    .into_iter()
    .filter(|id| id.ends_with("-opened"))
    .collect();
  assert_eq!(
    opened.len(),
    2,
    "the group and the call say the search opened them: {opened:?}"
  );
  let around = ids(&mut window, "conversation-entry-")
    .into_iter()
    .find(|id| id.ends_with("-output"))
    .expect("the output");
  let head = window.text(&around);
  assert!(head.contains("around the matches"), "{head}");
  assert!(head.contains("Show all output"), "{head}");
}

#[test]
fn search_goes_round_its_places_and_closing_folds_what_it_opened() {
  let (mut window, _) = retry_webhooks();
  open_bar(&mut window, "sed -n");
  let total = found(&mut window);
  assert!(total > 2, "the Builder reads files with sed: {total}");
  window.press("conversation-search-next");
  window.until("the second place", |window| {
    window.text(COUNT) == format!("2 of {total}")
  });
  window.press("conversation-search-previous");
  window.press("conversation-search-previous");
  window.until("round past the start to the last", |window| {
    window.text(COUNT) == format!("{total} of {total}")
  });
  window.until("a group opened for its place", |window| {
    ids(window, "conversation-group-")
      .iter()
      .any(|id| id.ends_with("-opened"))
  });
  // Esc in the field closes the bar.
  window.press(FIELD);
  window
    .tree
    .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  window.until("the bar closed", |window| !exists(window, "conversation-search-bar"));
  window.until("what the search opened folded again", |window| {
    ids(window, "conversation-").iter().all(|id| !id.ends_with("-opened"))
  });
  assert!(
    !exists(&mut window, "conversation-search-current"),
    "the marks go with the bar"
  );
  // The query stays: Ctrl F brings it back.
  find_keys(&mut window);
  assert_eq!(found(&mut window), total, "the same places again");
}

#[test]
fn search_leaves_folded_what_the_person_folds() {
  let (mut window, _) = retry_webhooks();
  search(&mut window, "jitter_bound", "1 of 1");
  let mut opened = String::new();
  window.until("a group opened for the place", |window| {
    opened = ids(window, "conversation-group-")
      .into_iter()
      .find(|id| id.ends_with("-opened"))
      .unwrap_or_default();
    !opened.is_empty()
  });
  let group = opened.trim_end_matches("-opened").to_owned();
  window.press(&group);
  for _ in 0..5 {
    window.pass();
  }
  assert!(
    !exists(&mut window, &opened),
    "the group the person folded is theirs now"
  );
  assert!(
    exists(&mut window, &format!("{group}-matches")),
    "the folded group stays folded and counts its place"
  );
  assert!(
    !exists(&mut window, "conversation-search-current"),
    "the place stays hidden in the folded group"
  );
}

/// Lines of the call's output whose matches lie far apart, and the two lines that match.
const FAR_LINES: usize = 520;
const NEAR_MATCH: usize = 10;
const FAR_MATCH: usize = 500;

#[test]
fn search_shows_a_place_far_down_a_long_output() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project database");
  let invocation = at_work(&db, &key);
  let output: Vec<String> = (1..=FAR_LINES)
    .map(|line| {
      if line == NEAR_MATCH || line == FAR_MATCH {
        format!("line {line} needle_far")
      } else {
        format!("line {line}")
      }
    })
    .collect();
  let (item, command) = ("far-output".to_owned(), "cargo test far_output".to_owned());
  let at = Timestamp::now();
  let events = [
    (
      at,
      AgentEvent::CommandStarted {
        item: item.clone(),
        command: command.clone(),
      },
    ),
    (
      at,
      AgentEvent::CommandFinished {
        item,
        command,
        output: output.join("\n"),
        exit_code: Some(0),
        status: CommandStatus::Completed,
      },
    ),
  ];
  db.append_activity(&invocation, &events).expect("the call is added");
  open(&mut window, &seeded, &key, "conversation-chat");
  search(&mut window, "needle_far", "1 of 2");
  window.press("conversation-search-next");
  window.until("the far place current", |window| window.text(COUNT) == "2 of 2");
  let mut head = String::new();
  window.until("the output around the far place", |window| {
    head = ids(window, "conversation-entry-")
      .into_iter()
      .find(|id| id.ends_with("-output"))
      .map(|id| window.text(&id))
      .unwrap_or_default();
    head.contains("lines 303–502")
  });
  assert!(
    exists(&mut window, "conversation-search-current"),
    "the far place is drawn as the current one: {head}"
  );
}

#[test]
fn search_says_what_it_searched_when_nothing_matches() {
  let (mut window, _) = retry_webhooks();
  search(&mut window, "zebra crossing", "No matches");
  let note = window.text("conversation-search-note");
  assert!(note.starts_with("Searched all "), "{note}");
  assert!(note.ends_with("entries: messages, tool calls, output"), "{note}");
}

/// Messages and commands, one each per turn: 2,000 entries.
const TURNS: usize = 1_000;
/// The longest a frame may take while the long transcript is searched, in a debug build.
const FRAME_WITHIN: Duration = Duration::from_secs(2);
/// How long the search may take, in a debug build.
const FOUND_WITHIN: Duration = Duration::from_secs(10);

#[test]
fn search_a_long_transcript_off_the_ui_thread() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project database");
  let invocation = at_work(&db, &key);
  let start = Timestamp::now().unix_millis();
  let events: Vec<(Timestamp, AgentEvent)> = (0..TURNS)
    .flat_map(|turn| {
      let at = |offset: i64| Timestamp::from_unix_millis(start + i64::try_from(turn).expect("few turns") * 3 + offset);
      let (item, command) = (format!("long-{turn}"), format!("cargo test case_{turn}"));
      [
        (
          at(0),
          AgentEvent::Message {
            text: format!("Turn {turn} done."),
          },
        ),
        (
          at(1),
          AgentEvent::CommandStarted {
            item: item.clone(),
            command: command.clone(),
          },
        ),
        (
          at(2),
          AgentEvent::CommandFinished {
            item,
            command,
            output: format!("test case_{turn} ... ok"),
            exit_code: Some(0),
            status: CommandStatus::Completed,
          },
        ),
      ]
    })
    .collect();
  db.append_activity(&invocation, &events).expect("the turns are added");
  open(&mut window, &seeded, &key, "conversation-chat");
  window.until("the timeline's rows", |window| {
    !ids(window, "conversation-row:").is_empty()
  });
  find_keys(&mut window);
  window.until("the find bar", |window| exists(window, "conversation-search-bar"));
  window.set(FIELD, "case_7");
  // Every frame while the search runs, as the window draws them: none waits for the search.
  let (began, mut longest) = (Instant::now(), Duration::ZERO);
  let found = loop {
    window.tree.tick_futures();
    let frame = Instant::now();
    window.pass();
    longest = longest.max(frame.elapsed());
    let count = window.texts(COUNT).map(|texts| texts.concat()).unwrap_or_default();
    if count.starts_with("1 of ") {
      break count;
    }
    assert!(
      began.elapsed() < FOUND_WITHIN,
      "searching 2,000 entries took over {FOUND_WITHIN:?}: {count}"
    );
    std::thread::sleep(Duration::from_millis(5));
  };
  // case_7 and case_70…79 and case_700…799, each in a command's line and its output.
  assert_eq!(found, "1 of 222");
  assert!(
    longest < FRAME_WITHIN,
    "a frame took {longest:?} while 2,000 entries were searched in {:?}",
    began.elapsed()
  );
  let built = ids(&mut window, "conversation-row:").len();
  assert!(built < 200, "the list still builds the rows near its view: {built}");
}
