//! *Add source* (`t4r78`): a 28 high box with a 13 `folder-search` glyph for a typed or pasted path, *Choose
//! folder…* (the system's folder picker, whose choice fills the box) and *Add source*, 8 apart, over a hint 6 under
//! them: a 12 glyph and 11.5/1.4 words, 6 apart. The path is checked in place as it changes
//! (`orchester_workspace::sources::check`), and the hint says what was found (`Sources · states` `lOOY6`): a
//! readable folder or file (`circle-check` in `success`), a path that is not needed (`info`), or why it is
//! refused, in `danger` with the box's stroke; *Add source* is pressed only for a readable one. In an inspector
//! (`Add` `sbqLR`) the box runs over the buttons, *Add* at the end; Pipeline settings has only *Add folder*
//! (`m90ZI`): a chosen folder that passes the check is added, and the hint says why one is refused.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::source::SourcePath;
use orchester_workspace::{
  background,
  messages::Message,
  sources::check::{Candidate, Place, Refusal},
};
use parking_lot::Mutex;

use super::{
  button::{ActionButton, ButtonKind},
  field::FieldInput,
  icon::{Icon, icon},
  message::message_text,
};
use crate::{
  services::path_picker::{PathKind, PathPicker},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
};

/// How the control is laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
  /// The box and both buttons on one line, the hint under them (a page's list).
  Line,
  /// The box over the buttons (an inspector).
  Stacked,
  /// Only *Add folder*.
  Picker,
}

type OnAdd = Arc<dyn Fn(SourcePath) + Send + Sync>;

/// A source that passed the check, which the person is asked about before it is added. A signal's value needs
/// `DevtoolsInspectable`, which the app cannot give core's `SourcePath` itself (orphan rule).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked(pub SourcePath);

impl DevtoolsInspectable for Asked {}

#[derive(Clone)]
pub struct AddSourceProps {
  /// The element ids start with it: `<id>-path`, `<id>-choose`, `<id>-add`, `<id>-hint`.
  pub id: Arc<str>,
  /// Where a path is checked; `None` while the sources it would join are read.
  pub place: Option<Place>,
  pub layout: Layout,
  /// *Add source*, *Add* or *Add folder*.
  pub add: Arc<str>,
  /// Nothing can be added now: a question or a save is under way.
  pub locked: bool,
  /// Takes a path that passed the check.
  pub on_add: OnAdd,
}

/// The props are equal when they show the same: the handler is made anew on every render.
impl PartialEq for AddSourceProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.id, &self.place, self.layout, &self.add, self.locked)
      == (&other.id, &other.place, other.layout, &other.add, other.locked)
  }
}

impl DevtoolsInspectable for AddSourceProps {}

/// What the check said of a path.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Checked(String, Result<Candidate, Refusal>);

impl DevtoolsInspectable for Checked {}

/// The folder the picker gave, and, for *Add folder*, what the check said of it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Picked {
  path: Option<String>,
  checked: Option<Checked>,
}

impl DevtoolsInspectable for Picked {}

type PickAction = FutureAction<(Arc<str>, Option<Place>), Picked, Message>;

pub struct AddSource {
  text: Signal<String>,
  pick: PickAction,
  /// Why the folder *Add folder* chose last was refused, until another is chosen.
  refused: Signal<Option<Checked>>,
  /// The handler the props gave last, which takes a chosen folder that passed; `None` while locked.
  on_add: Arc<Mutex<Option<OnAdd>>>,
}

impl Component for AddSource {
  type Props = AddSourceProps;

  fn create(ctx: &mut Ctx) -> Self {
    let (text, refused) = (ctx.signal(String::new()), ctx.signal(None));
    let on_add: Arc<Mutex<Option<OnAdd>>> = Arc::default();
    let pick = pick_action(ctx);
    let (filled, shown, taker) = (text.clone(), refused.clone(), on_add.clone());
    ctx.watch(&pick.state(), move |state| {
      let Some(picked) = state.fulfilled_data() else {
        return;
      };
      match (&picked.path, &picked.checked) {
        (Some(path), None) => filled.set(path.clone()),
        (_, Some(Checked(_, Ok(candidate)))) => {
          shown.set(None);
          if let Some(add) = taker.lock().clone() {
            add(candidate.path.clone());
          }
        }
        (_, Some(checked)) => shown.set(Some(checked.clone())),
        (None, None) => {}
      }
    });
    Self {
      text,
      pick,
      refused,
      on_add,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AddSourceProps>().clone();
    *self.on_add.lock() = (!props.locked).then(|| props.on_add.clone());
    let typed = self.text.get().trim().to_owned();
    let checked = self.check(ctx, &props, &typed);
    let choose = self.choose_button(ctx, &props);
    if props.layout == Layout::Picker {
      let refused = self.refused.get();
      return Column::new()
        .id(props.id.to_string())
        .spacing(ExtraSpacing::Snug.role())
        .child(choose)
        .with_children(refused.map(|checked| hint(ctx, &props.id, Some(&checked.1), props.layout)));
    }
    let ready = checked.as_ref().and_then(|checked| checked.1.as_ref().ok()).cloned();
    let refused = checked
      .as_ref()
      .and_then(|checked| checked.1.as_ref().err())
      .is_some_and(Refusal::is_error);
    let add = self.add_button(&props, ready.filter(|_| !props.locked));
    let field = self.field(ctx, &props, refused);
    let hint = hint(ctx, &props.id, checked.as_ref().map(|checked| &checked.1), props.layout);
    let column = Column::new().id(props.id.to_string()).width(Dimension::Pct(100.0));
    if props.layout == Layout::Stacked {
      return column
        .spacing(SpacingSize::Sm)
        .child(field)
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .align_items(Alignment::Center)
            .child(choose)
            .child(Spacer::new().flex(1.0))
            .child(add),
        )
        .with_children(checked.is_some().then_some(hint));
    }
    column
      .spacing(ExtraSpacing::Snug.role())
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(field))
          .child(choose)
          .child(add),
      )
      .child(hint)
  }
}

/// Opens the system's folder picker; for *Add folder* (a place given) the chosen folder is checked there too.
fn pick_action(ctx: &mut Ctx) -> PickAction {
  let picker = PathPicker::of(ctx);
  ctx.future_action(move |(title, place): (Arc<str>, Option<Place>)| {
    let picker = picker.clone();
    async move {
      let Some(path) = picker.pick(PathKind::Folder, title).await else {
        return Ok(Picked {
          path: None,
          checked: None,
        });
      };
      let path = path.to_string_lossy().into_owned();
      let checked = match place {
        Some(place) => {
          let typed = path.clone();
          Some(background::run(move || Checked(typed.clone(), place.check(&typed))).await?)
        }
        None => None,
      };
      Ok(Picked {
        path: Some(path),
        checked,
      })
    }
  })
}

impl AddSource {
  /// What the check said of `typed`, once it ended; `None` while nothing is typed or it runs.
  fn check(&self, ctx: &mut Ctx, props: &AddSourceProps, typed: &str) -> Option<Checked> {
    let deps = (props.place.clone(), typed.to_owned());
    let state = ctx
      .future(deps, |(place, typed)| async move {
        let Some(place) = place.filter(|_| !typed.is_empty()) else {
          return Ok(None);
        };
        background::run(move || Some(Checked(typed.clone(), place.check(&typed)))).await
      })
      .state()
      .get();
    state.data.flatten().filter(|checked| checked.0 == typed)
  }

  fn field(&self, ctx: &mut Ctx, props: &AddSourceProps, refused: bool) -> Element {
    let field = FieldInput::new(
      format!("{}-path", props.id),
      ctx.t("ui.addSource.path"),
      self.text.clone(),
    )
    .placeholder(ctx.t("ui.addSource.path"))
    .leading(Icon::FolderSearch)
    .mono();
    let field = if refused { field.invalid() } else { field };
    let field = if props.locked { field.locked() } else { field };
    field.render(ctx)
  }

  fn choose_button(&self, ctx: &Ctx, props: &AddSourceProps) -> ActionButton {
    let title = ctx.t("ui.addSource.pickerTitle");
    let (id, label, glyph, place) = match props.layout {
      Layout::Picker => (
        format!("{}-add", props.id),
        props.add.clone(),
        Icon::FolderPlus,
        props.place.clone(),
      ),
      Layout::Line | Layout::Stacked => (
        format!("{}-choose", props.id),
        ctx.t("ui.addSource.choose"),
        Icon::FolderOpen,
        None,
      ),
    };
    let button = ActionButton::new(id, ButtonKind::Secondary, label).icon(glyph);
    let waiting = props.layout == Layout::Picker && place.is_none();
    if props.locked || waiting || self.pick.state().get().is_pending() {
      return button.disabled();
    }
    let pick = self.pick.clone();
    button.on_click(move || pick.run((title.clone(), place.clone())))
  }

  fn add_button(&self, props: &AddSourceProps, ready: Option<Candidate>) -> ActionButton {
    let button =
      ActionButton::new(format!("{}-add", props.id), ButtonKind::Secondary, props.add.clone()).icon(Icon::Plus);
    let Some(candidate) = ready else {
      return button.disabled();
    };
    let (text, on_add) = (self.text.clone(), props.on_add.clone());
    button.on_click(move || {
      text.set(String::new());
      on_add(candidate.path.clone());
    })
  }
}

/// What the hint says: how to add a source, what the check found, or why it refuses the path.
fn hint(ctx: &Ctx, id: &str, checked: Option<&Result<Candidate, Refusal>>, layout: Layout) -> Element {
  let (glyph, mark, words, color, state) = match checked {
    None => (
      Icon::Info,
      PaletteColor::TextMuted,
      ctx.t("ui.addSource.hint"),
      PaletteColor::TextMuted,
      "idle",
    ),
    Some(Ok(candidate)) => (
      Icon::CircleCheck,
      PaletteColor::Success,
      message_text(ctx, &candidate.message()),
      PaletteColor::TextMuted,
      "readable",
    ),
    Some(Err(refusal)) if refusal.is_error() => (
      if *refusal == Refusal::Unreadable {
        Icon::Ban
      } else {
        Icon::CircleAlert
      },
      PaletteColor::Danger,
      message_text(ctx, &refusal.message()),
      PaletteColor::Danger,
      "refused",
    ),
    Some(Err(refusal)) => (
      Icon::Info,
      PaletteColor::TextSecondary,
      message_text(ctx, &refusal.message()),
      PaletteColor::TextMuted,
      "not_needed",
    ),
  };
  let row = Row::new()
    .id(format!("{id}-hint"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .describe("state", state)
    .child(icon(glyph, IconSize::Sm, mark))
    .child(
      Text::new(&words)
        .id(format!("{id}-hint-text"))
        .variant(ExtraTypography::Note.role())
        .color(color)
        .flex(1.0),
    );
  if layout == Layout::Picker {
    return row.into();
  }
  row.width(Dimension::Pct(100.0)).into()
}
