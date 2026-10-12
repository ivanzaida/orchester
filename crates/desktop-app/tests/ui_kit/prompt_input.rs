//! Prompt insertion uses the real retained input, its on_input path, focus and history.
use crate::harness::Kit;
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{FormTextRole, PaletteColor},
  },
  components::{Column, InputEditError, InputToken, InputTokens},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_app::{
  theme::instructions,
  ui::{
    button::{ActionButton, ButtonKind},
    prompt_input::{EditFeedback, PromptInput},
  },
};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone)]
struct Props {
  raw: Signal<String>,
  locked: Signal<bool>,
  mounted: Signal<bool>,
  masked: Signal<bool>,
  slot: Arc<Mutex<Option<PromptInput>>>,
  events: Arc<Mutex<Vec<String>>>,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.raw.id() == other.raw.id()
  }
}
impl DevtoolsInspectable for Props {}
struct Host {
  input: PromptInput,
}
impl Component for Host {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let input = PromptInput::new(ctx, props.raw);
    *props.slot.lock() = Some(input.clone());
    Self { input }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let raw = props.raw.get();
    let tokens = orchester_workspace::instructions::tokens::inline_references(&raw)
      .into_iter()
      .map(|reference| InputToken::new(reference.range, instructions::token_style(false)))
      .collect();
    let tokens = InputTokens::new(&raw, tokens).expect("valid domain ranges");
    let style = FormTextRole {
      typography: instructions::prompt_text(),
      color: PaletteColor::TextSecondary,
    }
    .resolve(&ctx.theme().typography(), &ctx.theme().palette());
    let locked = props.locked.get();
    let events = props.events.clone();
    let mut field = self
      .input
      .field(ctx, style, tokens)
      .id("editor")
      .width(Dimension::Px(300.0))
      .rows(1, 20)
      .on_input(move |event: lurq::app::events::TextInputEvent| {
        if locked {
          event.prevent_default();
        }
        events.lock().push(event.new_value);
      });
    if props.masked.get() {
      field = field.mask();
    }
    let input = self.input.clone();
    Column::new().with_children(props.mounted.get().then_some(field)).child(
      ActionButton::new("choose", ButtonKind::Secondary, "Insert variable")
        .on_click(move || input.report(input.insert("{{task.title}}"))),
    )
  }
}
struct Fixture {
  kit: Kit,
  props: Props,
  input: PromptInput,
}
impl Fixture {
  fn new(raw: &str) -> Self {
    let props = Props {
      raw: Signal::new(raw.into()),
      locked: Signal::new(false),
      mounted: Signal::new(true),
      masked: Signal::new(false),
      slot: Arc::default(),
      events: Arc::default(),
    };
    let shown = props.clone();
    let kit = Kit::show(move |ctx| ctx.mount_keyed::<Host>("prompt-fixture", shown.clone()));
    let input = props.slot.lock().clone().expect("mounted prompt input");
    Self { kit, props, input }
  }
  fn shortcut(&mut self, key: &str, code: &str, shift: bool) {
    self.kit.tree.key_down(key.into(), code.into(), shift, true, false);
    self.kit.pass();
  }
}
#[test]
fn prompt_input_retains_utf8_selection_after_picker_and_inserts_as_one_undo_step() {
  let mut fixture = Fixture::new("é😀 old tail");
  fixture.input.controller.select(7, 10).expect("UTF-8 byte selection");
  fixture.kit.pass();
  let selected = fixture.input.controller.selection().expect("mounted editor");
  fixture.kit.pass();
  assert_eq!(
    fixture.input.controller.selection().expect("retained selection"),
    selected
  );
  fixture.kit.click("choose");
  fixture.kit.pass();
  assert_eq!(fixture.props.raw.get(), "é😀 {{task.title}} tail");
  assert_eq!(fixture.props.events.lock().len(), 1);
  assert_eq!(
    fixture.kit.tree.focused_element().and_then(|element| element.id()),
    Some("editor")
  );
  fixture.shortcut("z", "KeyZ", false);
  assert_eq!(fixture.props.raw.get(), "é😀 old tail");
  fixture.shortcut("z", "KeyZ", true);
  assert_eq!(fixture.props.raw.get(), "é😀 {{task.title}} tail");
}
#[test]
fn prompt_input_uses_sdk_decoration_and_removes_stale_ranges_after_edits() {
  let mut fixture = Fixture::new("before {{task.title}} after");
  assert!(
    fixture
      .kit
      .tree
      .painted_quads()
      .iter()
      .any(|quad| matches!(quad.content, lurq::layout::quad::QuadContent::DecoratedInput { .. }))
  );
  fixture.props.raw.set("before {{task.titl}} after".into());
  fixture.kit.pass();
  assert!(
    !fixture
      .kit
      .tree
      .painted_quads()
      .iter()
      .any(|quad| matches!(quad.content, lurq::layout::quad::QuadContent::DecoratedInput { .. }))
  );
  assert_eq!(fixture.props.raw.get(), "before {{task.titl}} after");
}

#[test]
fn prompt_input_prevented_edits_and_unmounted_masked_invalid_selections_are_typed() {
  let mut fixture = Fixture::new("é tail");
  assert_eq!(
    fixture.input.controller.select(1, 1),
    Err(InputEditError::InvalidSelection)
  );
  fixture.input.controller.select(2, 2).expect("valid boundary");
  fixture.props.locked.set(true);
  fixture.kit.pass();
  fixture.kit.click("choose");
  assert_eq!(fixture.props.raw.get(), "é tail");
  assert_eq!(fixture.input.feedback.get(), EditFeedback::Prevented);
  fixture.props.locked.set(false);
  fixture.props.masked.set(true);
  fixture.kit.pass();
  assert_eq!(fixture.input.insert("x"), Err(InputEditError::Masked));
  fixture.props.mounted.set(false);
  fixture.kit.pass();
  assert_eq!(fixture.input.insert("x"), Err(InputEditError::NotMounted));
}
#[test]
fn prompt_input_reparses_crlf_edits_and_uses_current_source_after_external_reset() {
  let mut fixture = Fixture::new("é\r\n{{task.title}}\r\ntail\r\n");
  fixture.input.controller.select(2, 2).expect("before CRLF");
  fixture.kit.click("choose");
  fixture.kit.pass();
  assert_eq!(fixture.props.raw.get(), "é{{task.title}}\r\n{{task.title}}\r\ntail\r\n");
  fixture.props.raw.set("新\r\nreset\r\n".into());
  fixture.kit.pass();
  fixture
    .input
    .controller
    .select("新\r\n".len(), "新\r\n".len())
    .expect("reset byte position");
  fixture.kit.click("choose");
  fixture.kit.pass();
  assert_eq!(fixture.props.raw.get(), "新\r\n{{task.title}}reset\r\n");
}
