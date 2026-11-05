//! The box pinned over a chat's composer while its agent works (`Plan · pinned` in 07 `Y6ouq`, 08 `lEQgI`):
//! `surface_panel` in a `border` outline, radius 10. Its 34 high head has `list-checks`, *Plan*, the count (done of
//! all), then who keeps the list and where at its end and a chevron; a press on the head folds the box to that head
//! with the item in progress (09 b `dq8jP`). Under it the items, 1 apart, 6 in from the box's sides and over its
//! bottom. A list past [`super::WHOLE`] items folds (07c `UfhkY`); opened, past [`SCROLLS_PAST`] rows it scrolls
//! inside the box, with *Show less* under it (09 c `tMJof`). The person's fold and opening are the box's own, kept
//! for as long as the chat keeps it mounted; its items follow each update in place.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::connection::stream::{TodoList, TodoState};

use super::{Shown, WHOLE, count, current, folded_line, item_row, shown};
use crate::{
  theme::{ExtraSpacing, IconSize, TodoText},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The head's height (`Head` `Y8rS8W`).
const HEAD_HEIGHT: f32 = 34.0;
/// Opened, more rows than this scroll inside the box.
pub const SCROLLS_PAST: usize = 8;
/// The height the rows of an opened long list scroll in (`Scroll` `Jzkq3`).
const SCROLL_HEIGHT: f32 = 246.0;

/// What the pinned box shows: `list`, kept by `owner` (*Builder · 03 Implement*, *Orchestrator · answering*); `id`
/// names it and its parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedProps {
  pub id: Arc<str>,
  pub list: TodoList,
  pub owner: Arc<str>,
}

impl DevtoolsInspectable for PinnedProps {}

pub struct PinnedPlan {
  /// The person folded the box to its head.
  folded: Signal<bool>,
  /// The person opened a long list whole.
  opened: Signal<bool>,
}

impl Component for PinnedPlan {
  type Props = PinnedProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      folded: ctx.signal(false),
      opened: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PinnedProps>().clone();
    let folded = self.folded.get();
    let head = self.head(ctx, &props, folded);
    let body = (!folded).then(|| self.items(ctx, &props));
    Column::new()
      .id(props.id.to_string())
      .describe("folded", folded.to_string())
      .width(Dimension::Pct(100.0))
      .background(PaletteColor::SurfacePanel)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(head)
      .with_children(body)
  }
}

impl PinnedPlan {
  /// The head; folded, it names the item in progress instead of who keeps the list.
  fn head(&self, ctx: &Ctx, props: &PinnedProps, folded: bool) -> Element {
    let id = &props.id;
    let toggle = self.folded.clone();
    let lead: [Element; 3] = [
      icon(Icon::ListChecks, IconSize::Base, PaletteColor::TextSecondary).into(),
      Text::new(&ctx.t("conversation.plan.title"))
        .variant(TodoText::Title.role())
        .nowrap()
        .into(),
      Text::new(&count(ctx, &props.list))
        .id(format!("{id}-count"))
        .variant(TodoText::Count.role())
        .nowrap()
        .into(),
    ];
    let end: Vec<Element> = if folded {
      let now = current(&props.list).map(|item| {
        let glyph = if item.state == TodoState::InProgress {
          Icon::LoaderCircle
        } else {
          Icon::Circle
        };
        Row::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(icon(glyph, IconSize::Base, PaletteColor::TextSecondary))
          .child(
            Text::new(&item.text)
              .id(format!("{id}-current"))
              .variant(TodoText::Item.role())
              .color(PaletteColor::TextPrimary)
              .nowrap()
              .text_overflow(TextOverflow::Elipsis)
              .flex(1.0)
              .min_width(Dimension::Px(0.0)),
          )
          .into()
      });
      now
        .into_iter()
        .chain(std::iter::once(chevron(Icon::ChevronUp)))
        .collect()
    } else {
      vec![
        Row::new().flex(1.0).into(),
        Text::new(&props.owner)
          .id(format!("{id}-owner"))
          .variant(TodoText::Owner.role())
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .min_width(Dimension::Px(0.0))
          .flex_shrink(1.0)
          .into(),
        chevron(Icon::ChevronDown),
      ]
    };
    pressable(move || toggle.update(|folded| *folded = !*folded))
      .id(format!("{id}-head"))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(HEAD_HEIGHT))
      .padding_horizontal(SpacingSize::Md)
      .spacing(SpacingSize::Sm)
      .with_children(lead)
      .with_children(end)
      .into()
  }

  /// The rows under the head, and *Show less* under an opened long list.
  fn items(&self, ctx: &Ctx, props: &PinnedProps) -> Element {
    let id = &props.id;
    let items = props.list.items();
    let opened = self.opened.get();
    let open = {
      let opened = self.opened.clone();
      Arc::new(move || opened.set(true)) as Arc<dyn Fn() + Send + Sync>
    };
    let rows: Vec<Element> = shown(items, opened)
      .into_iter()
      .map(|row| match row {
        Shown::Item(index) => item_row(format!("{id}-item-{index}"), &items[index], false),
        Shown::Done(done) => folded_line(
          format!("{id}-done"),
          (Icon::CircleCheck, PaletteColor::Success),
          &ctx.t_args("conversation.plan.doneFolded", [("count", done.to_string().as_str())]),
          (ctx.t("conversation.plan.show"), open.clone()),
        ),
        Shown::More(more) => folded_line(
          format!("{id}-more"),
          (Icon::Ellipsis, PaletteColor::TextMuted),
          &ctx.t_args("conversation.plan.more", [("count", more.to_string().as_str())]),
          (ctx.t("conversation.plan.showAll"), open.clone()),
        ),
      })
      .collect();
    let scrolls = rows.len() > SCROLLS_PAST;
    let rows = Column::new()
      .id(format!("{id}-items"))
      .width(Dimension::Pct(100.0))
      .padding_horizontal(ExtraSpacing::Snug.role())
      .spacing(ExtraSpacing::Hair.role())
      .with_children(rows);
    let rows: Element = if scrolls {
      ScrollVertical::new(rows)
        .id(format!("{id}-scroll"))
        .width(Dimension::Pct(100.0))
        .max_height(Dimension::Px(SCROLL_HEIGHT))
        .into()
    } else {
      rows.into()
    };
    let less = (opened && items.len() > WHOLE).then(|| {
      let opened = self.opened.clone();
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .top(SpacingSize::Sm)
            .bottom(SpacingSize::Xs)
            .left(SpacingSize::Md)
            .right(SpacingSize::Md),
        )
        .justify(Justify::End)
        .child(
          pressable(move || opened.set(false)).id(format!("{id}-less")).child(
            Text::new(&ctx.t("conversation.plan.showLess"))
              .variant(TodoText::Action.role())
              .nowrap(),
          ),
        )
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_bottom(ExtraSpacing::Snug.role())
      .child(rows)
      .with_children(less)
      .into()
  }
}

/// The head's chevron: down while the box is open (it folds down onto the composer), up while folded.
fn chevron(glyph: Icon) -> Element {
  icon(glyph, IconSize::Sm, PaletteColor::TextMuted).into()
}
