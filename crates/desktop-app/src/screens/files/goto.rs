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
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::files::{FileList, NameMatches};

use super::{Page, keys, version};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    command_trigger::CommandTrigger,
    field::FieldInput,
    icon::icon,
    kbd::key_cap,
    popover::{PopoverSide, popover, popover_panel},
    pressable::pressable,
  },
};

/// The trigger's width (`YErsN`), the search's (`Go to file · open` in `y29f1V`) and how many files it lists.
const TRIGGER_WIDTH: f32 = 200.0;
const PANEL_WIDTH: f32 = 560.0;
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
  let (caps, _) = keys::caps(ctx);
  CommandTrigger::new(
    "files-go-to",
    (ctx.t("files.goTo.trigger"), caps),
    TRIGGER_WIDTH,
    state.anchor.clone(),
    move || state.show(),
  )
  .into()
}

/// The search under the trigger while it is open, over the files of `list`.
pub fn search(ctx: &mut Ctx, page: &Page<'_>, list: Option<&FileList>) -> Option<Popup> {
  let state = page.view.goto.clone();
  if !state.open.get() {
    return None;
  }
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
  let rows: Vec<Element> = found
    .paths
    .iter()
    .enumerate()
    .map(|(at, path)| {
      let choose = open(ctx, path);
      result(ctx, path, at == current, choose)
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
  .element(state.input.clone());
  let panel = popover_panel("files-go-to-search", PANEL_WIDTH)
    .on_key_down(keys)
    .child(field.render(ctx))
    .child(
      Column::new()
        .id("files-go-to-results")
        .width(Dimension::Pct(100.0))
        .with_children(rows),
    )
    .child(footer(ctx, &found));
  Some(popover(
    state.anchor.clone(),
    state.open.clone(),
    PopoverSide::Below,
    panel,
  ))
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

/// A file the search found: its icon, name and folder; the current one with the *Enter* cap.
fn result(ctx: &Ctx, path: &str, current: bool, choose: impl Fn() + Send + Sync + 'static) -> Element {
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
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .child(icon(super::tree::file_icon(path), IconSize::Md, glyph_color))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(name)
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextPrimary)
            .nowrap(),
        )
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

/// How many files match, and the keys (`Footer` `OyqYi`).
fn footer(ctx: &Ctx, found: &NameMatches) -> Element {
  let count = if found.total == 0 {
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
    .padding(Padding::new().top(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
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
