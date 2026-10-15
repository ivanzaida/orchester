//! The file (`File` `ZNjnP`, on `code_bg`): its head (`File head` `eBH0g`, 40 high, 16 from its start and 10
//! from its end, a `border` line under it) with its path in Geist Mono 12.5/500 and, at its end, the lines
//! selected and *Find* with its key cap, or the search within the file while it is open; under it the
//! lines, virtualised, so a long file opens at once. The lines show plain at once and highlighted when a
//! background thread finished them (the Changes tab's highlighting). The selected lines and the current
//! place found scroll into view. Ctrl+C (Cmd+C) copies the selected lines, a person's press only. A binary
//! or too large file shows its head and why it is not shown; what stands for a file otherwise is in
//! [`super::states`].

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text, VirtualizedList},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  files::{FilePath, FileText, FileView, LineRange, TextMatch},
  highlight::{LineTokens, plain},
};
use parking_lot::Mutex;

use super::{
  Page, Refusal, actions,
  find::FindState,
  keys,
  line::{CODE_CHAR_WIDTH, CodeLine, Found, GAP_WIDTH, LineProps, LineView, NUMBER_WIDTH, Select},
  reads::{self, Shown},
  states,
};
use crate::{
  navigation::shortcuts::Keys,
  theme::{CodeText, ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::Copier,
    icon::Icon,
    kbd::key_cap,
    state::loading,
  },
};

/// The head's height (`eBH0g`); width changes below the epsilon are layout noise, not a resize.
const HEAD_HEIGHT: f32 = 40.0;
const WIDTH_EPSILON: f32 = 0.5;

/// The file's pane, or what stands for it.
pub fn pane(ctx: &mut Ctx, page: &Page<'_>) -> Element {
  let content = match (&page.path, &page.read) {
    (None, _) => states::no_file(ctx, page),
    (Some(Err(error)), _) => states::refused(ctx, page, error),
    (Some(Ok(_)), None | Some(Shown::Reading)) => loading("files-file-loading", &ctx.t("files.readingFile")).into(),
    (Some(Ok(_)), Some(Shown::Failed(failure))) => states::unreadable(ctx, failure),
    (Some(Ok(file)), Some(Shown::Read(read))) => match read {
      FileView::Text(text) => text_pane(ctx, page, file, text),
      FileView::Binary { size } => column(file, &states::size_note(ctx, "files.meta.binary", *size))
        .child(states::binary(ctx, file))
        .into(),
      FileView::TooLarge { size } => column(file, &states::size_note(ctx, "files.meta.text", *size))
        .child(states::too_large(ctx, file, *size))
        .into(),
    },
  };
  Column::new()
    .id("files-file")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .min_width(Dimension::Px(0.0))
    .background(ExtraColor::CodeBackground.role())
    .child(content)
    .into()
}

/// The pane of a file that is not shown: its head with `note` at its end.
fn column(file: &FilePath, note: &str) -> Column {
  let head = head_row(file).child(Row::new().flex(1.0)).child(
    Text::new(note)
      .id("files-file-note")
      .variant(ExtraTypography::Note.role())
      .nowrap(),
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(head)
}

/// The head's start: the file's path.
fn head_row(file: &FilePath) -> Row {
  Row::new()
    .id("files-file-head")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::new().left(SpacingSize::Lg).right(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(file.as_str())
        .id("files-file-path")
        .variant(CodeText::DiffPath.role())
        .nowrap(),
    )
}

fn text_pane(ctx: &mut Ctx, page: &Page<'_>, file: &FilePath, text: &Arc<FileText>) -> Element {
  let tokens = reads::tokens(ctx, file, text);
  let clipboard = Copier::of(ctx);
  let select = selector(ctx, page);
  ctx.mount_keyed::<CodePane>(
    "files-code",
    CodeProps {
      file: file.clone(),
      text: text.clone(),
      tokens,
      lines: page.route.line(),
      find: page.view.find.clone(),
      select,
      refusal: page.view.refusal.clone(),
      clipboard,
    },
  )
}

/// Selects a line, or with `extend` the lines from the selection's first to it; a press on the only line
/// selected clears the selection.
fn selector(ctx: &mut Ctx, page: &Page<'_>) -> Select {
  let select = page.select_lines(ctx);
  let current = page.route.line();
  Arc::new(move |index: usize, extend: bool| {
    let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
    let lines = match current {
      Some(range) if extend => LineRange::new(range.first(), number),
      Some(range) if range.first() == number && range.last() == number => None,
      _ => LineRange::new(number, number),
    };
    select(lines);
  })
}

#[derive(Clone)]
struct CodeProps {
  file: FilePath,
  text: Arc<FileText>,
  tokens: Option<Arc<Vec<LineTokens>>>,
  lines: Option<LineRange>,
  find: FindState,
  select: Select,
  refusal: Signal<Option<Refusal>>,
  clipboard: Copier,
}

impl PartialEq for CodeProps {
  fn eq(&self, other: &Self) -> bool {
    self.file == other.file
      && Arc::ptr_eq(&self.text, &other.text)
      && match (&self.tokens, &other.tokens) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
      }
      && self.lines == other.lines
  }
}

impl DevtoolsInspectable for CodeProps {}

/// What the search found, for the text and query it looked in and for.
type FoundFor = ((usize, Option<(String, bool)>), Arc<Vec<TextMatch>>);
/// The lines, for what they were made of.
type LinesFor = (
  (usize, usize, Option<LineRange>, usize, Option<usize>),
  Arc<Vec<CodeLine>>,
);

struct CodePane {
  view: ElementRef,
  width: Signal<f32>,
  found: Mutex<Option<FoundFor>>,
  lines: Mutex<Option<LinesFor>>,
  plain: Mutex<Option<(usize, Plain)>>,
}

/// A text's lines as plain tokens, and the width its longest line needs.
#[derive(Clone)]
struct Plain {
  tokens: Arc<Vec<LineTokens>>,
  width: f32,
}

impl Component for CodePane {
  type Props = CodeProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      view: ElementRef::new(),
      width: ctx.signal(0.0),
      found: Mutex::new(None),
      lines: Mutex::new(None),
      plain: Mutex::new(None),
    }
  }

  /// Keeps the pane's width after a layout that changed it, e.g. a resized window.
  fn after_layout(&self) {
    if !self.view.is_attached() {
      return;
    }
    let width = self.view.width();
    if (width - self.width.get_untracked()).abs() > WIDTH_EPSILON {
      self.width.set(width);
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CodeProps>().clone();
    let found = self.found(&props);
    let current = props.find.current(found.len());
    let lines = self.lines(&props, &found, current);
    let reveal = current
      .and_then(|at| found.get(at))
      .map(|place| place.line)
      .or_else(|| props.lines.map(LineRange::first_index));
    let width = self.plain(&props.text).width.max(self.width.get());
    let select = props.select.clone();
    let list: Element = VirtualizedList::new_shared(ctx, lines)
      .list_key(format!("files-lines-{}", props.file))
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .horizontal_scroll(true)
      .reveal_key(reveal.map(|line| line.to_string()))
      .mount_keyed::<LineView, _, _, _>(
        |line: &CodeLine| line.index,
        move |line| LineProps {
          line: line.clone(),
          width,
          select: select.clone(),
        },
      );
    let head = self.head(ctx, &props, found.len());
    Column::new()
      .id("files-code")
      .ref_element(self.view.clone())
      .describe("highlighted", props.tokens.is_some().to_string())
      .describe("lines", props.text.lines().len().to_string())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .on_key_down(copy_keys(&props))
      .child(head)
      .child(list)
  }
}

impl CodePane {
  /// The places the search found, worked out once per text and query; a new query starts at its first.
  fn found(&self, props: &CodeProps) -> Arc<Vec<TextMatch>> {
    let key = (Arc::as_ptr(&props.text) as usize, props.find.looking_for());
    let mut kept = self.found.lock();
    if let Some((made_for, found)) = kept.as_ref()
      && *made_for == key
    {
      return found.clone();
    }
    let found = key
      .1
      .as_ref()
      .map_or_else(|| Arc::new(Vec::new()), |query| FindState::found(&props.text, query));
    if kept.is_some() {
      props.find.at.set(0);
    }
    *kept = Some((key, found.clone()));
    found
  }

  /// The lines to show, made again only when their text, colors, selection or places found change.
  fn lines(&self, props: &CodeProps, found: &Arc<Vec<TextMatch>>, current: Option<usize>) -> Arc<Vec<CodeLine>> {
    let text_key = Arc::as_ptr(&props.text) as usize;
    let tokens_key = props.tokens.as_ref().map_or(0, |tokens| Arc::as_ptr(tokens) as usize);
    let key = (text_key, tokens_key, props.lines, Arc::as_ptr(found) as usize, current);
    let mut kept = self.lines.lock();
    if let Some((made_for, lines)) = kept.as_ref()
      && *made_for == key
    {
      return lines.clone();
    }
    let tokens = match &props.tokens {
      Some(tokens) => tokens.clone(),
      None => self.plain(&props.text).tokens,
    };
    let mut places: Vec<Vec<Found>> = vec![Vec::new(); props.text.lines().len()];
    for (at, place) in found.iter().enumerate() {
      if let Some(line) = places.get_mut(place.line) {
        line.push(Found {
          start: place.start,
          end: place.end,
          current: Some(at) == current,
        });
      }
    }
    let lines: Arc<Vec<CodeLine>> = Arc::new(
      places
        .into_iter()
        .enumerate()
        .map(|(index, found)| CodeLine {
          index,
          tokens: tokens.get(index).cloned().unwrap_or_else(|| plain("")),
          selected: props.lines.is_some_and(|range| range.contains_index(index)),
          found: found.into(),
        })
        .collect(),
    );
    *kept = Some((key, lines.clone()));
    lines
  }

  /// The lines as plain text, shown before or without highlighting, and their width; worked out once per
  /// text.
  fn plain(&self, text: &Arc<FileText>) -> Plain {
    let key = Arc::as_ptr(text) as usize;
    let mut kept = self.plain.lock();
    if let Some((made_for, plain_lines)) = kept.as_ref()
      && *made_for == key
    {
      return plain_lines.clone();
    }
    let tokens: Vec<LineTokens> = text.lines().iter().map(|line| plain(line)).collect();
    let longest = tokens
      .iter()
      .map(|line| line.iter().map(|token| token.text.chars().count()).sum::<usize>())
      .max()
      .unwrap_or(0);
    let made = Plain {
      tokens: Arc::new(tokens),
      width: content_width(longest),
    };
    *kept = Some((key, made.clone()));
    made
  }

  /// The path, and at the head's end the lines selected and *Find*, or the search while it is open.
  fn head(&self, ctx: &mut Ctx, props: &CodeProps, found: usize) -> Element {
    let head = head_row(&props.file).child(Row::new().flex(1.0));
    if props.find.open.get() {
      return head.child(props.find.bar(ctx, found)).into();
    }
    let selection = props.lines.map(|range| {
      let first = range.first().to_string();
      let text = if range.first() == range.last() {
        ctx.t_args("files.selection.one", [("first", first.as_str())])
      } else {
        let last = range.last().to_string();
        ctx.t_args(
          "files.selection.range",
          [("first", first.as_str()), ("last", last.as_str())],
        )
      };
      Text::new(&text)
        .id("files-selection")
        .variant(ExtraTypography::Note.role())
        .nowrap()
    });
    let find = props.find.clone();
    let (_, caps) = keys::caps(ctx);
    head
      .with_children(selection)
      .child(
        ActionButton::new("files-find-open", ButtonKind::Ghost, ctx.t("files.find.button"))
          .icon(Icon::Search)
          .on_click(move || find.show()),
      )
      .child(key_cap("files-find-keys", &caps))
      .into()
  }
}

/// The width a line of `chars` characters needs.
#[allow(
  clippy::cast_precision_loss,
  reason = "a line holds far fewer characters than f32 counts exactly"
)]
fn content_width(chars: usize) -> f32 {
  NUMBER_WIDTH + GAP_WIDTH + chars as f32 * CODE_CHAR_WIDTH
}

/// Ctrl+C (Cmd+C) copies the selected lines while no field has the focus.
fn copy_keys(props: &CodeProps) -> impl Fn(KeyboardEvent) + Send + Sync + 'static + use<> {
  let copy = props.lines.map(|lines| {
    actions::copy(
      props.clipboard.clone(),
      props.refusal.clone(),
      (props.text.clone(), Some(lines)),
    )
  });
  move |event: KeyboardEvent| {
    let keys = Keys::of(&event);
    if keys.typing || !keys.only_command() || !keys.is("c", "KeyC") {
      return;
    }
    if let Some(copy) = &copy {
      copy();
      event.prevent_default();
    }
  }
}
