//! The palette itself (`Palette` `Tg8dQ` in `aDYAX`): 560 wide, `surface_raised` inside `border_strong`, radius
//! 10. Its field is 44 high, 14 at its sides, a 14 search glass 10 before what is typed in 13 (`Field` `F2jpx`);
//! the results, 6 inside and 2 apart, are grouped under 11/500 `text_muted` headings (`Group · Tasks` `Q1ZgAS`),
//! each 32 high with a 14 icon 10 before its words in 12.5 `text_primary`, the current one on `hover` with an
//! *Enter* key cap (`qFB9n`); the footer, 32 high, says how many results it shows and the keys (`Footer` `okuy2`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextInput},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::found::{Found, Group, Target, find};
use crate::{
  navigation::{Navigation, link::Open, press::OpenIn, shortcuts::Keys},
  project::Projects,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, ShellText},
  ui::{
    icon::{Icon, icon},
    kbd::key_cap,
    pressable::opening,
  },
};

/// The palette's width (`Tg8dQ`), its field's height (`F2jpx`), a result's (`qFB9n`) and the footer's (`okuy2`).
const WIDTH: f32 = 560.0;
const FIELD_HEIGHT: f32 = 44.0;
const RESULT_HEIGHT: f32 = 32.0;
const FOOTER_HEIGHT: f32 = 32.0;

/// What the palette keeps while it is open.
pub(super) struct State {
  pub query: Signal<String>,
  pub at: Signal<usize>,
  pub input: ElementRef,
  pub open: Signal<bool>,
}

/// The palette with the results for what is typed, in the active tab's project.
pub(super) fn palette(ctx: &mut Ctx, navigation: &Navigation, state: &State) -> Element {
  let project = navigation.tabs().active().sidebar_project();
  let tasks = project.map(|project| Projects::of(ctx).feed(project).tasks.get());
  let query = state.query.get();
  let found = find(ctx, project, tasks.as_ref().and_then(|tasks| tasks.ready()), &query);
  let current = state.at.get().min(found.len().saturating_sub(1));
  let opens: Vec<Open> = found
    .iter()
    .map(|found| opener(navigation, &state.open, &found.target))
    .collect();
  let mut results = Column::new()
    .id("jump-to-results")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Snug.role())
    .spacing(ExtraSpacing::Tight.role());
  for group in Group::ALL {
    let mut rows = found
      .iter()
      .enumerate()
      .filter(|(_, found)| found.group == group)
      .peekable();
    if rows.peek().is_none() {
      continue;
    }
    results = results.child(heading(ctx, group));
    for (at, found) in rows {
      results = results.child(result(ctx, found, at == current, opens[at].clone()));
    }
  }
  if found.is_empty() {
    results = results.child(nothing(ctx, &query));
  }
  let keys = move_keys(&state.at, found.len(), opens.get(current).cloned());
  Column::new()
    .id("jump-to")
    .width(Dimension::Px(WIDTH))
    .rounded(RadiusSize::Lg)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .box_shadow(ShadowStyle::Lg)
    .on_key_down(keys)
    .child(field(ctx, state))
    .child(results)
    .child(footer(ctx, found.len()))
    .into()
}

/// What opens `target` where a press or a key says, closing the palette first.
fn opener(navigation: &Navigation, open: &Signal<bool>, target: &Target) -> Open {
  let (navigation, open, target) = (navigation.clone(), open.clone(), target.clone());
  Arc::new(move |at: OpenIn| {
    open.set(false);
    match &target {
      Target::Route(route) => navigation.open(route.clone(), at),
      Target::NewTab => navigation.open_tab(),
    }
  })
}

/// ↑ and ↓ move among `count` results; Enter opens the current one, in a new tab with Ctrl (Cmd on macOS).
fn move_keys(at: &Signal<usize>, count: usize, chosen: Option<Open>) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  let at = at.clone();
  move |event: KeyboardEvent| match event.key.as_str() {
    "ArrowDown" if count > 0 => {
      at.update(|at| *at = (*at + 1).min(count - 1));
      event.prevent_default();
    }
    "ArrowUp" => {
      at.update(|at| *at = at.saturating_sub(1));
      event.prevent_default();
    }
    "Enter" => {
      if let Some(chosen) = &chosen {
        let new_tab = Keys::of(&event).only_command();
        chosen(if new_tab { OpenIn::NewTab } else { OpenIn::ActiveTab });
      }
      event.prevent_default();
    }
    _ => {}
  }
}

/// The field: a search glass and what is typed, a hairline under it.
fn field(ctx: &mut Ctx, state: &State) -> Row {
  let theme = ctx.theme();
  let style = |color| {
    FormTextRole {
      typography: TypographyStyle::Body,
      color,
    }
    .resolve(&theme.typography(), &theme.palette())
  };
  let placeholder = ctx.t("shell.jump.placeholder");
  let input = TextInput::styled(state.query.clone(), style(PaletteColor::TextPrimary))
    .placeholder_style(style(PaletteColor::TextMuted))
    .placeholder(&placeholder)
    .caret_color(PaletteColor::TextPrimary)
    .id("jump-to-field")
    .describe("a11y_name", placeholder.clone())
    .ref_element(state.input.clone())
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .single_line();
  let at = state.at.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FIELD_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    // What is typed anew starts from the first result.
    .on_key_down(move |event: KeyboardEvent| {
      if event.key.chars().count() == 1 || event.key == "Backspace" || event.key == "Delete" {
        at.set(0);
      }
    })
    .child(icon(Icon::Search, IconSize::Md, PaletteColor::TextMuted))
    .child(input)
}

/// A group's heading (`Group · Tasks` `Q1ZgAS`).
fn heading(ctx: &Ctx, group: Group) -> Element {
  Row::new()
    .id(format!("jump-to-group-{}", group.name()))
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::horizontal(SpacingSize::Sm)
        .top(SpacingSize::Sm)
        .bottom(SpacingSize::Xs),
    )
    .child(
      Text::new(&ctx.t(group.title_key()))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// A result: its icon and words; the current one on `hover` with the *Enter* cap.
fn result(ctx: &Ctx, found: &Found, current: bool, open: Open) -> Element {
  let row = opening(open)
    .id(format!("jump-to-{}-{}", found.group.name(), found.id))
    .describe("a11y_name", found.label.clone())
    .describe("state", if current { "current" } else { "enabled" })
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(RESULT_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .rounded(RadiusSize::Md)
    .align_items(Alignment::Center)
    .child(icon(found.icon, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&found.label)
        .variant(ShellText::JumpResult.role())
        .nowrap()
        .flex(1.0),
    )
    .with_children(current.then(|| key_cap("jump-to-enter", &ctx.t("shell.jump.enter"))));
  if current {
    row.background(ExtraColor::Hover.role()).into()
  } else {
    row
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .into()
  }
}

/// In place of the results when none answers what is typed.
fn nothing(ctx: &Ctx, query: &str) -> Element {
  Row::new()
    .id("jump-to-nothing")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Loose.role()))
    .child(
      Text::new(&ctx.t_args("shell.jump.none", [("query", query.trim())]))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .into()
}

/// How many results show, and the keys.
fn footer(ctx: &Ctx, count: usize) -> Element {
  let key = if count == 1 {
    "shell.jump.count.one"
  } else {
    "shell.jump.count.other"
  };
  let new_tab = if cfg!(target_os = "macos") {
    "shell.jump.hints.newTabMac"
  } else {
    "shell.jump.hints.newTab"
  };
  let hint = |key: &str| {
    Text::new(&ctx.t(key))
      .variant(ExtraTypography::MonoNote.role())
      .nowrap()
  };
  Row::new()
    .id("jump-to-footer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Inset.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&ctx.t_args(key, [("count", count.to_string())]))
        .id("jump-to-count")
        .variant(ShellText::JumpCount.role())
        .nowrap()
        .flex(1.0),
    )
    .child(hint("shell.jump.hints.move"))
    .child(hint("shell.jump.hints.open"))
    .child(hint(new_tab))
    .child(hint("shell.jump.hints.close"))
    .into()
}
