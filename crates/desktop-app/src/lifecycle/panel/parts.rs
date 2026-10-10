//! The closing panel's parts besides its runs and actions: the heading with how long Orchester has waited,
//! the tasks that only wait for a person, a note line, the two choices, and the progress pill.

use std::time::Duration;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::close::{PersonWait, Stage, WaitingTask, waiting_for_person};

use crate::{
  navigation::Navigation,
  project::Projects,
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize, LifecycleText, StartupText},
  ui::{
    icon::{Icon, glyph, icon},
    loader,
  },
};

/// The icon tile beside the heading (`Icon tile` `H4Wnl`).
const TILE_SIZE: f32 = 40.0;

/// The icon tile, the heading and what it means; while the agents save, how long it has waited
/// (`Head` `d5mNQ`, `lxswN`, `b0VfYe`).
pub(super) fn head(ctx: &Ctx, stage: Stage, elapsed: Option<Duration>) -> Element {
  let (glyph_icon, color, title, description) = match stage {
    Stage::Confirm => (
      Icon::Power,
      PaletteColor::TextSecondary,
      "lifecycle.close.confirm.title",
      "lifecycle.close.confirm.description",
    ),
    Stage::Waiting => (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      "lifecycle.close.waiting.title",
      "lifecycle.close.waiting.description",
    ),
    Stage::Saved => (
      Icon::CircleCheck,
      PaletteColor::Success,
      "lifecycle.close.saved.title",
      "lifecycle.close.saved.description",
    ),
  };
  let tile_glyph = Text::new(&glyph(glyph_icon))
    .variant(StartupText::TileIcon.role())
    .color(color);
  let tile_glyph = if glyph_icon == Icon::LoaderCircle {
    loader::turning(tile_glyph)
  } else {
    tile_glyph
  };
  let tile = Row::new()
    .width(Dimension::Px(TILE_SIZE))
    .height(Dimension::Px(TILE_SIZE))
    .flex_shrink(0.0)
    .rounded(RadiusSize::Lg)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(tile_glyph);
  let elapsed = (stage == Stage::Waiting).then(|| elapsed_line(ctx, elapsed.unwrap_or_default()));
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(tile)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Snug.role())
        .child(
          Text::new(&ctx.t(title))
            .id("closing-title")
            .variant(TypographyStyle::Heading),
        )
        .child(
          Text::new(&ctx.t(description))
            .variant(TypographyStyle::Description)
            .width(Dimension::Pct(100.0)),
        )
        .with_children(elapsed),
    )
    .into()
}

/// *Waiting for 1 min 12 s* (`Elapsed` `loGeE`), from when Wait for agents to save was chosen.
fn elapsed_line(ctx: &Ctx, elapsed: Duration) -> Element {
  let seconds = elapsed.as_secs();
  let words = if seconds < 60 {
    ctx.t_args("lifecycle.close.elapsed.seconds", [("seconds", seconds.to_string())])
  } else {
    ctx.t_args(
      "lifecycle.close.elapsed.minutes",
      [
        ("minutes", (seconds / 60).to_string()),
        ("seconds", (seconds % 60).to_string()),
      ],
    )
  };
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Timer, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .id("closing-elapsed")
        .variant(ExtraTypography::Note.role()),
    )
    .into()
}

/// The tasks of the projects the tabs show that only wait for a person, named once (`Note` `QeO8y`).
pub(super) fn waiting_note(ctx: &mut Ctx) -> Option<Element> {
  let projects = Projects::of(ctx);
  let tabs = Navigation::of(ctx).tabs();
  let mut shown = Vec::new();
  let mut waiting: Vec<WaitingTask> = Vec::new();
  for project in tabs.tabs().iter().filter_map(|tab| tab.route().project()) {
    if shown.contains(&project)
      || projects
        .state(project)
        .and_then(|state| state.open().cloned())
        .is_none()
    {
      continue;
    }
    shown.push(project);
    let tasks = projects
      .feed(project)
      .tasks
      .with(|tasks| tasks.ready().map(waiting_for_person));
    waiting.extend(tasks.unwrap_or_default());
  }
  let words = match waiting.as_slice() {
    [] => return None,
    [one] => {
      let key = match one.wait {
        PersonWait::Plan => "lifecycle.close.waitingNote.plan",
        PersonWait::Answer => "lifecycle.close.waitingNote.answer",
        PersonWait::Decision => "lifecycle.close.waitingNote.decision",
      };
      ctx.t_args(key, [("task", &*one.key)])
    }
    several => {
      let separator = ctx.t("lifecycle.close.waitingNote.separator");
      let tasks: Vec<&str> = several.iter().map(|task| &*task.key).collect();
      ctx.t_args(
        "lifecycle.close.waitingNote.several",
        [("tasks", tasks.join(&separator))],
      )
    }
  };
  Some(note("closing-waiting-note", &words))
}

/// An information line under the rows (`Note` `QeO8y`, `gI4rP`).
pub(super) fn note(id: &str, words: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(icon(Icon::Info, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .id(id)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0),
    )
    .into()
}

/// What each choice does (`Choices` `sK6LZ`).
pub(super) fn choices(ctx: &Ctx) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Stretch)
    .child(choice(
      ctx,
      Icon::Save,
      "lifecycle.close.wait",
      "lifecycle.close.choice.waitDetail",
    ))
    .child(choice(
      ctx,
      Icon::Power,
      "lifecycle.close.quitNow",
      "lifecycle.close.choice.quitDetail",
    ))
    .into()
}

/// A choice's card (`Choice card` `zdSKi`).
fn choice(ctx: &Ctx, glyph_icon: Icon, title: &str, detail: &str) -> Column {
  Column::new()
    .flex(1.0)
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Snug.role())
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Lg)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(glyph_icon, IconSize::Nav, PaletteColor::TextSecondary))
        .child(Text::new(&ctx.t(title)).variant(LifecycleText::ChoiceTitle.role())),
    )
    .child(
      Text::new(&ctx.t(detail))
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    )
}

/// *Closes after 2 more runs*, *Closing Orchester…*: shaped like a button, but not one (`Progress pill`
/// `jRUlg`).
pub(super) fn progress(words: &str) -> Element {
  Row::new()
    .id("closing-progress")
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::LoaderCircle, IconSize::Md, PaletteColor::TextSecondary))
    .child(Text::new(words).variant(TypographyStyle::Button).nowrap())
    .into()
}
