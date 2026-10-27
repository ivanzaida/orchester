//! Quick add, in Backlog alone (`Quick add` `TyBnZ` in `06 Quick add` `ml1zC`): Backlog's head's `+` and its *New
//! task* foot open a field at the column's foot, `surface_input` in a `border_focus` stroke, radius 6, with the
//! title typed in 13/500 and under it *Enter add · Esc cancel* and that it makes a draft. Enter adds the draft, under a
//! key made once for it so a retry never adds a second one, and keeps the field open for the next; Esc closes it. The
//! new card shows once the tasks are read again. Once the person is no longer a member of the shared list, quick add is
//! locked and says why (`15 Not a member` `kMKKv`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::IdempotencyKey,
  name::{Name, NameError},
};
use orchester_workspace::{background, messages::Message, tasks::moves::write::quick_add_in};
use parking_lot::Mutex;

use super::actions::BoardState;
use crate::{
  project::ShownProject,
  services::Services,
  theme::{DISABLED_OPACITY, ExtraSpacing, IconSize},
  ui::{
    field::{FieldInput, field_error},
    icon::{Icon, icon},
    kbd::key_cap,
    message::message_text,
    pressable::{inert, pressable},
  },
};

/// The foot's height (`New task` `WwXjA`).
const FOOT_HEIGHT: f32 = 28.0;

#[derive(Clone, PartialEq)]
pub struct QuickAddProps {
  pub project: ShownProject,
  /// Why quick add is locked, once the person is no longer a member.
  pub locked: Option<Arc<str>>,
}

impl DevtoolsInspectable for QuickAddProps {}

/// What one Enter asks: the draft's key, made once for it, and its title.
type Adding = (IdempotencyKey, Name);

pub struct QuickAdd {
  title: Signal<String>,
  /// The key of the draft the field is typed for; a new one once it was added.
  batch: Arc<Mutex<IdempotencyKey>>,
  add: FutureAction<Adding, (), Message>,
  /// Why the title cannot be a draft's, until it is typed again.
  invalid: Signal<Option<Arc<str>>>,
  input: ElementRef,
  /// The field opened and wants the keys.
  focus: Signal<bool>,
}

impl Component for QuickAdd {
  type Props = QuickAddProps;

  fn create(ctx: &mut Ctx) -> Self {
    let project = ctx.props::<QuickAddProps>().project.clone();
    let title = ctx.signal(String::new());
    let batch = Arc::new(Mutex::new(IdempotencyKey::generate()));
    let services = Services::of(ctx);
    let (typed, next) = (title.clone(), batch.clone());
    let add = ctx.future_action(move |(key, name): Adding| {
      let (root, servers, feed) = (project.open.repo_root.clone(), services.servers.clone(), project.feed.clone());
      let (typed, next) = (typed.clone(), next.clone());
      async move {
        let added = background::run(move || quick_add_in(&root, &servers, key, name)).await?;
        added?;
        typed.set(String::new());
        *next.lock() = IdempotencyKey::generate();
        feed.saved();
        Ok(())
      }
    });
    Self {
      title,
      batch,
      add,
      invalid: ctx.signal(None),
      input: ElementRef::default(),
      focus: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<QuickAddProps>().clone();
    let open = BoardState::of(ctx).quick_open;
    if let Some(reason) = props.locked {
      return foot(ctx, Err(reason));
    }
    if !open.get() {
      let (open, focus) = (open.clone(), self.focus.clone());
      return foot(
        ctx,
        Ok(Arc::new(move || {
          open.set(true);
          focus.set(true);
        })),
      );
    }
    if self.focus.get() {
      ctx.focus(&self.input);
      self.focus.set(false);
    }
    self.field(ctx, &open).into()
  }
}

impl QuickAdd {
  /// The open field: the title, the keys that add and cancel, and why the last Enter did not add a draft.
  fn field(&self, ctx: &mut Ctx, open: &Signal<bool>) -> Column {
    let state = self.add.state().get();
    let pending = state.is_pending();
    let failed = state
      .error
      .filter(|_| !pending)
      .map(|error| message_text(ctx, &error))
      .or_else(|| self.invalid.get());
    let input = FieldInput::new("tasks-board-quick-add-title", ctx.t("tasks.board.quickAdd.label"), self.title.clone())
      .placeholder(ctx.t("tasks.board.quickAdd.placeholder"))
      .element(self.input.clone())
      .render(ctx);
    let keys = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .child(key_cap("tasks-board-quick-add-enter", &ctx.t("tasks.board.keys.enter")))
      .child(note(&ctx.t("tasks.board.quickAdd.add")))
      .child(key_cap("tasks-board-quick-add-esc", &ctx.t("tasks.board.keys.esc")))
      .child(note(&ctx.t("tasks.board.quickAdd.cancel")))
      .child(Spacer::new().flex(1.0))
      .child(note(&ctx.t("tasks.board.quickAdd.draft")));
    let (title, batch, add, invalid, close) = (
      self.title.clone(),
      self.batch.clone(),
      self.add.clone(),
      self.invalid.clone(),
      open.clone(),
    );
    let too_long = ctx.t_args("tasks.board.quickAdd.tooLong", [("count", Name::MAX_CHARS.to_string())]);
    Column::new()
      .id("tasks-board-quick-add")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .rounded(RadiusSize::Md)
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderFocus))
      .on_key_down(move |event: KeyboardEvent| match event.key.as_str() {
        "Enter" if event.text_input_focused => {
          event.prevent_default();
          if add.state().get_untracked().is_pending() {
            return;
          }
          // A blank title adds nothing and says nothing: the field stays as it is.
          match Name::new(&title.get_untracked()) {
            Ok(name) => {
              invalid.set(None);
              add.run((*batch.lock(), name));
            }
            Err(NameError::TooLong { .. }) => invalid.set(Some(too_long.clone())),
            Err(NameError::Blank) => {}
          }
        }
        "Escape" => {
          event.prevent_default();
          close.set(false);
        }
        _ => {}
      })
      .child(input)
      .child(keys)
      .with_children(failed.map(|reason| {
        field_error(
          "tasks-board-quick-add-error",
          &ctx.t_args("tasks.board.quickAdd.failed", [("reason", &*reason)]),
        )
      }))
  }
}

fn note(words: &str) -> Text {
  Text::new(words)
    .variant(crate::theme::ExtraTypography::Note.role())
    .nowrap()
}

/// *New task* at Backlog's foot, which opens the field; faded and pressed in vain while quick add is locked, its tooltip
/// saying why.
fn foot(ctx: &Ctx, open: Result<Arc<dyn Fn() + Send + Sync>, Arc<str>>) -> Element {
  let content = |row: Row| {
    row
      .id("tasks-board-new-task")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FOOT_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .child(icon(Icon::Plus, IconSize::Base, PaletteColor::TextMuted))
      .child(
        Text::new(&ctx.t("tasks.newTask"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
  };
  match open {
    Ok(open) => pressable(move || open())
      .width(Dimension::Pct(100.0))
      .hovered_style(lurq::node::Style::new().background(crate::theme::ExtraColor::Hover.role()))
      .rounded(RadiusSize::Md)
      .child(content(Row::new()))
      .into(),
    Err(reason) => inert(lurq::components::Button::empty())
      .width(Dimension::Pct(100.0))
      .describe("state", "unavailable")
      .describe("reason", reason)
      .opacity(DISABLED_OPACITY)
      .child(content(Row::new()))
      .into(),
  }
}
