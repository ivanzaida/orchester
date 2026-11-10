//! What the compact *Runs on* picker takes from a chat that has more to say than the pipelines' use of it (`05b · On
//! DeepSeek` `AZp3I`, `Codex at its limit` `yfqxy`, `Another account` `vsqci` in `Orchestrator · redesign` `hujxu`): a
//! second line under a connection's name, a model Search takes that the connection does not list, an effort typed in
//! a field where the connection lists none, notes under the efforts, and a tooltip with a note, headless at 1440 × 900.

use orchester_app::ui::{
  icon::Icon,
  runs_on_picker::{Chosen, Connection, Detail, Notice, NoticeTone, Pick, RunsOnPicker},
};

use super::runs_on_picker::{ID, chosen, connections, effort, item, key, open_over, row, type_text};
use crate::harness::{Kit, assert_close};

/// The words of a note: its texts without the glyph before them.
fn words(kit: &mut Kit, id: &str) -> String {
  kit.texts(id).pop().expect("a note says something")
}

/// The fixture connections with `edit` applied to the one named `id`.
fn with(id: &str, edit: impl FnOnce(&mut Connection)) -> Vec<Connection> {
  let mut all = connections();
  edit(all.iter_mut().find(|connection| &*connection.id == id).expect("a connection"));
  all
}

fn on_deepseek(effort: Option<&str>) -> Chosen {
  Chosen {
    connection: "deepseek".into(),
    model: Some("deepseek-v4-pro".into()),
    effort: effort.map(Into::into),
  }
}

/// An API provider as `05b` draws it: it lists two models, takes others and a typed effort, and says where data goes.
fn provider() -> Vec<Connection> {
  with("deepseek", |connection| {
    connection.caption = "Claude Code · api.deepseek.com".into();
    connection.detail = Some(Detail {
      text: "1.2M today · no plan limit".into(),
      tip: Some("Its key: secret DEEPSEEK_KEY".into()),
      warns: false,
    });
    connection.typed_model = true;
    connection.typed_effort = true;
    connection.models = vec![
      super::runs_on_picker::model("deepseek-v4-pro", &[]),
      super::runs_on_picker::model("deepseek-v4-flash", &[]),
    ];
    connection.notice = Some(Notice {
      icon: Icon::Send,
      tone: NoticeTone::Warning,
      text: "Prompts, repository content and command output go to api.deepseek.com under its terms.".into(),
    });
  })
}

#[test]
fn a_detail_is_a_second_line_under_the_name_and_a_limit_fills_the_meter_in_warning() {
  let limit = with("home", |connection| {
    connection.caption = "Claude Code · limit until 21:51".into();
    connection.five_hour = Some(100);
    connection.detail = Some(Detail {
      text: "limit reached".into(),
      tip: None,
      warns: true,
    });
  });
  let mut open = open_over(
    limit,
    Chosen {
      connection: "home".into(),
      model: Some("claude-opus-5-5".into()),
      effort: None,
    },
  );
  let kit = &mut open.kit;
  assert_eq!(kit.texts(&format!("{ID}-who-detail")), ["limit reached"]);
  assert!(
    !kit.exists(&format!("{ID}-who-use")),
    "the second line replaces the use in words"
  );
  let (first, second) = (
    kit.bounds(&format!("{ID}-who-first")),
    kit.bounds(&format!("{ID}-who-second")),
  );
  assert_close(second.y - (first.y + first.height), 3.0, "the lines are 3 apart");
  let (meter, line) = (kit.bounds(&format!("{ID}-who-meter")), second);
  assert_close(meter.width, 32.0, "the meter's width");
  assert_close(
    meter.x + meter.width,
    line.x + line.width,
    "the meter ends the second line",
  );
  assert_close(
    kit.bounds(&format!("{ID}-who-meter-fill")).width,
    32.0,
    "a limit fills the meter",
  );
}

#[test]
fn an_api_provider_says_its_use_on_a_second_line_with_no_meter_and_where_its_data_goes() {
  let mut open = open_over(provider(), on_deepseek(None));
  let kit = &mut open.kit;
  assert_eq!(kit.texts(&format!("{ID}-who-detail")), ["1.2M today · no plan limit"]);
  assert!(!kit.exists(&format!("{ID}-who-meter")), "no plan limit, no meter");
  assert_eq!(
    words(kit, &format!("{ID}-note-0")),
    "Prompts, repository content and command output go to api.deepseek.com under its terms."
  );
  kit.click(&item("work"));
  assert!(
    !kit.exists(&format!("{ID}-note-0")),
    "the note is the connection's: another one shown has none"
  );
}

#[test]
fn search_takes_a_model_the_connection_does_not_list_and_enter_hands_it_back() {
  let mut open = open_over(provider(), on_deepseek(None));
  let kit = &mut open.kit;
  type_text(kit, "deepseek-v4-ultra");
  assert_eq!(kit.texts(&format!("{ID}-typed")), ["Use deepseek-v4-ultra"]);
  assert!(!kit.exists(&row(0)), "nothing listed holds it");
  key(kit, "Enter");
  assert_eq!(
    *open.picks.lock(),
    [Pick::Model {
      connection: "deepseek".into(),
      model: "deepseek-v4-ultra".into()
    }]
  );
}

#[test]
fn a_model_that_is_listed_is_not_offered_twice_and_a_connection_that_lists_all_it_takes_offers_none() {
  let mut open = open_over(provider(), on_deepseek(None));
  let kit = &mut open.kit;
  type_text(kit, "deepseek-v4-flash");
  assert!(!kit.exists(&format!("{ID}-typed")), "it is in the list already");
  assert_eq!(kit.texts(&row(0)), ["deepseek-v4-flash"]);
  let mut open = open_over(connections(), chosen());
  type_text(&mut open.kit, "no-such-model");
  assert!(
    !open.kit.exists(&format!("{ID}-typed")),
    "a CLI takes only the models it lists"
  );
}

#[test]
fn an_effort_is_typed_where_the_connection_lists_none_and_nothing_typed_is_default() {
  let mut open = open_over(provider(), on_deepseek(None));
  let kit = &mut open.kit;
  assert!(!kit.exists(&effort("default")), "no segments, a field");
  assert_eq!(
    kit.texts(&format!("{ID}-effort-hint")),
    ["The provider lists no efforts. Empty is Default."]
  );
  assert_close(kit.bounds(&format!("{ID}-effort-field")).height, 28.0, "the field's height");
  kit.click(&format!("{ID}-effort-typed"));
  type_text(kit, "think-hard");
  key(kit, "Enter");
  assert_eq!(*open.picks.lock(), [Pick::Effort(Some("think-hard".into()))]);

  let mut open = open_over(provider(), on_deepseek(Some("think-hard")));
  let kit = &mut open.kit;
  assert_eq!(
    kit.texts(&format!("{ID}-effort-typed")),
    ["think-hard"],
    "the field shows the effort chosen"
  );
  kit.click(&format!("{ID}-effort-typed"));
  for _ in 0.."think-hard".len() {
    key(kit, "Backspace");
  }
  key(kit, "Enter");
  assert_eq!(*open.picks.lock(), [Pick::Effort(None)], "empty is Default");
}

#[test]
fn tab_takes_the_keys_to_the_typed_effort_where_space_is_typed_and_enter_chooses() {
  let mut open = open_over(provider(), on_deepseek(None));
  let kit = &mut open.kit;
  key(kit, "Tab");
  type_text(kit, "a b");
  key(kit, "Enter");
  assert_eq!(*open.picks.lock(), [Pick::Effort(Some("a b".into()))]);
}

#[test]
fn the_picker_shows_its_own_notes_under_the_efforts_whichever_connection_is_shown() {
  let mut kit = Kit::show(|ctx| {
    RunsOnPicker::new(ID, (connections(), chosen()), |_| {}, || {})
      .notes(vec![Notice {
        icon: Icon::CircleAlert,
        tone: NoticeTone::Danger,
        text: "Not saved: the settings are being edited".into(),
      }])
      .element(ctx)
  });
  kit.pass();
  assert_eq!(words(&mut kit, &format!("{ID}-note-0")), "Not saved: the settings are being edited");
  kit.click(&item("team"));
  assert!(kit.exists(&format!("{ID}-note-0")), "whichever connection is shown");
}

#[test]
fn a_tooltip_can_carry_a_note_under_its_title() {
  let others = with("home", |connection| {
    connection.tip = "Claude Code · Home".into();
    connection.tip_note = Some("Another account starts a new session: earlier messages aren't remembered.".into());
  });
  let mut open = open_over(others, chosen());
  let kit = &mut open.kit;
  kit.rest_on(&item("home"));
  assert_eq!(
    kit.texts(&format!("{}-tip-tooltip", item("home"))),
    [
      "Claude Code · Home",
      "Another account starts a new session: earlier messages aren't remembered."
    ]
  );
  kit.rest_on(&item("team"));
  assert_eq!(kit.texts(&format!("{}-tip-tooltip", item("team"))), ["team tip"]);
}

#[test]
fn the_list_opens_scrolled_to_the_chosen_model() {
  let mut open = open_over(
    connections(),
    Chosen {
      connection: "work".into(),
      model: Some("sonnet".into()),
      effort: None,
    },
  );
  let kit = &mut open.kit;
  let list = kit.bounds(&format!("{ID}-model-scroll"));
  let chosen = kit.bounds(&row(5));
  assert!(
    chosen.y >= list.y - 0.5 && chosen.y + chosen.height <= list.y + list.height + 0.5,
    "the chosen model, the last of six, shows whole in a list of four rows: {chosen:?} {list:?}"
  );
}
