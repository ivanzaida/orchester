//! Skeletons: what stands in a part of a screen while its data is read (`Loading states · work screens` `xed4a`,
//! `Loading states · configure screens` `psFv5`, first built for Tasks in `05 · Loading` `R1nQlx` and `Reading ·
//! skeleton` `UWoGx`). A bone is a `border` block, radius 4, as large as the part it stands for and where it will
//! stand, so the content replaces it in place. A [`Skeleton`] shows its bones only after
//! [`SKELETON_DELAY`](crate::theme::SKELETON_DELAY), so a fast reading never flashes them, or under a page's
//! [`FromFirstFrame`] at once; each bone is lurq's `ui::Skeleton`, which pulse gently in one phase, in the theme's
//! steps (`theme::kit::skeleton`), unless the operating system asks for reduced motion, when they stay still, and
//! lower rows fade ([`fading`]). The screen's reading words stay its accessible name. lurq asks for a frame only when a
//! pulsing bone's pose changes. `rows` holds the shapes screens share: lines
//! of words, row groups and their rows.

mod rows;

use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
  ui::{Skeleton as UiSkeleton, SkeletonPart},
};

pub use self::rows::{
  BoneRow, Words, button, chevron, count, count_in_room, field, group, line, lines, list, outline, read_back, select,
  status, switch, unit_field,
};
use crate::theme::{SKELETON_DELAY, SKELETON_FADE};

/// A list's rows while they are read: each row's bone widths, its name, the line under it and what stands at its end.
pub type RowBones = &'static [(f32, f32, f32)];

/// A part of a screen while it is read: under `id`, named by the screen's reading words, its bones once the reading
/// took [`SKELETON_DELAY`].
pub struct Skeleton {
  id: Arc<str>,
  label: Arc<str>,
  room: Room,
  since: Option<Instant>,
}

/// The room a skeleton takes where it stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Room {
  /// Its column's width.
  Width,
  /// Its column's width and the room left in it.
  Fills,
  /// Its bones' own size: a value inside a loaded row.
  Inline,
}

impl Skeleton {
  pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      room: Room::Width,
      since: None,
    }
  }

  /// It takes the room left in its column, as the list it stands for does.
  pub const fn fills(mut self) -> Self {
    self.room = Room::Fills;
    self
  }

  /// It is as large as its bones: one value still read inside a row that is shown (`A value still reading in a loaded
  /// group` in `17 · Patterns` `d2yqa`).
  pub const fn inline(mut self) -> Self {
    self.room = Room::Inline;
    self
  }

  /// The reading began at `started`, before this skeleton was made: a skeleton that stands in for another one of the
  /// same reading, in another part of the screen, keeps its delay rather than starting it again.
  pub const fn since(mut self, started: Instant) -> Self {
    self.since = Some(started);
    self
  }

  /// The skeleton with `bones`, which show under `<id>-bones` once the reading took [`SKELETON_DELAY`].
  pub fn show(self, ctx: &mut Ctx, bones: impl Into<Element>) -> Element {
    let props = SkeletonProps {
      id: self.id,
      label: self.label,
      room: self.room,
      since: self.since,
    };
    ctx.mount_keyed_with::<Delayed>(&format!("skeleton-{}", props.id), props, vec![bones.into()])
  }
}

#[derive(Clone, Debug, PartialEq)]
struct SkeletonProps {
  id: Arc<str>,
  label: Arc<str>,
  room: Room,
  since: Option<Instant>,
}

impl DevtoolsInspectable for SkeletonProps {}

/// Provided by a page whose tabs switch in place, so the content under them never comes up empty: its skeletons show
/// their bones from the first frame, without [`SKELETON_DELAY`] (`Settings loading · redesign` `iDl7k`).
#[derive(Clone, Copy, Debug)]
pub struct FromFirstFrame;

struct Delayed {
  shown: Signal<bool>,
  /// Shows the bones once the reading took [`SKELETON_DELAY`]; none when it already had.
  _delay: Option<Timeout>,
}

impl Component for Delayed {
  type Props = SkeletonProps;

  fn create(ctx: &mut Ctx) -> Self {
    let since = ctx.props::<SkeletonProps>().since;
    let left = if ctx.use_context::<FromFirstFrame>().is_some() {
      Duration::ZERO
    } else {
      since.map_or(SKELETON_DELAY, |started| {
        SKELETON_DELAY.saturating_sub(started.elapsed())
      })
    };
    let shown = ctx.signal(left.is_zero());
    if left.is_zero() {
      return Self { shown, _delay: None };
    }
    let show = shown.clone();
    let delay = ctx.create_timeout(left, move || show.set(true));
    delay.start();
    Self {
      shown,
      _delay: Some(delay),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SkeletonProps>().clone();
    let shown = self.shown.get();
    let frame = Column::new()
      .id(props.id.to_string())
      .describe("a11y_name", props.label.clone())
      .describe("state", if shown { "skeleton" } else { "waiting" });
    let frame = match props.room {
      Room::Width => frame.width(Dimension::Pct(100.0)),
      Room::Fills => frame
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0)),
      Room::Inline => frame.flex_shrink(0.0),
    };
    if !shown {
      return frame;
    }
    let bones = Column::new()
      .id(format!("{}-bones", props.id))
      .with_children(ctx.children().iter().cloned());
    // Bones that fill take the frame's height, so panes in them fill it too and a transcript fits it.
    let bones = match props.room {
      Room::Inline => bones,
      Room::Width => bones.width(Dimension::Pct(100.0)),
      Room::Fills => bones
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0)),
    };
    frame.child(bones)
  }
}

/// A line of bones that can fade.
pub trait Fades {
  fn faded(self, opacity: f32) -> Self;
}

impl Fades for Row {
  fn faded(self, opacity: f32) -> Self {
    self.opacity(opacity)
  }
}

impl Fades for UiSkeleton {
  fn faded(self, opacity: f32) -> Self {
    self.style(SkeletonPart::Root, |style| style.opacity(opacity))
  }
}

impl Fades for Column {
  fn faded(self, opacity: f32) -> Self {
    self.opacity(opacity)
  }
}

/// `lines`, the lower ones fading away as [`SKELETON_FADE`] steps: all of them but the first, five at most.
pub fn fading<T: Fades>(lines: Vec<T>) -> Vec<T> {
  let fade = lines.len().saturating_sub(1).min(SKELETON_FADE.len());
  let start = lines.len() - fade;
  lines
    .into_iter()
    .enumerate()
    .map(|(index, line)| match index.checked_sub(start) {
      Some(step) => line.faded(SKELETON_FADE[step]),
      None => line,
    })
    .collect()
}

/// A lower group of a column whose groups read on their own, as faded as [`SKELETON_FADE`]'s `step` says.
pub fn lower_group(step: usize, group: impl Into<Element>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .opacity(SKELETON_FADE[step])
    .child(group)
}

/// `lines` over a composer, the upper ones fading away: older entries lie above them.
pub fn fading_up<T: Fades>(mut lines: Vec<T>) -> Vec<T> {
  lines.reverse();
  let mut faded = fading(lines);
  faded.reverse();
  faded
}

/// A bone `width` × `height`: lurq's `ui::Skeleton` in the theme's recipe (`theme::kit::skeleton`), pulsing in the
/// theme's steps.
pub fn bone(width: f32, height: f32) -> UiSkeleton {
  UiSkeleton::block(width, height).flex_shrink(0.0)
}

/// A round bone `side` across, for a status mark.
pub fn round_bone(side: f32) -> UiSkeleton {
  UiSkeleton::circle(side).flex_shrink(0.0)
}
