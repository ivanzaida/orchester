//! *Jump to latest* after the person opened a tall tool row (a command whose output runs to hundreds of
//! lines, like a `read_file` of a skill): the chat goes to its end and follows again, whether the tall row
//! is above the end or is the last row.

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{Window, click, ids, key_of, long::at_work, open, seeded, wheel};

/// Turns added to KON-12's transcript, a message and a command each.
const TURNS: usize = 40;
/// The tall command's output, in lines.
const TALL_LINES: usize = 300;
const TALL_COMMAND: &str = "cargo run --example skill_dump";

/// KON-12 with [`TURNS`] more turns, the command of turn `tall` printing [`TALL_LINES`] lines, its chat
/// open at its end.
fn with_tall_command(tall: usize) -> Window {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project database");
  let invocation = at_work(&db, &key);
  let start = Timestamp::now().unix_millis();
  let events: Vec<(Timestamp, AgentEvent)> = (0..TURNS)
    .flat_map(|turn| {
      let at = |offset: i64| Timestamp::from_unix_millis(start + i64::try_from(turn).expect("few turns") * 3 + offset);
      let item = format!("jump-{turn}");
      let (command, output) = if turn == tall {
        let lines: Vec<String> = (1..=TALL_LINES)
          .map(|line| format!("line {line} of the skill"))
          .collect();
        (TALL_COMMAND.to_owned(), lines.join("\n"))
      } else {
        (format!("cargo test case_{turn}"), "ok".to_owned())
      };
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
            output,
            exit_code: Some(0),
            status: CommandStatus::Completed,
          },
        ),
      ]
    })
    .collect();
  db.append_activity(&invocation, &events).expect("the turns are added");
  open(&mut window, &seeded, &key, "conversation-chat");
  let last = format!("Turn {} done.", TURNS - 1);
  window.until("the last turn", |window| {
    window
      .texts("conversation")
      .is_some_and(|texts| texts.iter().any(|text| text.contains(&last)))
  });
  window
}

/// Whether the element `id` lies wholly inside the chat's view.
fn in_view(window: &mut Window, id: &str) -> bool {
  let bounds = |window: &mut Window, id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
  };
  let (Some(chat), Some(element)) = (bounds(window, "conversation-chat"), bounds(window, id)) else {
    return false;
  };
  element.y >= chat.y - 0.5 && element.y + element.height <= chat.y + chat.height + 0.5
}

/// The id of the element under the chat with `prefix` whose texts contain `text`, while it is built.
fn find(window: &mut Window, prefix: &str, text: &str) -> Option<String> {
  ids(window, prefix)
    .into_iter()
    .find(|id| window.texts(id).is_some_and(|texts| texts.join(" ").contains(text)))
}

/// Scrolls up to the tall command's chip, opens its group and its row, and returns the row's output id.
fn open_tall_row(window: &mut Window) -> String {
  let mut chip = None;
  for _ in 0..200 {
    chip = find(window, "conversation-group-tools-", "skill_dump").filter(|chip| in_view(window, chip));
    if chip.is_some() {
      break;
    }
    wheel(window, 120.0);
  }
  let chip = chip.expect("the tall command's chip scrolls into view");
  click(window, &chip);
  // Its rows open under the chip, at the chat's foot where *Jump to latest* floats: up into view.
  wheel(window, -300.0);
  let row = ids(window, "conversation-entry-")
    .into_iter()
    .filter(|id| id.ends_with("-title"))
    .find(|id| window.texts(id).is_some_and(|texts| texts.concat() == TALL_COMMAND))
    .expect("the tall command's row");
  let row = row.trim_end_matches("-title").to_owned();
  click(window, &format!("{row}-toggle"));
  let output = format!("{row}-output");
  window.until("the tall output", |window| window.texts(&output).is_some());
  output
}

/// Presses *Jump to latest* and lets the chat settle.
fn jump(window: &mut Window) {
  click(window, "conversation-jump");
  for _ in 0..20 {
    window.tree.tick_futures();
    window.pass();
  }
}

#[test]
fn timeline_jump_to_latest_reaches_the_end_past_an_opened_tall_row() {
  let mut window = with_tall_command(TURNS - 6);
  open_tall_row(&mut window);
  wheel(&mut window, 400.0);
  assert!(
    window.tree.get_element_by_id("conversation-jump").is_some(),
    "scrolled up, the chat no longer follows"
  );
  jump(&mut window);
  assert!(
    window.tree.get_element_by_id("conversation-jump").is_none(),
    "following again after the jump"
  );
  let last = find(&mut window, "conversation-group-tools-", &format!("case_{}", TURNS - 1));
  assert!(
    last.is_some_and(|last| in_view(&mut window, &last)),
    "the last turn's chip is in view after the jump"
  );
}

#[test]
fn timeline_jump_to_latest_reaches_the_end_of_an_opened_tall_last_row() {
  let mut window = with_tall_command(TURNS - 1);
  let output = open_tall_row(&mut window);
  wheel(&mut window, 2000.0);
  assert!(
    !in_view(&mut window, &output),
    "scrolled up, the tall output's end is out of view"
  );
  jump(&mut window);
  assert!(
    window.tree.get_element_by_id("conversation-jump").is_none(),
    "following again after the jump"
  );
  let bounds = |window: &mut Window, id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
  };
  let (chat, output) = (bounds(&mut window, "conversation-chat"), bounds(&mut window, &output));
  let end_shown = chat.zip(output).is_some_and(|(chat, output)| {
    let (bottom, chat_bottom) = (output.y + output.height, chat.y + chat.height);
    bottom <= chat_bottom + 0.5 && bottom >= chat.y
  });
  assert!(
    end_shown,
    "the tall output's end is in view after the jump: {output:?} in {chat:?}"
  );
}
