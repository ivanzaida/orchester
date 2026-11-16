//! One seed request (`Request 1` `jD6Vy`): its number, its method on the `info_muted` fill and its URL, then its body
//! and what it captures as pills (`Pill · team_id ← $.id` `W0bxh`), the handle at its end. Pressed, it opens for
//! editing: the method, the URL, the body, each capture as a name and the place in the answer it is read, and moving
//! it up or down among the requests or removing it (the design draws only the closed request). Its fields type into
//! the page's requests at its place; leaving one saves.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, SelectOption, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::agent_testing::editor::form::{CaptureLine, METHODS, RequestLine, method_name};

use super::{
  super::{Commit, sections::Sections},
  capture::{CaptureProps, CaptureRow, READ_FROM},
};
use crate::{
  theme::{ControlHeight, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField, field_error},
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The method's select in an open request.
const METHOD_WIDTH: f32 = 104.0;
/// A method chip (`Method POST` `w5alke`).
const METHOD_HEIGHT: f32 = 20.0;
/// What an empty URL shows: a URL written with a placeholder, not words (`gGowW`).
const URL_EXAMPLE: &str = "{{url}}/api/";

#[derive(Clone)]
pub(super) struct RequestProps {
  pub at: usize,
  pub count: usize,
  pub line: RequestLine,
  pub open: bool,
  /// Why it makes no request, or what is wrong with a placeholder in it.
  pub said: Option<Arc<str>>,
  pub sections: Sections,
  pub commit: Commit,
}

/// Compared by what the row shows; the signals and the save are the page's.
impl PartialEq for RequestProps {
  fn eq(&self, other: &Self) -> bool {
    self.at == other.at
      && self.count == other.count
      && self.line == other.line
      && self.open == other.open
      && self.said == other.said
  }
}

impl DevtoolsInspectable for RequestProps {}

pub(super) struct RequestRow {
  method: Signal<String>,
  url: Signal<String>,
  body: Signal<String>,
}

impl Component for RequestRow {
  type Props = RequestProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<RequestProps>().clone();
    let row = Self {
      method: ctx.signal(method_name(props.line.method).to_owned()),
      url: ctx.signal(props.line.url.clone()),
      body: ctx.signal(props.line.body.clone()),
    };
    let (seeds, at, commit) = (props.sections.seeds.clone(), props.at, props.commit.clone());
    ctx.watch(&row.method, move |typed: &String| {
      let Some(method) = METHODS
        .iter()
        .find(|(_, name)| *name == typed.as_str())
        .map(|(method, _)| *method)
      else {
        return;
      };
      let mut changed = false;
      seeds.update(|seeds| {
        if let Some(line) = seeds.requests.get_mut(at).filter(|line| line.method != method) {
          line.method = method;
          changed = true;
        }
      });
      if changed {
        commit();
      }
    });
    for (part, body) in [(&row.url, false), (&row.body, true)] {
      let (seeds, at) = (props.sections.seeds.clone(), props.at);
      ctx.watch(part, move |typed: &String| {
        seeds.update(|seeds| {
          if let Some(line) = seeds.requests.get_mut(at) {
            let slot = if body { &mut line.body } else { &mut line.url };
            slot.clone_from(typed);
          }
        });
      });
    }
    row
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RequestProps>().clone();
    // The page's requests move only through this row's fields once it shows, but a fill (the page read again) or a
    // move puts what is stored at its place in them.
    for (signal, value) in [
      (&self.method, method_name(props.line.method).to_owned()),
      (&self.url, props.line.url.clone()),
      (&self.body, props.line.body.clone()),
    ] {
      if signal.get_untracked() != value {
        signal.set(value);
      }
    }
    let id = format!("settings-service-seed-request-{}", props.at);
    let shown = if props.open {
      self.editing(ctx, &props, &id)
    } else {
      closed(ctx, &props, &id)
    };
    Column::new()
      .id(id.clone())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        ExtraSpacing::Loose.role(),
        ExtraSpacing::Inline.role(),
      ))
      .spacing(ExtraSpacing::Snug.role())
      .child(shown)
      .with_children(
        props
          .said
          .map(|said| Element::from(field_error(&format!("{id}-error"), &said))),
      )
  }
}

/// Its number, in Geist Mono 11 `text_muted` (`MAani`).
fn number(at: usize) -> Text {
  Text::new(&(at + 1).to_string()).variant(ExtraTypography::MonoNote.role())
}

/// The closed request, as drawn; pressing it opens it.
fn closed(ctx: &Ctx, props: &RequestProps, id: &str) -> Element {
  let line = &props.line;
  let url = if line.url.trim().is_empty() {
    Text::new(&ctx.t("project_settings.services.editor.seed.noUrl"))
      .variant(ExtraTypography::PropertyMono.role())
      .color(PaletteColor::TextMuted)
  } else {
    Text::new(&line.url).variant(ExtraTypography::PropertyMono.role())
  };
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(method_chip(line))
    .child(url.id(format!("{id}-url")).flex(1.0).min_width(Dimension::Px(0.0)));
  let detail = (!line.body.trim().is_empty() || !line.captures.is_empty()).then(|| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&line.body)
          .id(format!("{id}-body"))
          .variant(ExtraTypography::MonoKey.role())
          .nowrap(),
      )
      .child(Spacer::new().flex(1.0))
      .with_children(
        line
          .captures
          .iter()
          .enumerate()
          .map(|(at, capture)| pill(id, at, capture)),
      )
  });
  let (open, at) = (props.sections.open_request.clone(), props.at);
  pressable(move || open.set(Some(at)))
    .id(format!("{id}-open"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .rounded(RadiusSize::Sm)
    .describe(
      "a11y_name",
      ctx.t_args(
        "project_settings.services.editor.seed.edit",
        [("number", (props.at + 1).to_string())],
      ),
    )
    .child(number(props.at))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Pair.role())
        .child(head)
        .with_children(detail),
    )
    .child(icon(Icon::GripVertical, IconSize::Base, PaletteColor::TextMuted))
    .into()
}

/// The method in Geist Mono 11/500 `info` on the `info_muted` fill (`Method POST` `w5alke`).
fn method_chip(line: &RequestLine) -> Row {
  Row::new()
    .height(Dimension::Px(METHOD_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
    .align_items(Alignment::Center)
    .background(PaletteColor::InfoMuted)
    .rounded(RadiusSize::Sm)
    .child(
      Text::new(method_name(line.method))
        .variant(ExtraTypography::MonoLabel.role())
        .color(PaletteColor::Info),
    )
}

/// What a request captures, as a pill: a mark, its name and where it is read (`W0bxh`).
fn pill(id: &str, at: usize, capture: &CaptureLine) -> Element {
  Row::new()
    .id(format!("{id}-capture-{at}"))
    .height(Dimension::Px(ControlHeight::Sm.value()))
    .padding(Padding::horizontal(ExtraSpacing::Inline.role()))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Full.role())
    .child(icon(Icon::CornerDownRight, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&format!("{} {READ_FROM} {}", capture.name, capture.path))
        .variant(ExtraTypography::MonoKey.role())
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .into()
}

impl RequestRow {
  /// The open request: the method and the URL, the body, the captures, and what can be done with it.
  fn editing(&self, ctx: &mut Ctx, props: &RequestProps, id: &str) -> Element {
    let leave = || {
      let commit = props.commit.clone();
      move || commit()
    };
    let methods: Vec<SelectOption<String>> = METHODS
      .iter()
      .map(|(_, name)| SelectOption::new((*name).to_owned(), (*name).to_owned()))
      .collect();
    let method = SelectField::new(
      format!("{id}-method"),
      ctx.t("project_settings.services.editor.seed.method"),
      self.method.clone(),
      methods,
    )
    .mono()
    .width(Dimension::Px(METHOD_WIDTH))
    .render(ctx);
    let url = FieldInput::new(
      format!("{id}-url"),
      ctx.t("project_settings.services.editor.seed.url"),
      self.url.clone(),
    )
    .mono()
    .placeholder(URL_EXAMPLE)
    .on_leave(leave())
    .render(ctx);
    let body = FieldInput::new(
      format!("{id}-body"),
      ctx.t("project_settings.services.editor.seed.body"),
      self.body.clone(),
    )
    .compact_text_area()
    .text_style(ExtraTypography::MonoArea.role())
    .placeholder(ctx.t("project_settings.services.editor.seed.bodyHint"))
    .on_leave(leave())
    .render(ctx);
    let head = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(number(props.at))
      .child(method)
      .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(url));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(head)
      .child(body)
      .with_children(captures(ctx, props, id))
      .child(actions(ctx, props, id))
      .into()
  }
}

/// Each capture's row.
fn captures(ctx: &mut Ctx, props: &RequestProps, id: &str) -> Vec<Element> {
  props
    .line
    .captures
    .iter()
    .enumerate()
    .map(|(at, line)| {
      ctx.mount_keyed::<CaptureRow>(
        &format!("{id}-capture-{at}"),
        CaptureProps {
          request: props.at,
          at,
          line: line.clone(),
          seeds: props.sections.seeds.clone(),
          commit: props.commit.clone(),
        },
      )
    })
    .collect()
}

/// What an open request's buttons do.
#[derive(Clone, Copy)]
enum Act {
  AddCapture,
  Up,
  Down,
  Remove,
  Done,
}

impl Act {
  /// Its button's id part, kind, words and glyph.
  const fn face(self) -> (&'static str, ButtonKind, &'static str, Icon) {
    match self {
      Self::AddCapture => (
        "add-capture",
        ButtonKind::Ghost,
        "project_settings.services.editor.seed.addCapture",
        Icon::Plus,
      ),
      Self::Up => (
        "up",
        ButtonKind::Ghost,
        "project_settings.services.editor.seed.moveUp",
        Icon::ArrowUp,
      ),
      Self::Down => (
        "down",
        ButtonKind::Ghost,
        "project_settings.services.editor.seed.moveDown",
        Icon::ArrowDown,
      ),
      Self::Remove => (
        "remove",
        ButtonKind::DangerGhost,
        "project_settings.services.editor.seed.remove",
        Icon::Trash2,
      ),
      Self::Done => (
        "done",
        ButtonKind::Ghost,
        "project_settings.services.editor.seed.done",
        Icon::Check,
      ),
    }
  }

  /// Does it to the request `props` shows: a move or a removal saves; a new capture saves once it is typed.
  fn run(self, props: &RequestProps) {
    let (seeds, open, at) = (&props.sections.seeds, &props.sections.open_request, props.at);
    match self {
      Self::AddCapture => seeds.update(|seeds| {
        if let Some(line) = seeds.requests.get_mut(at) {
          line.captures.push(CaptureLine::default());
        }
      }),
      Self::Up | Self::Down => {
        let to = if matches!(self, Self::Up) {
          at.checked_sub(1)
        } else {
          at.checked_add(1)
        };
        let Some(to) = to.filter(|to| *to < seeds.get_untracked().requests.len()) else {
          return;
        };
        if at >= seeds.get_untracked().requests.len() {
          return;
        }
        seeds.update(|seeds| seeds.requests.swap(at, to));
        open.set(Some(to));
        (props.commit)();
      }
      Self::Remove => {
        seeds.update(|seeds| {
          if at < seeds.requests.len() {
            seeds.requests.remove(at);
          }
        });
        open.set(None);
        (props.commit)();
      }
      Self::Done => open.set(None),
    }
  }
}

/// *Add capture*, then moving the request up or down, removing it and closing it.
fn actions(ctx: &Ctx, props: &RequestProps, id: &str) -> Element {
  let button = |act: Act| {
    let (name, kind, key, glyph) = act.face();
    let props = props.clone();
    Element::from(
      ActionButton::new(format!("{id}-{name}"), kind, ctx.t(key))
        .icon(glyph)
        .small()
        .on_click(move || act.run(&props)),
    )
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(button(Act::AddCapture))
    .child(Spacer::new().flex(1.0))
    .with_children((props.at > 0).then(|| button(Act::Up)))
    .with_children((props.at + 1 < props.count).then(|| button(Act::Down)))
    .child(button(Act::Remove))
    .child(button(Act::Done))
    .into()
}
