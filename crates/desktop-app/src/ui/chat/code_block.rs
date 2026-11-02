//! A fenced code block in what an agent wrote (`Code block` `PqqIK` in 07 `GgXmd`, `chLpT` in 09 `w7TIt`, the head's
//! Copy in its states in 09e `Ezec2`, ORC-323): on `code_bg` inside a `border` stroke, radius 6. Its head, over a
//! `border` rule, says the language in 11 `text_muted`, with the file the fence names (*rust · sender.rs*), 12 from
//! the start, and holds Copy at its end, 4 inside it: a 13 `copy` and 11/500 *Copy* in `text_secondary`, 6 apart and
//! 8 at their sides, the `hover` fill under the pointer, always shown. A press puts the code on the clipboard without
//! its fence or its language; then for 2 s it says *Copied* by a `check` in `success` on the `hover` fill, or
//! *Couldn't copy* by a `triangle-alert`, both in `danger`. The code is lurq's Markdown code, Geist Mono 12/1.5 in
//! `text_primary`, 8 over and under it and 12 at its sides; its lines never wrap and scroll sideways inside the box,
//! and a search's places are filled behind them ([`super::found`]). lurq 0.54 draws no token colors for it: its text
//! takes no colored runs from an app, and its Markdown colors Rust only, in colors of its own.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Markdown, MarkdownProps, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  copy::{COPY_FEEDBACK, Copied},
  fences::CodeBlock,
  found::Found,
};
use crate::{
  theme::{ExtraColor, ExtraSpacing, IconSize, code_lines},
  ui::{
    clipboard::Clipboard,
    icon::{Icon, icon},
    pressable::{Focus, Press},
  },
};

/// The code block `block`, the `index`th of the message `id`, with what a search found in it (`found`) filled behind
/// its code. Its element id is `<id>-code-<index>`, its Copy's `<id>-code-<index>-copy`.
pub fn code_block(ctx: &mut Ctx, (id, index): (&str, usize), block: &CodeBlock, found: Option<&Found>) -> Element {
  let id = format!("{id}-code-{index}");
  let clipboard = Clipboard::of(ctx);
  ctx.mount_keyed::<CodeBlockView>(
    &id.clone(),
    CodeBlockProps {
      id: Arc::from(id),
      language: block.language.as_deref().map(Arc::from),
      file: block.file.as_deref().map(Arc::from),
      code: Arc::from(block.code.as_str()),
      found: found.cloned(),
      clipboard,
    },
  )
}

#[derive(Clone, PartialEq)]
struct CodeBlockProps {
  id: Arc<str>,
  language: Option<Arc<str>>,
  file: Option<Arc<str>>,
  code: Arc<str>,
  found: Option<Found>,
  clipboard: Clipboard,
}

impl DevtoolsInspectable for CodeBlockProps {}

struct CodeBlockView {
  copied: Signal<Copied>,
  reset: Timeout,
}

impl Component for CodeBlockView {
  type Props = CodeBlockProps;

  fn create(ctx: &mut Ctx) -> Self {
    let copied = ctx.signal(Copied::Not);
    let back = copied.clone();
    let reset = ctx.create_timeout(COPY_FEEDBACK, move || back.set(Copied::Not));
    Self { copied, reset }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CodeBlockProps>().clone();
    let head = self.head(ctx, &props);
    let lines = lines(ctx, &props);
    Column::new()
      .id(props.id.to_string())
      .width(Dimension::Pct(100.0))
      .background(ExtraColor::CodeBackground.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .describe("language", props.language.as_deref().unwrap_or_default())
      .child(head)
      .child(lines)
  }
}

impl CodeBlockView {
  /// The language and the file at its start, Copy at its end.
  fn head(&self, ctx: &mut Ctx, props: &CodeBlockProps) -> Row {
    let label = match (&props.language, &props.file) {
      (Some(language), Some(file)) => Some(ctx.t_args(
        "ui.chat.codeLanguageFile",
        [("language", &**language), ("file", &**file)],
      )),
      (Some(language), None) => Some(language.clone()),
      (None, Some(file)) => Some(file.clone()),
      (None, None) => None,
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Xs)
          .bottom(SpacingSize::Xs)
          .right(SpacingSize::Xs)
          .left(SpacingSize::Md),
      )
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(
        Row::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .with_children(label.map(|label| {
            Text::new(&label)
              .id(format!("{}-language", props.id))
              .variant(TypographyStyle::Caption)
              .nowrap()
          })),
      )
      .child(self.copy(ctx, props))
  }

  fn copy(&self, ctx: &mut Ctx, props: &CodeBlockProps) -> Button {
    let copied = self.copied.get();
    let (glyph, glyph_color, key) = match copied {
      Copied::Not => (Icon::Copy, PaletteColor::TextSecondary, "common.copy"),
      Copied::Done => (Icon::Check, PaletteColor::Success, "ui.copied"),
      Copied::Failed => (Icon::TriangleAlert, PaletteColor::Danger, "ui.copyFailed"),
    };
    let words_color = match copied {
      Copied::Not => PaletteColor::TextSecondary,
      Copied::Done => PaletteColor::TextPrimary,
      Copied::Failed => PaletteColor::Danger,
    };
    let words = ctx.t(key);
    let (state, reset) = (self.copied.clone(), self.reset.clone());
    let (clipboard, code) = (props.clipboard.clone(), props.code.clone());
    let press = Press::run(move || {
      state.set(Copied::of(&clipboard, &code));
      reset.restart();
    });
    let button = Button::empty()
      .id(format!("{}-copy", props.id))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .describe("a11y_name", words.clone())
      .class(copied.class())
      .cursor(CursorIcon::Pointer)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .active_style(Style::new().background(ExtraColor::Pressed.role()))
      .child(icon(glyph, IconSize::Base, glyph_color))
      .child(
        Text::new(&words)
          .id(format!("{}-copy-label", props.id))
          .variant(TypographyStyle::FieldLabel)
          .color(words_color)
          .nowrap(),
      );
    let button = if copied == Copied::Done {
      button.background(ExtraColor::Hover.role())
    } else {
      button
    };
    press.on(Focus::Keyboard.apply(button))
  }
}

/// The code's lines as lurq's Markdown draws a code block, without a box of its own: one selectable text that never
/// wraps and scrolls sideways, what a search found in it filled behind it.
fn lines(ctx: &mut Ctx, props: &CodeBlockProps) -> Element {
  let style = {
    let theme = ctx.theme();
    FormTextRole {
      typography: TypographyStyle::Body,
      color: PaletteColor::TextPrimary,
    }
    .resolve(&theme.typography(), &theme.palette())
  };
  let theme = code_lines(&ctx.theme().markdown());
  let markdown = MarkdownProps::styled(fenced(&props.code), style)
    .theme(theme)
    .width(Dimension::Pct(100.0))
    .selectable(true);
  let lines_id = format!("{}-lines", props.id);
  let Some(found) = &props.found else {
    return Markdown::mount(ctx, markdown).id(lines_id);
  };
  let highlights = found.highlights(ctx, &markdown.rendered_text());
  let places = highlights.len();
  let words = Markdown::mount(ctx, markdown.highlights(highlights)).id(lines_id);
  found.classed(words, places)
}

/// `code` as a fenced block without a language, its fence longer than any run of backticks in it.
fn fenced(code: &str) -> String {
  let longest = code.split(|ch| ch != '`').map(str::len).max().unwrap_or_default();
  let fence = "`".repeat(longest.max(2) + 1);
  format!("{fence}\n{code}\n{fence}")
}
