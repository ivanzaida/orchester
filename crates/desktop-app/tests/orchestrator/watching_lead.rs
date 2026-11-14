//! A subscription that leads its target to the end, in the `/watching` answer (`L02 · /watching` `HpcQJ` and `Parts ·
//! Ceilings` a–d `H6wvm`, board `Orchestrator autonomy · frames to approve`; OA-10): the total names what is led to
//! the end and what is followed; its head row has the mode control before *Stop leading*, and under it the ceilings
//! row with *Change…*; a followed one has neither. The form opens on the subscription's ceilings, says what is
//! allowed in place of a hint whose field is out of bounds and saves nothing until every field is in bounds, then
//! applies at once; Cancel and Esc change nothing. The mode's menu has both ways, the current one checked, and
//! *Follow* has the conversation only follow from then on. Changing either is a person's.
//! `cargo test -p orchester-app --features test-support --test orchestrator watching_lead`.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  id::ChatId,
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{Ceilings, Chat, WatchMode, WatchTarget, watch::NamedTarget},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_workspace::orchestrator::watch::{lead_in, watch_in, watching_in};

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
};

/// What the conversation under test shows of its `/watching`.
struct Answer {
  screen: Orchestrator,
  chat: ChatId,
  /// The module it leads to the end.
  led: WatchTarget,
  /// The row of the answer.
  row: String,
}

impl Answer {
  /// The head row of subscription `at`.
  fn head(&self, at: usize) -> String {
    format!("{}-answer-subscription-{at}", self.row)
  }

  /// The ceilings row of subscription `at`; its form is `{row}-form`.
  fn ceilings(&self, at: usize) -> String {
    format!("{}-answer-ceilings-{at}", self.row)
  }

  /// How the conversation takes the module now.
  fn mode(&self) -> WatchMode {
    let subscriptions = self.screen.db().active_subscriptions().expect("read");
    subscriptions.of(self.led).expect("still watched").mode()
  }

  fn value(&mut self, id: &str) -> String {
    self
      .screen
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .value()
  }
}

/// The Orchestrator on a conversation that leads the module *Webhooks v2* to the end and follows the tasks without a
/// module, its `/watching` asked.
fn answer() -> Answer {
  let mut screen = Orchestrator::new(Setup::Ready);
  let chat = kept(&screen, "Webhooks work", Timestamp::now(), "Noted.");
  let details = ModuleDetails::new(
    Name::new("Webhooks v2").expect("name"),
    "",
    (None, None),
    ModuleState::Open,
  )
  .expect("details");
  let module = screen
    .db()
    .create_module(details, Timestamp::now())
    .expect("module")
    .id();
  let led = NamedTarget {
    target: WatchTarget::Module(module),
    name: Some("Webhooks v2".to_owned()),
  };
  let rest = NamedTarget {
    target: WatchTarget::WithoutModule,
    name: None,
  };
  let here = Chat::named(chat, "", Timestamp::now());
  lead_in(
    &screen.root,
    &here,
    std::slice::from_ref(&led),
    "Answer agents from the brief",
  )
  .expect("led");
  watch_in(&screen.root, &here, &[rest], "").expect("followed");
  watching_in(&screen.root, &here).expect("asked");
  let seq = screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .last()
    .expect("the command")
    .seq;
  let row = format!("orchestrator-watching-wait-{seq}");
  screen.go(OrchestratorChat::Latest);
  let ceilings = format!("{row}-answer-ceilings-0");
  screen.until("the answer with its ceilings", |screen| screen.exists(&ceilings));
  Answer {
    screen,
    chat,
    led: led.target,
    row,
  }
}

#[test]
fn orchestrator_watching_lead_shows_the_mode_and_the_ceilings_of_what_is_led_to_the_end() {
  let mut answer = answer();
  let total = answer.screen.text(&format!("{}-answer-total", answer.row));
  assert!(
    total.starts_with("This conversation leads Webhooks v2 to the end and follows "),
    "{total}"
  );
  let (led, followed) = (answer.head(0), answer.head(1));
  let head = answer.screen.text(&led);
  for words in ["Webhooks v2", "module · since", "Lead to the end", "Stop leading"] {
    assert!(head.contains(words), "{words}: {head}");
  }
  assert!(!head.contains("Stop watching"), "{head}");
  let (mode, stop) = (
    answer.screen.bounds(&format!("{led}-mode")),
    answer.screen.bounds(&format!("{led}-stop")),
  );
  assert!(
    mode.x + mode.width <= stop.x,
    "the mode before Stop leading: {mode:?} {stop:?}"
  );
  assert!((mode.height - 22.0).abs() < 0.5, "{mode:?}");

  // A followed subscription keeps Stop watching, with no mode control and no ceilings.
  let head = answer.screen.text(&followed);
  assert!(
    head.contains("Stop watching") && !head.contains("Lead to the end"),
    "{head}"
  );
  assert!(!answer.screen.exists(&format!("{followed}-mode")));
  assert!(!answer.screen.exists(&answer.ceilings(1)));

  // The ceilings, right under the head row.
  let row = answer.ceilings(0);
  assert_eq!(
    answer.screen.text(&row),
    "Runs of a task 3 Returns per run 4 Raises 1 by 50 % Budget none Change…"
  );
  let (head, ceilings) = (answer.screen.bounds(&led), answer.screen.bounds(&row));
  assert!(
    (ceilings.y - (head.y + head.height)).abs() < 0.5 && (ceilings.height - 30.0).abs() < 0.5,
    "{head:?} {ceilings:?}"
  );
}

/// The form `form` names each ceiling with its bounds, and ends in its note and its two buttons.
fn says_its_rows(answer: &mut Answer, form: &str) {
  let hint = |name: &str| format!("{form}-{name}-hint");
  for (name, said) in [
    ("runs", "How many times it may run one task again · 1 to 5"),
    ("returns", "Returns to an earlier step inside one run · 0 to 10"),
    (
      "raises",
      "How often it may raise a run's limit, and by how much · 0 to 3, up to 100 %",
    ),
    ("budget", "Tokens for the whole target · empty: no budget"),
  ] {
    assert_eq!(answer.screen.text(&hint(name)), said);
  }
  let shown = answer.screen.text(form);
  for words in [
    "Runs of a task",
    "Returns per run",
    "Raises of a limit",
    "by",
    "%",
    "Budget",
    "tokens",
    "This subscription only · defaults in Project settings ↗",
    "Cancel",
    "Save",
  ] {
    assert!(shown.contains(words), "{words}: {shown}");
  }
}

#[test]
fn orchestrator_watching_lead_changes_the_ceilings_in_the_form_once_every_field_is_in_bounds() {
  let mut answer = answer();
  let row = answer.ceilings(0);
  let (change, form) = (format!("{row}-change"), format!("{row}-form"));
  let field = |name: &str| format!("{form}-{name}-field");
  let hint = |name: &str| format!("{form}-{name}-hint");
  answer.screen.click(&change);
  answer.screen.until("the form", |screen| screen.exists(&form));
  assert_eq!(answer.screen.text(&format!("{form}-title")), "Ceilings · Webhooks v2");
  assert!((answer.screen.bounds(&form).width - 440.0).abs() < 0.5);
  says_its_rows(&mut answer, &form);
  let values = ["runs", "returns", "raises", "percent", "budget"].map(|name| answer.value(&field(name)));
  assert_eq!(values, ["3", "4", "1", "50", ""], "the subscription's own ceilings");

  // Out of bounds, as the frame draws it: the hint says what is allowed, and Save does nothing.
  answer.screen.set(&field("runs"), "8");
  answer.screen.set(&field("percent"), "150");
  answer.screen.set(&field("budget"), "lots");
  answer
    .screen
    .until("what is allowed", |screen| screen.text(&hint("runs")) == "From 1 to 5");
  assert_eq!(answer.screen.text(&hint("raises")), "Up to 100 %");
  assert_eq!(
    answer.screen.text(&hint("budget")),
    "A number of tokens, or empty for no budget"
  );
  assert_eq!(
    answer.screen.text(&hint("returns")),
    "Returns to an earlier step inside one run · 0 to 10",
    "a field in bounds keeps its hint"
  );
  answer.screen.click(&format!("{form}-save"));
  assert!(answer.screen.exists(&form), "the form stays");
  assert_eq!(
    answer.mode(),
    WatchMode::Lead {
      ceilings: Ceilings::default()
    },
    "nothing was applied"
  );

  // In bounds again; automation's Save is refused, the person's applies at once and closes the form.
  answer.screen.set(&field("runs"), "5");
  answer.screen.set(&field("percent"), "100");
  answer.screen.set(&field("budget"), "40000000");
  answer.screen.until("the hints back", |screen| {
    screen.text(&hint("runs")) == "How many times it may run one task again · 1 to 5"
  });

  with_synthetic_input(|| answer.screen.click(&format!("{form}-save")));
  assert!(answer.screen.window.refused_press().is_some(), "refused to the client");
  assert_eq!(
    answer.mode(),
    WatchMode::Lead {
      ceilings: Ceilings::default()
    },
    "a synthetic press changes no ceiling"
  );
  assert!(answer.screen.exists(&form), "and the form stays, with what was typed");
  assert_eq!(answer.value(&field("runs")), "5");
  answer.screen.click(&format!("{form}-save"));
  let budget = TokenBudget::new(40_000_000).expect("a budget");
  let asked = Ceilings::new((5, 4), (1, 100), Some(budget)).expect("ceilings");
  answer.screen.until("the ceilings applied", |screen| {
    let subscriptions = screen.db().active_subscriptions().expect("read");
    subscriptions
      .as_slice()
      .iter()
      .any(|led| led.mode() == WatchMode::Lead { ceilings: asked })
  });
  answer.screen.until("the row to say them", |screen| {
    !screen.exists(&form)
      && screen.text(&row) == "Runs of a task 5 Returns per run 4 Raises 1 by 100 % Budget 0 of 40M tokens Change…"
  });
  assert!(!answer.screen.exists("orchestrator-watching-problem"));
}

/// Cancel and Esc change nothing, and the form opens again on the subscription's ceilings, whatever was typed.
#[test]
fn orchestrator_watching_lead_form_forgets_what_was_typed_when_it_is_cancelled() {
  let mut answer = answer();
  let row = answer.ceilings(0);
  let (change, form) = (format!("{row}-change"), format!("{row}-form"));
  let field = |name: &str| format!("{form}-{name}-field");
  open(&mut answer.screen, &change, &form);
  assert_eq!(answer.value(&field("runs")), "3");
  answer.screen.set(&field("runs"), "1");
  answer.screen.click(&format!("{form}-cancel"));
  answer.screen.until("the form closed", |screen| !screen.exists(&form));
  open(&mut answer.screen, &change, &form);
  assert_eq!(answer.value(&field("runs")), "3", "what was typed before is gone");
  answer.screen.key(("Escape", "Escape"), false);
  answer.screen.until("Esc closes it", |screen| !screen.exists(&form));
  assert_eq!(
    answer.mode(),
    WatchMode::Lead {
      ceilings: Ceilings::default()
    }
  );
}

/// Presses `opener` until `shown` is there. The answer is read again after a change of its subscriptions, and a
/// press that lands while its rows are rebuilt opens nothing: a person presses again, and so does this.
fn open(screen: &mut crate::harness::Orchestrator, opener: &str, shown: &str) {
  screen.until("what the press opens", |screen| {
    if !screen.exists(shown) {
      screen.click(opener);
    }
    screen.exists(shown)
  });
}

#[test]
fn orchestrator_watching_lead_follows_from_the_mode_menu_by_a_persons_press_only() {
  let mut answer = answer();
  let control = format!("{}-mode", answer.head(0));
  let menu = format!("{control}-menu");
  open(&mut answer.screen, &control, &menu);
  assert!((answer.screen.bounds(&menu).width - 440.0).abs() < 0.5);
  assert_eq!(
    answer.screen.text(&format!("{menu}-follow")),
    "Follow Tells you and answers agents; you start, merge and deliver"
  );
  assert_eq!(
    answer.screen.text(&format!("{menu}-lead")),
    "Lead to the end Starts its tasks, runs failed ones again and leaves you what it cannot do"
  );
  let (follow, lead) = (
    answer.screen.bounds(&format!("{menu}-follow")),
    answer.screen.bounds(&format!("{menu}-lead")),
  );
  assert!(follow.y < lead.y, "Follow first: {follow:?} {lead:?}");

  // The current way closes the menu and changes nothing.
  answer.screen.click(&format!("{menu}-lead"));
  answer.screen.until("the menu closed", |screen| !screen.exists(&menu));
  assert!(answer.mode().ceilings().is_some());

  // Automation's choice is refused.
  open(&mut answer.screen, &control, &menu);
  with_synthetic_input(|| answer.screen.click(&format!("{menu}-follow")));
  assert!(answer.screen.window.refused_press().is_some(), "refused to the client");
  assert!(answer.mode().ceilings().is_some(), "a synthetic press changes no mode");

  // The person's: the conversation only follows the module from now on, with the same subscription.
  let since = {
    let subscriptions = answer.screen.db().active_subscriptions().expect("read");
    subscriptions.of(answer.led).expect("led").since()
  };
  open(&mut answer.screen, &control, &menu);
  answer.screen.click(&format!("{menu}-follow"));
  answer.screen.until("followed", |screen| {
    let subscriptions = screen.db().active_subscriptions().expect("read");
    subscriptions
      .as_slice()
      .iter()
      .all(|led| led.mode() == WatchMode::Follow)
  });
  let (head, ceilings, total) = (
    answer.head(0),
    answer.ceilings(0),
    format!("{}-answer-total", answer.row),
  );
  answer.screen.until("the answer to follow the live state", |screen| {
    !screen.exists(&control) && !screen.exists(&ceilings) && screen.text(&head).contains("Stop watching")
  });
  assert!(
    answer
      .screen
      .text(&total)
      .starts_with("This conversation leads 1 module"),
    "{}",
    answer.screen.text(&total)
  );
  let subscriptions = answer.screen.db().active_subscriptions().expect("read");
  let now = subscriptions.of(answer.led).expect("still watched");
  assert_eq!((now.chat(), now.since()), (answer.chat, since), "nothing ended");
  assert!(!answer.screen.exists("orchestrator-watching-problem"));
}
