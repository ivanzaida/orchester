//! Skeletons: what stands in a part of a screen while its data is read (`Loading states · work screens` `xed4a`,
//! `Loading states · configure screens` `psFv5`, first built for Tasks in `05 · Loading` `R1nQlx` and `Reading ·
//! skeleton` `UWoGx`). A bone is a `border` block, radius 4, as large as the part it stands for and where it will
//! stand, so the content replaces it in place. A [`Skeleton`] shows its bones only after
//! [`SKELETON_DELAY`](crate::theme::SKELETON_DELAY), so a fast reading never flashes them; they pulse gently unless
//! the operating system asks for reduced motion (`appearance::motion`), when they stay still, and lower rows fade
//! ([`fading`]). The screen's reading words stay its accessible name. lurq plays the pulse on its animation clock and
//! asks for frames only while a pulsing skeleton is in the window's tree. `rows` holds the shapes screens share: lines
//! of words, row groups and their rows.

mod rows;

use std::sync::{Arc, OnceLock};

use lurq::{
  animation::{AnimatableProperty, Animation, Easing, Keyframes, KeyframesId, StepPosition},
  app::{
    Tree,
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{PaletteColor, RadiusSize},
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};

pub use self::rows::{
  BoneRow, Words, button, chevron, count, field, group, line, lines, list, outline, select, status, switch, unit_field,
};
use crate::theme::{ExtraRadius, SKELETON_DELAY, SKELETON_DIM, SKELETON_FADE, SKELETON_PULSE, SKELETON_STEPS};

/// A list's rows while they are read: each row's bone widths, its name, the line under it and what stands at its end.
pub type RowBones = &'static [(f32, f32, f32)];

/// A part of a screen while it is read: under `id`, named by the screen's reading words, its bones once the reading
/// took [`SKELETON_DELAY`].
pub struct Skeleton {
  id: Arc<str>,
  label: Arc<str>,
  room: Room,
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

  /// The skeleton with `bones`, which show under `<id>-bones` once the reading took [`SKELETON_DELAY`].
  pub fn show(self, ctx: &mut Ctx, bones: impl Into<Element>) -> Element {
    let props = SkeletonProps {
      id: self.id,
      label: self.label,
      room: self.room,
    };
    ctx.mount_keyed_with::<Delayed>(&format!("skeleton-{}", props.id), props, vec![bones.into()])
  }
}

#[derive(Clone, Debug, PartialEq)]
struct SkeletonProps {
  id: Arc<str>,
  label: Arc<str>,
  room: Room,
}

impl DevtoolsInspectable for SkeletonProps {}

struct Delayed {
  shown: Signal<bool>,
  /// Shows the bones once the reading took [`SKELETON_DELAY`].
  _delay: Timeout,
}

impl Component for Delayed {
  type Props = SkeletonProps;

  fn create(ctx: &mut Ctx) -> Self {
    let shown = ctx.signal(false);
    let show = shown.clone();
    let delay = ctx.create_timeout(SKELETON_DELAY, move || show.set(true));
    delay.start();
    Self { shown, _delay: delay }
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
      Room::Fills => frame.width(Dimension::Pct(100.0)).flex(1.0),
      Room::Inline => frame.flex_shrink(0.0),
    };
    if !shown {
      return frame;
    }
    let bones = Column::new()
      .id(format!("{}-bones", props.id))
      .with_children(ctx.children().iter().cloned());
    let bones = match props.room {
      Room::Inline => bones,
      Room::Width | Room::Fills => bones.width(Dimension::Pct(100.0)),
    };
    frame.child(pulsing(bones))
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

/// `lines` over a composer, the upper ones fading away: older entries lie above them.
pub fn fading_up<T: Fades>(mut lines: Vec<T>) -> Vec<T> {
  lines.reverse();
  let mut faded = fading(lines);
  faded.reverse();
  faded
}

/// The pulse's keyframes, registered once per window tree by [`install`]; the loader's turn is 1.
const PULSE: KeyframesId = KeyframesId::new(2);

/// Whether the operating system asks for reduced motion, once it was read when the app started; until then, and when
/// it cannot be read, skeletons pulse.
static STILL: OnceLock<bool> = OnceLock::new();

/// Registers the pulse with a window's tree.
pub fn install(tree: &mut Tree) {
  tree.register_keyframes_in_phase(
    Keyframes::new(PULSE)
      .frame(0.0, |frame| {
        frame.set(AnimatableProperty::Opacity, 1.0);
      })
      .frame(0.5, |frame| {
        frame.set(AnimatableProperty::Opacity, SKELETON_DIM);
      })
      .frame(1.0, |frame| {
        frame.set(AnimatableProperty::Opacity, 1.0);
      }),
  );
}

/// Keeps what the operating system said about reduced motion: `still` skeletons do not pulse. The first answer stays.
pub fn hold_still(still: bool) {
  STILL.get_or_init(|| still);
}

/// `skeleton`, pulsing unless the operating system asks for reduced motion.
pub fn pulsing(skeleton: Column) -> Column {
  if STILL.get().copied().unwrap_or(false) {
    return skeleton;
  }
  let steps = Easing::Steps {
    count: SKELETON_STEPS,
    position: StepPosition::JumpEnd,
  };
  skeleton.animation(Animation::new(PULSE).duration(SKELETON_PULSE).easing(steps).infinite())
}

/// A bone `width` × `height`.
pub fn bone(width: f32, height: f32) -> Column {
  Column::new()
    .width(Dimension::Px(width))
    .height(Dimension::Px(height))
    .flex_shrink(0.0)
    .rounded(RadiusSize::Sm)
    .background(PaletteColor::Border)
}

/// A round bone `side` across, for a status mark.
pub fn round_bone(side: f32) -> Column {
  bone(side, side).rounded(ExtraRadius::Full.role())
}
