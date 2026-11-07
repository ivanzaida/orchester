//! A slash command that takes an argument (ORC-771; its picker v2, ORC-889), through `ui::slash`'s public API with a
//! picker of the test's own: choosing it makes the command chip and opens its picker over the box, the focus in the
//! picker's own search field, whose words filter it; Enter adds the focused choice as a chip and closes the picker,
//! the composer's field taking the focus back; Shift Enter adds it and keeps the picker open; Backspace in an empty
//! field removes the last chip; Esc closes the picker and keeps the chips, and the command chip opens it again; the
//! words in the composer are the command's text; Ctrl Enter applies the command to its choices and text instead of
//! sending.

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
    self, Applied, ArgumentPicker, Asked, Choice, Does, Offer, SkillReach, SkillsOffer, SlashCommand, SlashField,
    SlashMenuProps, Taking, with_chips,
  },
};
use parking_lot::Mutex;

use crate::harness::Kit;

const ID: &str = "test-composer";
const INPUT: &str = "test-input";
const CHIP: &str = "test-composer-command-chip";
const PICKER: &str = "test-picker";
const SEARCH: &str = "test-picker-search";

/// The picker of the test: its search field, then three regions, their names filtered by what is typed there; `gamma`
/// cannot be chosen. A row says `focused` and `taken` after its name while it is.
struct Regions;

impl ArgumentPicker for Regions {
  fn offer(&self, ctx: &mut Ctx, asked: &Asked<'_>) -> Offer {
    let rows: Vec<(Choice, bool)> = ["alpha", "beta", "gamma"]
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
  applied: Arc<Mutex<Vec<Applied>>>,
  sent: Arc<Mutex<u32>>,
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
    let sent = props.sent.clone();
    let input = TextInput::new(self.draft.clone())
      .id(INPUT)
      .ref_element(self.input.clone())
      .width(Dimension::Px(400.0))
      .on_focus(move || gained.set(true))
      .on_blur(move || lost.set(false))
      .on_key_down(move |event: KeyboardEvent| {
        if !slash.key(&event) && event.key == "Enter" && event.ctrl {
          *sent.lock() += 1;
        }
      });
    let field = with_chips(ctx, (ID, SkillReach::Text, &self.anchor), &self.slash, input.into());
    let applied = props.applied;
    let regions = SlashCommand {
      name: "watch-region",
      icon: Icon::Eye,
      label: Arc::from("Watch regions"),
      detail: Arc::from("Their events come here"),
      does: Some(Does::Take(Taking {
        picker: Arc::new(Regions),
        apply: Arc::new(move |done| {
          applied.lock().push(done);
          true
        }),
      })),
    };
    let menu = slash::menu(
      ctx,
      SlashMenuProps {
        id: ID,
        field: self.slash.clone(),
        anchor: self.anchor.clone(),
        commands: vec![regions],
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

  /// `/watch` chosen in the slash menu with Enter.
  fn choose_command(&mut self) {
    self.write(INPUT, "/watch");
    assert!(self.kit.exists("test-composer-slash-watch-region"), "the command's row");
    self.key(("Enter", "Enter"), (false, false));
  }
}

#[test]
fn slash_argument_becomes_a_chip_and_its_picker_adds_several_choices() {
  let mut fixture = Fixture::new();
  fixture.choose_command();
  assert!(fixture.kit.exists(CHIP), "the command chip");
  assert_eq!(fixture.kit.texts(&format!("{CHIP}-name")), ["/watch-region"]);
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
  assert!(
    fixture.kit.exists("test-composer-choice-alpha"),
    "Shift Enter adds alpha"
  );
  assert!(fixture.kit.exists(PICKER), "and keeps the picker open");
  assert_eq!(fixture.picker_row("alpha"), "alpha focused taken");
  fixture.key(("ArrowDown", "ArrowDown"), (false, false));
  assert_eq!(fixture.picker_row("beta"), "beta focused");
  fixture.key(("ArrowDown", "ArrowDown"), (false, false));
  assert_eq!(
    fixture.picker_row("alpha"),
    "alpha focused taken",
    "gamma cannot be chosen: the focus goes around it"
  );

  fixture.write(SEARCH, "bet");
  assert!(
    !fixture.kit.exists(&format!("{PICKER}-alpha")),
    "the search's words filter the picker"
  );
  assert_eq!(fixture.value(INPUT), "", "the composer's words stay as they were");
  fixture.key(("Enter", "Enter"), (false, false));
  assert!(fixture.kit.exists("test-composer-choice-beta"), "Enter adds beta");
  assert!(!fixture.kit.exists(PICKER), "and closes the picker");

  fixture.key(("Backspace", "Backspace"), (false, false));
  assert!(
    !fixture.kit.exists("test-composer-choice-beta"),
    "back in the composer's field, Backspace removes the last chip"
  );
  assert!(fixture.kit.exists("test-composer-choice-alpha"));

  fixture.kit.click("test-composer-choice-alpha-remove");
  assert!(
    !fixture.kit.exists("test-composer-choice-alpha"),
    "its x removes a chip"
  );
  fixture.key(("Backspace", "Backspace"), (false, false));
  assert!(
    !fixture.kit.exists(CHIP),
    "with no choice left, Backspace removes the command"
  );
  assert!(!fixture.kit.exists(PICKER));
}

#[test]
fn slash_argument_esc_keeps_the_chips_and_ctrl_enter_applies_with_the_text() {
  let mut fixture = Fixture::new();
  fixture.choose_command();
  fixture.key(("Enter", "Enter"), (true, false));
  fixture.key(("Escape", "Escape"), (false, false));
  assert!(!fixture.kit.exists(PICKER), "Esc closes the picker");
  assert!(fixture.kit.exists(CHIP) && fixture.kit.exists("test-composer-choice-alpha"));

  fixture.kit.click(CHIP);
  assert!(fixture.kit.exists(PICKER), "the command chip opens it again");
  assert_eq!(fixture.value(SEARCH), "", "with an empty search");
  assert_eq!(fixture.picker_row("alpha"), "alpha focused taken");
  fixture.key(("Escape", "Escape"), (false, false));

  fixture.write(INPUT, "only the merges");
  fixture.key(("Enter", "Enter"), (false, true));
  let applied = fixture.props.applied.lock().clone();
  assert_eq!(applied.len(), 1, "Ctrl Enter applies the command");
  assert_eq!(applied[0].text, "only the merges");
  let keys: Vec<&str> = applied[0].choices.iter().map(|choice| &*choice.key).collect();
  assert_eq!(keys, ["alpha"]);
  assert_eq!(*fixture.props.sent.lock(), 0, "nothing is sent as a message");
  assert!(!fixture.kit.exists(CHIP), "the chips go");
  assert_eq!(fixture.value(INPUT), "", "and the text");

  fixture.key(("Enter", "Enter"), (false, true));
  assert_eq!(
    *fixture.props.sent.lock(),
    1,
    "without a command, Ctrl Enter sends again"
  );
}
