//! Resolved prompt (05.02 `fZa3J`, dialog `mWFGe`): the prompt's sources in order beside the
//! chosen source's text with its variables resolved, Copy full prompt, and a note that runs keep
//! the versions they started with. Read-only; Escape or the close control closes it and keeps the
//! edits.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  clipboard::copy_to_clipboard,
  components::{Button, Column, Modal, ModalTarget, Row, ScrollVertical, Spacer, Text},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::run::{Composition, InstructionVersion, PromptSection, PromptSource};

use super::{
  copy,
  editor::InstructionEditor,
  preview::{LoadedCatalog, number, source_description},
};
use crate::{
  instructions::data::InstructionEntry,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    command_row::Copied,
    icon::{Icon, IconSize, icon},
    pressable::{Ring, keyboard_stop},
    status_label::topbar_status_label,
  },
};

/// Dialog size, header and footer heights, source navigator width and row height, copy button
/// height (`mWFGe`, `PXBtd`, `UeN1j`, `DKyzq`, `ckJhN`, `zNo7n`).
const DIALOG_WIDTH: f32 = 1040.0;
const DIALOG_HEIGHT: f32 = 816.0;
const HEADER_HEIGHT: f32 = 78.0;
const FOOTER_HEIGHT: f32 = 50.0;
const NAVIGATOR_WIDTH: f32 = 304.0;
const ROW_HEIGHT: f32 = 48.0;
const COPY_HEIGHT: f32 = 32.0;

impl InstructionEditor {
  pub(super) fn viewer_modal(&self, ctx: &Ctx, live: &InstructionEntry, catalog: &LoadedCatalog) -> Element {
    let closed = || Modal::new(Column::new()).open_when(false).into();
    if !self.viewer_open.get() {
      return closed();
    }
    let Ok(Some(catalog)) = catalog else {
      return closed();
    };
    let Ok(resolved) = catalog.resolve(&self.preview.get()) else {
      return closed();
    };
    let composition = resolved.compose(self.edited(live), self.preview_event.get().0);
    let context = ctx.t_args(
      "instructions.viewer.context",
      [
        ("project", self.context.project.project.name().as_str()),
        ("task", resolved.task.task.key().to_string().as_str()),
        ("role", resolved.role()),
        ("step", resolved.step.name.as_str()),
        ("event", &*copy::event_label(ctx, composition.event())),
      ],
    );
    let navigator = Column::new()
      .width(Dimension::Px(NAVIGATOR_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Tight.role())
      .background(PaletteColor::SurfacePanel)
      .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(self.context_selectors(ctx, catalog, &resolved, live))
      .child(
        Text::new(&ctx.t_args(
          "instructions.viewer.sourceCount",
          [("count", composition.sections().len().to_string().as_str())],
        ))
        .variant(ExtraTypography::Overline.role()),
      )
      .child(ScrollVertical::new(self.navigator_rows(ctx, &composition, resolved.pipeline_name())).flex(1.0))
      .with_children(excluded(ctx, &composition));
    let dialog = Column::new()
      .id("instruction-viewer")
      .width(Dimension::Px(DIALOG_WIDTH))
      .height(Dimension::Px(DIALOG_HEIGHT))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .clip()
      .child(self.viewer_header(ctx, &context, &composition))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .child(navigator)
          .child(self.selected_source(ctx, &composition, resolved.pipeline_name())),
      )
      .child(footer(ctx));
    Modal::new(backdrop(dialog))
      .target(ModalTarget::Root)
      .open(self.viewer_open.clone())
      .into()
  }

  /// Title and context, Copy full prompt and the close control (`PXBtd`).
  fn viewer_header(&self, ctx: &Ctx, context: &str, composition: &Composition) -> Element {
    let text: Arc<str> = composition.text().into();
    let copied = self.copied.clone();
    let outcome = match &copied.get().0 {
      Some((copied, success)) if *copied == text => Some(*success),
      _ => None,
    };
    let label = match outcome {
      Some(true) => "instructions.viewer.copied",
      Some(false) => "instructions.viewer.copyFailed",
      None => "instructions.viewer.copy",
    };
    let copy = ActionButton::new(ctx.t(label))
      .icon(if outcome == Some(true) { Icon::Check } else { Icon::Copy })
      .id("instruction-copy-prompt")
      .in_form()
      .height(Dimension::Px(COPY_HEIGHT))
      .on_click(move || {
        // The prompt holds no secret values: the variable catalog has none.
        let success = copy_to_clipboard(&*text);
        if !success {
          tracing::warn!("the clipboard did not take the prompt");
        }
        copied.set(Copied(Some((text.clone(), success))));
      })
      .render(ctx);
    let open = self.viewer_open.clone();
    let close = keyboard_stop(Button::empty().button(), Ring::Outer)
      .id("instruction-viewer-close")
      .cursor(CursorIcon::Pointer)
      .rounded(RadiusSize::Sm)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .on_click(move |_| open.set(false))
      .child(icon(Icon::X, IconSize::Notice, PaletteColor::TextSecondary));
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(HEADER_HEIGHT))
      .padding_horizontal(SpacingSize::Lg)
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(
        Column::new()
          .flex(1.0)
          .spacing(SpacingSize::Sm)
          .child(Text::new(&ctx.t("instructions.viewer.title")).variant(ExtraTypography::DialogTitle.role()))
          .child(Text::new(context).variant(TypographyStyle::Caption)),
      )
      .child(copy)
      .child(close)
      .into()
  }

  fn navigator_rows(&self, ctx: &Ctx, composition: &Composition, pipeline: &str) -> Element {
    let selected = self.viewer_source.get();
    let rows = composition.sections().iter().enumerate().map(|(index, section)| {
      let (title, metadata) = copy::source_text(ctx, section, pipeline);
      let source = self.viewer_source.clone();
      let row = Button::empty()
        .id(Arc::<str>::from(format!("instruction-viewer-source-{index}")))
        .focusable(false)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(ROW_HEIGHT))
        .padding_horizontal(SpacingSize::Sm)
        .spacing(ExtraSpacing::Inline.role())
        .align_items(Alignment::Center)
        .rounded(RadiusSize::Sm)
        .cursor(CursorIcon::Pointer)
        .child(number(index))
        .child(source_description(&title, &metadata));
      if index == selected {
        row.background(PaletteColor::AccentMuted).describe("state", "selected")
      } else {
        row
          .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
          .on_click(move |_| source.set(index))
      }
    });
    Column::new().width(Dimension::Pct(100.0)).with_children(rows).into()
  }

  /// The chosen source (`z3eDQH`): its title and version, where it comes from, its text with the
  /// variables resolved, and how many resolved.
  fn selected_source(&self, ctx: &Ctx, composition: &Composition, pipeline: &str) -> Element {
    let sections = composition.sections();
    let index = self.viewer_source.get().min(sections.len().saturating_sub(1));
    let Some(section) = sections.get(index) else {
      return Column::new().flex(1.0).into();
    };
    let (title, _) = copy::source_text(ctx, section, pipeline);
    let heading = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .child(Text::new(&title).variant(ExtraTypography::Subheading.role()).flex(1.0))
      .with_children(
        copy::source_version(ctx, &section.source)
          .map(|version| topbar_status_label(None, &version, "instruction-viewer-version")),
      );
    let text = if section.text.trim().is_empty() {
      ctx.t("instructions.viewer.empty")
    } else {
      section.text.as_str().into()
    };
    let content = ScrollVertical::new(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(ExtraSpacing::Step.role())
        .child(
          Text::new(&text)
            .variant(ExtraTypography::PromptText.role())
            .width(Dimension::Pct(100.0))
            .selectable(true),
        ),
    )
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .rounded(ExtraRadius::Notice.role());
    Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .padding(SpacingSize::Lg)
      .spacing(SpacingSize::Md)
      .child(heading)
      .child(Text::new(&origin(ctx, section, composition)).variant(TypographyStyle::Caption))
      .child(content)
      .child(Text::new(&resolution(ctx, section, index, sections.len())).variant(TypographyStyle::Caption))
      .into()
  }
}

/// "Project rule · Agent start", and for a draft the version it stands in for.
fn origin(ctx: &Ctx, section: &PromptSection, composition: &Composition) -> Arc<str> {
  let (kind, event) = (
    copy::source_kind(ctx, &section.source),
    copy::event_label(ctx, composition.event()),
  );
  match section.source {
    PromptSource::Instruction {
      version: InstructionVersion::Draft {
        based_on: Some(published),
      },
      ..
    } => ctx.t_args(
      "instructions.viewer.originDraft",
      [
        ("kind", &*kind),
        ("event", &*event),
        ("version", published.get().to_string().as_str()),
      ],
    ),
    _ => ctx.t_args("instructions.viewer.origin", [("kind", &*kind), ("event", &*event)]),
  }
}

/// "Source 2 of 7 · 2 variables resolved", and those without a value.
fn resolution(ctx: &Ctx, section: &PromptSection, index: usize, count: usize) -> Arc<str> {
  let resolved = section.variables.len() - section.unavailable.len();
  let (position, count, resolved) = ((index + 1).to_string(), count.to_string(), resolved.to_string());
  if section.unavailable.is_empty() {
    return ctx.t_args(
      "instructions.viewer.resolved",
      [
        ("index", position.as_str()),
        ("count", count.as_str()),
        ("resolved", resolved.as_str()),
      ],
    );
  }
  let names: Vec<&str> = section.unavailable.iter().map(|variable| variable.name()).collect();
  ctx.t_args(
    "instructions.viewer.unresolved",
    [
      ("index", position.as_str()),
      ("count", count.as_str()),
      ("resolved", resolved.as_str()),
      ("unavailable", names.join(", ").as_str()),
    ],
  )
}

/// The rules the prompt leaves out, and why (`T8eQc`).
fn excluded(ctx: &Ctx, composition: &Composition) -> Vec<Element> {
  composition
    .excluded()
    .iter()
    .map(|excluded| {
      let text = ctx.t_args(
        "instructions.preview.excluded",
        [
          ("name", excluded.name.as_str()),
          ("version", &*copy::instruction_version(ctx, excluded.version)),
          ("reason", &*copy::exclusion(ctx, excluded.reason)),
        ],
      );
      Text::new(&text).variant(ExtraTypography::Hint.role()).into()
    })
    .collect()
}

fn footer(ctx: &Ctx) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .padding_horizontal(SpacingSize::Lg)
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon(Icon::Info, IconSize::Dismiss, PaletteColor::TextSecondary))
    .child(Text::new(&ctx.t("instructions.viewer.note")).variant(TypographyStyle::Caption))
    .child(Spacer::new().flex(1.0))
    .into()
}
