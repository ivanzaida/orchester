//! Searching the transcript (ORC-306, `Task page · redesign` `C3wC8` 13–13f) on KON-12's long transcript: Ctrl F (⌘ F
//! on macOS) opens the find bar beside the search button, the places are counted, the current one marked and brought
//! into view, next and previous go round them, a place in a closed tool group opens the group and the call with its
//! output around the place, nothing found says what was searched, Esc closes the bar and folds again what it opened,
//! and the query stays. A transcript 2,000 entries longer is searched off the UI thread: no frame waits for it. A
//! message fills behind its matched words, its Markdown rendered as ever, and its row is not ruled (ORC-622).

use std::time::{Duration, Instant};

use lurq::{
  app::theme::{PaletteColor, RadiusSize},
  layout::quad::QuadContent,
};
use orchester_app::ui::chat::found::{CURRENT_CLASS, MARKED_CLASS};
use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{agent_text::agent_says, ids, key_of, long::at_work, open, retry_webhooks, seeded, support::Window};

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

/// The texts of the message words that carry `class`, one entry per such element.
fn classed(window: &mut Window, class: &str) -> Vec<String> {
  let mut texts = Vec::new();
  for element in window.tree.get_elements_by_class_name(class) {
    let mut words = Vec::new();
    super::walk(element, &mut |inner| {
      words.extend(inner.text_content().map(str::to_owned))
    });
    texts.push(words.join(" "));
  }
  texts
}

/// The fills drawn inside the element `id`, `warning` (the current place) and `warning_muted` (another), each with its
/// corner radius: `(current, others)`.
fn fills(window: &mut Window, id: &str) -> (Vec<f32>, Vec<f32>) {
  window.pass();
  let (current, other) = {
    let theme = window.app.theme();
    let palette = theme.palette();
    (
      palette.get(PaletteColor::Warning),
      palette.get(PaletteColor::WarningMuted),
    )
  };
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let tree = &window.tree;
  let (mut currents, mut others) = (Vec::new(), Vec::new());
  for quad in tree.resolve_quads(tree.last_layout().expect("a layout")) {
    let QuadContent::Rect { color, .. } = quad.content else {
      continue;
    };
    let inside = quad.x >= bounds.x
      && quad.y >= bounds.y
      && quad.x + quad.width <= bounds.x + bounds.width + 0.5
      && quad.y + quad.height <= bounds.y + bounds.height + 0.5;
    if !inside {
      continue;
    }
    let radius = quad.border_radius.map_or(0.0, |radius| radius.top_left);
    if color == current {
      currents.push(radius);
    } else if color == other {
      others.push(radius);
    }
  }
  (currents, others)
}

/// The theme's `radius_sm`, a fill's corners.
fn small_radius(window: &Window) -> f32 {
  window.app.theme().radius_value(RadiusSize::Sm)
}

#[test]
fn search_fills_behind_the_matched_words_of_a_person_message() {
  let (mut window, _) = retry_webhooks();
  assert!(exists(&mut window, "task-search"), "the search button ends the tabs");
  search(&mut window, "CONFIGURABLE", "1 of 1");
  let mut current = Vec::new();
  window.until("the person's message holds the current place", |window| {
    current = classed(window, CURRENT_CLASS);
    current.iter().any(|words| words.contains("retry budget configurable"))
  });
  assert!(
    !exists(&mut window, "conversation-search-current-row") && !exists(&mut window, "conversation-search-row"),
    "a message's row is no longer ruled"
  );
  let message = super::with_text(&mut window, "conversation-message-", "retry budget configurable");
  let (currents, others) = fills(&mut window, &format!("{message}-text"));
  let radius = small_radius(&window);
  assert_eq!(
    currents,
    vec![radius],
    "one `warning` fill, `radius_sm`, behind the word"
  );
  assert!(others.is_empty(), "no other place in it: {others:?}");
}

#[test]
fn search_fills_behind_the_matched_words_of_an_agent_markdown_message() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  agent_says(
    &seeded,
    &key,
    "Read the **zanzibar** notes in [Zanzibar guide](https://example.com/guide), then set `zanzibar_ms`:\n\n- one \
     zanzibar item\n- two",
  );
  open(&mut window, &seeded, &key, "conversation-chat");
  search(&mut window, "zanzibar", "1 of 4");
  let message = super::with_text(&mut window, "conversation-text-", "Zanzibar guide");
  window.until("the agent's message holds the current place", |window| {
    classed(window, CURRENT_CLASS)
      .iter()
      .any(|words| words.contains("Zanzibar guide"))
  });
  let texts = window.text(&message);
  assert!(
    !texts.contains("**") && !texts.contains('`'),
    "its Markdown still renders: {texts}"
  );
  assert!(
    classed(&mut window, MARKED_CLASS)
      .iter()
      .any(|words| words.contains("one zanzibar item")),
    "its list still renders, its item marked"
  );
  let radius = small_radius(&window);
  let (currents, others) = fills(&mut window, &message);
  assert_eq!(currents, vec![radius], "the current place on `warning`");
  assert_eq!(
    others,
    vec![radius; 3],
    "the bold word, the link's and the code's on `warning_muted`"
  );
  // The next place becomes the current one within the same message.
  window.press("conversation-search-next");
  window.until("the second place", |window| window.text(COUNT) == "2 of 4");
  let (currents, others) = fills(&mut window, &message);
  assert_eq!(
    (currents.len(), others.len()),
    (1, 3),
    "the fills follow the current place"
  );
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
