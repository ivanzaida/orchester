//! *Go to file* (`Files · go to file open` `y29f1V`): the trigger in the header, and the search it opens
//! under it, 560 wide: a field that says which version it searches, the files whose names answer it (a
//! 14 file icon, the name in 12 `text_primary`, its folder under it in Geist Mono 11 `text_muted`; the
//! current one on `hover` with an *Enter* key cap), and a footer with how many match and the keys. ↑ and ↓
//! move, Enter opens, Esc closes. Ctrl+P (Cmd+P) opens it too. The workspace ranks the names
//! ([`FileList::search`]).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::files::{FileList, NameMatches};

use super::{Page, keys, version};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, FilesText, IconSize},
  ui::{
    command_trigger::CommandTrigger,
    field::FieldInput,
    icon::{file_icon, icon},
    kbd::key_cap,
    pressable::pressable,
  },
};

/// The trigger's width (`YErsN`), the search's (`Go to file popover` `h6mtSs`), its field's height (`dWexO`)
/// and how many files it lists.
const TRIGGER_WIDTH: f32 = 200.0;
const PANEL_WIDTH: f32 = 560.0;
const QUERY_HEIGHT: f32 = 32.0;
const SHOWN: usize = 7;

/// The search's state: whether it is open, what is typed and which file is current.
#[derive(Clone)]
pub struct GotoState {
  pub open: Signal<bool>,
  query: Signal<String>,
  at: Signal<usize>,
  focus: Signal<bool>,
  input: ElementRef,
  anchor: ElementRef,
}

impl GotoState {
  /// Closes the search and forgets what it looked for.
  pub fn close(&self) {
    if self.open.get_untracked() {
      self.open.set(false);
    }
    if !self.query.get_untracked().is_empty() {
      self.query.set(String::new());
    }
    self.at.set(0);
  }

  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      query: ctx.signal(String::new()),
      at: ctx.signal(0),
      focus: ctx.signal(false),
      input: ElementRef::new(),
      anchor: ElementRef::new(),
    }
  }

  /// Opens the search with the focus in its field.
  pub fn show(&self) {
    self.at.set(0);
    self.open.set(true);
    self.focus.set(true);
  }
}

/// The trigger in the header.
pub fn trigger(ctx: &mut Ctx, page: &Page<'_>) -> Element {
  let state = page.view.goto.clone();
  let open = state.open.get();
  let (caps, _) = keys::caps(ctx);
  CommandTrigger::new(
    "files-go-to",
    (ctx.t("files.goTo.trigger"), caps),
    TRIGGER_WIDTH,
    state.anchor.clone(),
    move || state.show(),
  )
  .open(open)
  .into()
}

/// The search under the trigger while it is open, over the files of `list`. It is declared while closed too, so
/// that a press on the trigger switches to it from another open dropdown.
pub fn search(ctx: &mut Ctx, page: &Page<'_>, list: Option<&FileList>) -> Popup {
  let state = &page.view.goto;
  let panel = if state.open.get() {
    search_panel(ctx, page, list)
  } else {
    Element::new()
  };
  Popup::new(state.anchor.clone(), panel)
    .open(state.open.clone())
    .placement(Placement::BottomStart)
    .offset(0.0, ExtraSpacing::Snug.value())
}

/// The search's field, the files found and its footer.
fn search_panel(ctx: &mut Ctx, page: &Page<'_>, list: Option<&FileList>) -> Element {
  let state = page.view.goto.clone();
  if state.focus.get() {
    ctx.focus(&state.input);
    state.focus.set(false);
  }
  let found = list
    .map(|list| list.search(&state.query.get(), SHOWN))
    .unwrap_or_default();
  let current = state.at.get().min(found.paths.len().saturating_sub(1));
  let open = |ctx: &mut Ctx, path: &Arc<str>| {
    let go = page.open_file(ctx, path);
    let open = state.open.clone();
    move || {
      open.set(false);
      go();
    }
  };
  let query = state.query.get();
  let rows: Vec<Element> = found
    .paths
    .iter()
    .enumerate()
    .map(|(at, path)| {
      let choose = open(ctx, path);
      result(ctx, (path, &query), at == current, choose)
    })
    .collect();
  let chosen = found.paths.get(current).map(|path| open(ctx, path));
  let keys = move_keys(&state, found.paths.len(), chosen);
  let version = version::label(ctx, page);
  let field = FieldInput::new(
    "files-go-to-field",
    ctx.t("files.goTo.placeholder"),
    state.query.clone(),
  )
  .search()
  .placeholder(ctx.t("files.goTo.placeholder"))
  .unit(ctx.t_args("files.goTo.in", [("version", version.as_str())]))
  .box_height(QUERY_HEIGHT)
  .element(state.input.clone());
  let panel = Column::new()
    .id("files-go-to-search")
    .width(Dimension::Px(PANEL_WIDTH))
    .padding(ExtraSpacing::Snug.role())
    .spacing(ExtraSpacing::Tight.role())
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .box_shadow(ShadowStyle::Lg)
    .on_key_down(keys)
    .child(field.render(ctx))
    .child(
      Column::new()
        .id("files-go-to-results")
        .width(Dimension::Pct(100.0))
        .padding(Padding::vertical(SpacingSize::Xs))
        .with_children(rows),
    )
    .child(footer(ctx, &found, !query.trim().is_empty()));
  panel.into()
}

/// ↑ and ↓ move among `count` files, Enter opens the current one.
fn move_keys<F: Fn() + Send + Sync + 'static>(
  state: &GotoState,
  count: usize,
  chosen: Option<F>,
) -> impl Fn(KeyboardEvent) + Send + Sync + 'static + use<F> {
  let at = state.at.clone();
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
        chosen();
      }
      event.prevent_default();
    }
    _ => {}
  }
}

/// A file the search found: its icon, its name with the part that answers `query` in 600, and its folder; the
/// current one with the *Enter* cap.
fn result(ctx: &Ctx, (path, query): (&str, &str), current: bool, choose: impl Fn() + Send + Sync + 'static) -> Element {
  let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
  let glyph_color = if current {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextMuted
  };
  let row = pressable(choose)
    .id(format!("files-go-to:{path}"))
    .describe("path", path.to_owned())
    .describe("state", if current { "current" } else { "enabled" })
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(RESULT_HEIGHT))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .child(icon(file_icon(path), IconSize::Md, glyph_color))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(name_parts(name, query))
        .with_children(
          (!folder.is_empty()).then(|| Text::new(folder).variant(ExtraTypography::MonoNote.role()).nowrap()),
        ),
    )
    .with_children(current.then(|| key_cap("files-go-to-enter", &ctx.t("files.keys.enter"))));
  if current {
    row.background(ExtraColor::Hover.role()).into()
  } else {
    row
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .into()
  }
}

/// `name` with the part that holds `query`, ignoring case, in 600 `text_primary` and the rest in `text_secondary`
/// (`wgY4q`, `pZvb2`); without such a part, all of it in `text_secondary`.
fn name_parts(name: &str, query: &str) -> Row {
  let chars: Vec<char> = name.chars().collect();
  let wanted: Vec<char> = query.trim().chars().flat_map(char::to_lowercase).collect();
  let lower: Vec<char> = chars
    .iter()
    .map(|character| character.to_lowercase().next().unwrap_or(*character))
    .collect();
  let start = (!wanted.is_empty() && wanted.len() == query.trim().chars().count())
    .then(|| {
      lower
        .windows(wanted.len())
        .position(|window| window == wanted.as_slice())
    })
    .flatten();
  let part = |range: std::ops::Range<usize>, matched: bool| {
    let text: String = chars[range].iter().collect();
    let role = if matched {
      FilesText::NameMatch.role()
    } else {
      FilesText::Name.role()
    };
    Text::new(&text).variant(role).nowrap()
  };
  let row = Row::new().align_items(Alignment::Center);
  match start {
    Some(start) => {
      let end = start + wanted.len();
      row
        .with_children((start > 0).then(|| part(0..start, false)))
        .child(part(start..end, true))
        .with_children((end < chars.len()).then(|| part(end..chars.len(), false)))
    }
    None => row.child(part(0..chars.len(), false)),
  }
}

/// A result's height (`MfxMF`).
const RESULT_HEIGHT: f32 = 44.0;

/// How many files match, or with nothing typed how many the version has, and the keys (`Footer` `OyqYi`).
fn footer(ctx: &Ctx, found: &NameMatches, typed: bool) -> Element {
  let count = if !typed {
    let key = if found.total == 1 {
      "files.goTo.all.one"
    } else {
      "files.goTo.all.other"
    };
    ctx.t_args(key, [("count", found.total.to_string())]).to_string()
  } else if found.total == 0 {
    ctx.t("files.goTo.none").to_string()
  } else {
    let key = if found.total == 1 {
      "files.goTo.count.one"
    } else {
      "files.goTo.count.other"
    };
    ctx.t_args(key, [("count", found.total.to_string())]).to_string()
  };
  let hint = |id: &str, keys: &str, words: &str| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(key_cap(id, &ctx.t(keys)))
      .child(Text::new(&ctx.t(words)).variant(TypographyStyle::Caption).nowrap())
  };
  Row::new()
    .id("files-go-to-footer")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Xs),
    )
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&count)
        .id("files-go-to-count")
        .variant(TypographyStyle::Caption)
        .nowrap()
        .flex(1.0),
    )
    .child(hint("files-go-to-move", "files.keys.arrows", "files.goTo.move"))
    .child(hint("files-go-to-open", "files.keys.enter", "files.goTo.open"))
    .child(hint("files-go-to-close", "files.keys.escape", "files.goTo.close"))
    .into()
}
