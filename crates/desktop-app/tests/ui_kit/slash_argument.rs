//! A slash command that takes an argument (ORC-771), through `ui::slash`'s public API with a picker of the test's own:
//! choosing it makes the command chip and opens its picker over the box; Enter adds the focused choice as a chip,
//! several allowed; Backspace in an empty field removes the last chip; Esc closes the picker and keeps the chips, and
//! the command chip opens it again; the words after the chips filter the picker and are the command's text; Ctrl Enter
//! applies the command to its choices and text instead of sending.

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

/// The picker of the test: three regions, their names filtered by what is typed; `gamma` cannot be chosen. A row says
/// `focused` and `taken` after its name while it is.
struct Regions;

impl ArgumentPicker for Regions {
  fn offer(&self, _ctx: &mut Ctx, asked: &Asked<'_>) -> Offer {
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
    let panel = Column::new()
      .id(PICKER)
      .width(Dimension::Px(300.0))
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
}

impl Component for Composer {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let (draft, focused, skill) = (ctx.signal(String::new()), ctx.signal(false), ctx.signal(None));
    let slash = SlashField::new(ctx, draft.clone(), (focused.clone(), skill));
    Self {
      draft,
      focused,
      slash,
      anchor: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let (gained, lost, slash) = (self.focused.clone(), self.focused.clone(), self.slash.clone());
    let sent = props.sent.clone();
    let input = TextInput::new(self.draft.clone())
      .id(INPUT)
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
        apply: Arc::new(move |done| applied.lock().push(done)),
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

  fn write(&mut self, text: &str) {
    self
      .kit
      .tree
      .get_element_by_id_mut(INPUT)
      .and_then(|element| element.as_text_input())
      .expect("the field")
      .set_value(text);
    self.kit.pass();
    self.kit.pass();
  }

  fn key(&mut self, (key, code): (&str, &str), ctrl: bool) {
    let tree = &mut self.kit.tree;
    tree.key_down(key.to_owned(), code.to_owned(), false, ctrl, false);
    tree.key_up(key.to_owned(), code.to_owned(), false, ctrl, false);
    self.kit.pass();
    self.kit.pass();
  }

  /// The words of the picker's row `key`, with whether it is focused or taken.
  fn picker_row(&mut self, key: &str) -> String {
    self.kit.texts(&format!("{PICKER}-{key}")).join(" ")
  }

  fn draft(&mut self) -> String {
    self
      .kit
      .tree
      .get_element_by_id_mut(INPUT)
      .and_then(|element| element.as_text_input())
      .expect("the field")
      .value()
  }

  /// `/watch` chosen in the slash menu with Enter.
  fn choose_command(&mut self) {
    self.write("/watch");
    assert!(self.kit.exists("test-composer-slash-watch-region"), "the command's row");
    self.key(("Enter", "Enter"), false);
  }
}

#[test]
fn slash_argument_becomes_a_chip_and_its_picker_adds_several_choices() {
  let mut fixture = Fixture::new();
  fixture.choose_command();
  assert!(fixture.kit.exists(CHIP), "the command chip");
  assert_eq!(fixture.kit.texts(&format!("{CHIP}-name")), ["/watch-region"]);
  assert_eq!(fixture.draft(), "", "the typed / goes");
  assert!(fixture.kit.exists(PICKER), "the picker opens");
  assert!(!fixture.kit.exists("test-composer-slash-menu"), "the slash menu closes");
  // Over the box, its bottom 8 over it.
  let (picker, boxed) = (fixture.kit.bounds(PICKER), fixture.kit.bounds("test-box"));
  assert!(
    (boxed.y - (picker.y + picker.height) - 8.0).abs() < 0.5,
    "{picker:?} {boxed:?}"
  );
  assert_eq!(fixture.picker_row("alpha"), "alpha focused");

  fixture.key(("Enter", "Enter"), false);
  assert!(fixture.kit.exists("test-composer-choice-alpha"), "alpha is a chip");
  assert_eq!(fixture.picker_row("alpha"), "alpha focused taken");
  fixture.key(("ArrowDown", "ArrowDown"), false);
  assert_eq!(fixture.picker_row("beta"), "beta focused");
  fixture.key(("ArrowDown", "ArrowDown"), false);
  assert_eq!(
    fixture.picker_row("alpha"),
    "alpha focused taken",
    "gamma cannot be chosen: the focus goes around it"
  );

  fixture.write("bet");
  assert!(
    !fixture.kit.exists(&format!("{PICKER}-alpha")),
    "the words filter the picker"
  );
  fixture.key(("Enter", "Enter"), false);
  assert!(fixture.kit.exists("test-composer-choice-beta"), "beta is a chip too");
  assert_eq!(fixture.draft(), "", "the filter empties");

  fixture.key(("Backspace", "Backspace"), false);
  assert!(
    !fixture.kit.exists("test-composer-choice-beta"),
    "Backspace removes the last chip"
  );
  assert!(fixture.kit.exists("test-composer-choice-alpha"));

  fixture.kit.click("test-composer-choice-alpha-remove");
  assert!(
    !fixture.kit.exists("test-composer-choice-alpha"),
    "its x removes a chip"
  );
  fixture.key(("Backspace", "Backspace"), false);
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
  fixture.key(("Enter", "Enter"), false);
  fixture.key(("Escape", "Escape"), false);
  assert!(!fixture.kit.exists(PICKER), "Esc closes the picker");
  assert!(fixture.kit.exists(CHIP) && fixture.kit.exists("test-composer-choice-alpha"));

  fixture.kit.click(CHIP);
  assert!(fixture.kit.exists(PICKER), "the command chip opens it again");
  assert_eq!(fixture.picker_row("alpha"), "alpha focused taken");
  fixture.key(("Escape", "Escape"), false);

  fixture.write("only the merges");
  fixture.key(("Enter", "Enter"), true);
  let applied = fixture.props.applied.lock().clone();
  assert_eq!(applied.len(), 1, "Ctrl Enter applies the command");
  assert_eq!(applied[0].text, "only the merges");
  let keys: Vec<&str> = applied[0].choices.iter().map(|choice| &*choice.key).collect();
  assert_eq!(keys, ["alpha"]);
  assert_eq!(*fixture.props.sent.lock(), 0, "nothing is sent as a message");
  assert!(!fixture.kit.exists(CHIP), "the chips go");
  assert_eq!(fixture.draft(), "", "and the text");

  fixture.key(("Enter", "Enter"), true);
  assert_eq!(
    *fixture.props.sent.lock(),
    1,
    "without a command, Ctrl Enter sends again"
  );
}
