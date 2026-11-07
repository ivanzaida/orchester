//! A slash command that takes an argument (ORC-771; its picker v2, ORC-889; its badge, ORC-890), through `ui::slash`'s
//! public API with pickers of the test's own: choosing it pins one badge at the top of the composer and opens its
//! picker over the box, the focus in the picker's own search field, whose words filter it; Enter adds the focused
//! choice to the badge and closes the picker, the composer's field taking the focus back; Shift Enter adds it and
//! keeps the picker open, several allowed, and the badge names two then counts the rest; Esc closes the picker and
//! keeps the badge, and a press on the badge opens it again; once it is closed the words typed are the message, and
//! the composer's send applies the badge with them; Backspace in the empty field removes the last badge, and a second
//! command makes a second badge.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
  },
  components::{Column, Text, TextInput},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_app::ui::{
  icon::Icon,
  slash::{
    self, Applied, ArgumentPicker, Asked, Badge, Choice, Does, Offer, SkillReach, SkillsOffer, SlashCommand,
    SlashField, SlashMenuProps, Taking, with_chips,
  },
};
use parking_lot::Mutex;

use crate::harness::Kit;

const ID: &str = "test-composer";
const INPUT: &str = "test-input";
const BADGE: &str = "test-composer-badge-watch-region";
const PICKER: &str = "test-picker";
const SEARCH: &str = "test-picker-search";

/// The picker of the test: its search field, then four regions, their names filtered by what is typed there; `gamma`
/// cannot be chosen. A row says `focused` and `taken` after its name while it is.
struct Regions;

impl ArgumentPicker for Regions {
  fn offer(&self, ctx: &mut Ctx, asked: &Asked<'_>) -> Offer {
    let rows: Vec<(Choice, bool)> = ["alpha", "beta", "gamma", "delta"]
      .into_iter()
      .filter(|name| name.contains(asked.filter))
      .map(|name| {
        let choice = Choice {
          key: Arc::from(name),
          icon: Icon::Eye,
          label: Arc::from(name),
        };
        (choice, name != "gamma")
      })
      .collect();
    let search = asked.search.field(ctx, SEARCH, (Arc::from("Search regions"), None));
    let panel = Column::new()
      .id(PICKER)
      .width(Dimension::Px(300.0))
      .child(search)
      .with_children(rows.iter().enumerate().map(|(at, (choice, _))| {
        let mut words = choice.label.to_string();
        if asked.focused == Some(at) {
          words.push_str(" focused");
        }
        if asked.taken.contains(choice) {
          words.push_str(" taken");
        }
        Text::new(&words).id(format!("{PICKER}-{}", choice.key))
      }));
    Offer {
      rows,
      panel: panel.into(),
    }
  }
}

#[derive(Clone)]
struct Props {
  applied: Arc<Mutex<Vec<(&'static str, Applied)>>>,
  sent: Arc<Mutex<Vec<String>>>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.applied, &other.applied)
  }
}

impl DevtoolsInspectable for Props {}

struct Composer {
  draft: Signal<String>,
  focused: Signal<bool>,
  slash: SlashField,
  anchor: ElementRef,
  input: ElementRef,
}

/// A command named `name` with the test's picker, whose badge says `verb`; applying it keeps what it was applied to.
fn command(name: &'static str, verb: &str, applied: &Arc<Mutex<Vec<(&'static str, Applied)>>>) -> SlashCommand {
  let applied = applied.clone();
  SlashCommand {
    name,
    icon: Icon::Eye,
    label: Arc::from(format!("Watch {name}")),
    detail: Arc::from("Their events come here"),
    does: Some(Does::Take(Taking {
      picker: Arc::new(Regions),
      apply: Arc::new(move |done| {
        applied.lock().push((name, done));
        true
      }),
      badge: Badge {
        icon: Icon::Eye,
        verb: Arc::from(verb),
      },
    })),
  }
}

impl Component for Composer {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let (draft, focused, skill) = (ctx.signal(String::new()), ctx.signal(false), ctx.signal(None));
    let input = ElementRef::new();
    let slash = SlashField::new(ctx, draft.clone(), (focused.clone(), skill)).returns_focus_to(input.clone());
    Self {
      draft,
      focused,
      slash,
      anchor: ElementRef::new(),
      input,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let (gained, lost, slash) = (self.focused.clone(), self.focused.clone(), self.slash.clone());
    let (sent, draft) = (props.sent.clone(), self.draft.clone());
    let enter_sends = self.slash.holds_choices();
    let input = TextInput::new(self.draft.clone())
      .id(INPUT)
      .ref_element(self.input.clone())
      .width(Dimension::Px(400.0))
      .on_focus(move || gained.set(true))
      .on_blur(move || lost.set(false))
      .on_key_down(move |event: KeyboardEvent| {
        if slash.key(&event) || event.key != "Enter" || !(event.ctrl || (enter_sends && !event.shift)) {
          return;
        }
        let text = draft.get_untracked();
        if slash.apply_held(&text, None) {
          sent.lock().push(text);
          draft.set(String::new());
        }
      });
    let field = with_chips(ctx, (ID, SkillReach::Text, &self.anchor), &self.slash, input.into());
    let menu = slash::menu(
      ctx,
      SlashMenuProps {
        id: ID,
        field: self.slash.clone(),
        anchor: self.anchor.clone(),
        commands: vec![
          command("watch-region", "Watch", &props.applied),
          command("unwatch-region", "Stop watching", &props.applied),
        ],
        skills: SkillsOffer {
          root: std::env::temp_dir().join("orchester-no-such-project"),
          revision: 0,
          refused: None,
        },
        note: None,
        send: None,
      },
    );
    // Room over the box for the menus.
    Column::new()
      .child(Column::new().height(Dimension::Px(400.0)))
      .child(
        Column::new()
          .id("test-box")
          .ref_element(self.anchor.clone())
          .width(Dimension::Px(600.0))
          .child(field),
      )
      .child(menu)
  }
}

struct Fixture {
  kit: Kit,
  props: Props,
}

impl Fixture {
  fn new() -> Self {
    let props = Props {
      applied: Arc::default(),
      sent: Arc::default(),
    };
    let shown = props.clone();
    let mut kit = Kit::show(move |ctx| ctx.mount_keyed::<Composer>("argument-fixture", shown.clone()));
    kit.tree.get_element_by_id_mut(INPUT).expect("the field").focus();
    kit.pass();
    Self { kit, props }
  }

  /// `text` in the text input `id`.
  fn write(&mut self, id: &str, text: &str) {
    self
      .kit
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .set_value(text);
    self.kit.pass();
    self.kit.pass();
  }

  fn key(&mut self, (key, code): (&str, &str), (shift, ctrl): (bool, bool)) {
    let tree = &mut self.kit.tree;
    tree.key_down(key.to_owned(), code.to_owned(), shift, ctrl, false);
    tree.key_up(key.to_owned(), code.to_owned(), shift, ctrl, false);
    self.kit.pass();
    self.kit.pass();
  }

  /// The words of the picker's row `key`, with whether it is focused or taken.
  fn picker_row(&mut self, key: &str) -> String {
    self.kit.texts(&format!("{PICKER}-{key}")).join(" ")
  }

  /// What the badge `badge` says: its verb, then `·` and its choices, then *+N*, without its icons.
  fn badge(&mut self, badge: &str) -> String {
    let mut words = self.kit.texts(&format!("{badge}-verb")).concat();
    for (part, before) in [("targets", " · "), ("more", " ")] {
      let id = format!("{badge}-{part}");
      if self.kit.exists(&id) {
        words.push_str(before);
        words.push_str(&self.kit.texts(&id).concat());
      }
    }
    words
  }

  /// What the text input `id` holds.
  fn value(&mut self, id: &str) -> String {
    self
      .kit
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .value()
  }

  /// The command `typed` matches first, chosen in the slash menu with Enter.
  fn choose_command(&mut self, typed: &str) {
    self.write(INPUT, typed);
    assert!(self.kit.exists("test-composer-slash-menu"), "the slash menu");
    self.key(("Enter", "Enter"), (false, false));
  }
}

#[test]
fn slash_argument_becomes_one_badge_and_its_picker_adds_several_choices() {
  let mut fixture = Fixture::new();
  assert!(
    !fixture.kit.exists("test-composer-badges"),
    "no badge row while none is held"
  );
  fixture.choose_command("/watch-region");
  assert!(fixture.kit.exists(BADGE), "the badge");
  assert_eq!(fixture.badge(BADGE), "Watch", "the verb alone while nothing is chosen");
  assert_eq!(fixture.value(INPUT), "", "the typed / goes");
  assert!(fixture.kit.exists(PICKER), "the picker opens");
  assert!(fixture.kit.exists(SEARCH), "with its own search field");
  assert!(!fixture.kit.exists("test-composer-slash-menu"), "the slash menu closes");
  // Over the box, its bottom 8 over it.
  let (picker, boxed) = (fixture.kit.bounds(PICKER), fixture.kit.bounds("test-box"));
  assert!(
    (boxed.y - (picker.y + picker.height) - 8.0).abs() < 0.5,
    "{picker:?} {boxed:?}"
  );
  assert_eq!(fixture.picker_row("alpha"), "alpha focused");
  fixture.key(("b", "KeyB"), (false, false));
  assert_eq!(
    (fixture.value(SEARCH), fixture.value(INPUT)),
    ("b".to_owned(), String::new()),
    "the search holds the focus: what is typed goes there"
  );
  fixture.write(SEARCH, "");

  fixture.key(("Enter", "Enter"), (true, false));
  assert_eq!(
    fixture.badge(BADGE),
    "Watch · alpha",
    "Shift Enter adds alpha to the badge"
  );
  assert!(fixture.kit.exists(PICKER), "and keeps the picker open");
  assert_eq!(fixture.picker_row("alpha"), "alpha focused taken");
  fixture.key(("ArrowDown", "ArrowDown"), (false, false));
  assert_eq!(fixture.picker_row("beta"), "beta focused");
  fixture.key(("ArrowDown", "ArrowDown"), (false, false));
  assert_eq!(
    fixture.picker_row("delta"),
    "delta focused",
    "gamma cannot be chosen: the focus goes around it"
  );

  fixture.write(SEARCH, "bet");
  assert!(
    !fixture.kit.exists(&format!("{PICKER}-alpha")),
    "the search's words filter the picker"
  );
  assert_eq!(fixture.value(INPUT), "", "the composer's words stay as they were");
  fixture.key(("Enter", "Enter"), (false, false));
  assert_eq!(fixture.badge(BADGE), "Watch · alpha, beta", "Enter adds beta");
  assert!(!fixture.kit.exists(PICKER), "and closes the picker");

  fixture.key(("Backspace", "Backspace"), (false, false));
  assert!(
    !fixture.kit.exists(BADGE),
    "back in the composer's empty field, Backspace removes the badge"
  );
  assert!(!fixture.kit.exists(PICKER));
  assert!(!fixture.kit.exists("test-composer-badges"), "and its row");
}

#[test]
fn slash_argument_badge_names_two_choices_and_counts_the_rest() {
  let mut fixture = Fixture::new();
  fixture.choose_command("/watch-region");
  fixture.key(("Enter", "Enter"), (true, false));
  fixture.key(("ArrowDown", "ArrowDown"), (false, false));
  fixture.key(("Enter", "Enter"), (true, false));
  fixture.key(("ArrowDown", "ArrowDown"), (false, false));
  fixture.key(("Enter", "Enter"), (true, false));
  assert_eq!(fixture.badge(BADGE), "Watch · alpha, beta +1");
  assert_eq!(fixture.kit.texts(&format!("{BADGE}-more")), ["+1"]);
  // The badge sits in its own row over the message, at the composer's start.
  let (badge, field) = (fixture.kit.bounds(BADGE), fixture.kit.bounds(INPUT));
  assert!(badge.y + badge.height <= field.y, "{badge:?} over {field:?}");
  assert!((badge.x - field.x).abs() < 0.5, "{badge:?} at the start like {field:?}");

  fixture.kit.click(&format!("{BADGE}-remove"));
  assert!(!fixture.kit.exists(BADGE), "its x removes the badge");
  assert!(fixture.props.applied.lock().is_empty(), "nothing was applied");
}

#[test]
fn slash_argument_esc_keeps_the_badge_and_enter_sends_the_message_with_it() {
  let mut fixture = Fixture::new();
  fixture.choose_command("/watch-region");
  fixture.key(("Enter", "Enter"), (true, false));
  fixture.key(("Escape", "Escape"), (false, false));
  assert!(!fixture.kit.exists(PICKER), "Esc closes the picker");
  assert_eq!(fixture.badge(BADGE), "Watch · alpha", "the badge stays");

  fixture.kit.click(BADGE);
  assert!(fixture.kit.exists(PICKER), "the badge opens it again");
  assert_eq!(fixture.value(SEARCH), "", "with an empty search");
  assert_eq!(fixture.picker_row("alpha"), "alpha focused taken");
  fixture.key(("Escape", "Escape"), (false, false));

  fixture.write(INPUT, "only the merges");
  assert!(!fixture.kit.exists(PICKER), "the words are the message, not a filter");
  fixture.key(("Enter", "Enter"), (false, false));
  let applied = fixture.props.applied.lock().clone();
  assert_eq!(applied.len(), 1, "Enter applies the badge");
  let (name, done) = &applied[0];
  assert_eq!(*name, "watch-region");
  assert_eq!(done.text, "only the merges", "with the message's words");
  let keys: Vec<&str> = done.choices.iter().map(|choice| &*choice.key).collect();
  assert_eq!(keys, ["alpha"]);
  assert_eq!(
    *fixture.props.sent.lock(),
    ["only the merges"],
    "and the message is sent"
  );
  assert!(!fixture.kit.exists(BADGE), "the badge goes");

  fixture.write(INPUT, "plain words");
  fixture.key(("Enter", "Enter"), (false, false));
  assert_eq!(
    fixture.props.sent.lock().len(),
    1,
    "without a badge, Enter alone does not send"
  );
}

#[test]
fn slash_argument_two_commands_make_two_badges_and_backspace_removes_the_last() {
  let mut fixture = Fixture::new();
  fixture.choose_command("/watch-region");
  fixture.key(("Enter", "Enter"), (false, false));
  assert!(!fixture.kit.exists(PICKER), "Enter picks alpha and closes the picker");
  fixture.choose_command("/unwatch-region");
  let second = "test-composer-badge-unwatch-region";
  assert!(fixture.kit.exists(second), "a second badge");
  fixture.key(("ArrowDown", "ArrowDown"), (false, false));
  fixture.key(("Enter", "Enter"), (false, false));
  assert!(!fixture.kit.exists(PICKER));
  assert_eq!(fixture.badge(BADGE), "Watch · alpha");
  assert_eq!(fixture.badge(second), "Stop watching · beta");
  let (first, next) = (fixture.kit.bounds(BADGE), fixture.kit.bounds(second));
  assert!(
    (first.y - next.y).abs() < 0.5 && next.x > first.x + first.width,
    "side by side: {first:?} {next:?}"
  );

  fixture.key(("Backspace", "Backspace"), (false, false));
  assert!(!fixture.kit.exists(second), "Backspace removes the last badge");
  assert!(fixture.kit.exists(BADGE), "the first stays");
}
