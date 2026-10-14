//! Dedicated full prompt viewer fTuZZ.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::{Composition, InstructionVersion, PromptSection, PromptSource};
use orchester_workspace::instructions::{
  form::Events,
  preview::{EditedInstruction, PreviewCatalog, Resolved},
};

use super::{
  panels::{Panel, Panels, Props},
  preview, sources, words,
};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, instructions},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
    icon_button::IconButton,
    resolved_source::ResolvedSource,
  },
};
// CKzcD / IAt1E / footer n96cB.
const HEADER_HEIGHT: f32 = 52.0;
const INDEX_WIDTH: f32 = 300.0;
const FOOTER_HEIGHT: f32 = 34.0;
// Outlined shortcut nPUFf.
const KEYCAP_HEIGHT: f32 = 18.0;

pub fn full(ctx: &mut Ctx, state: &Panels, props: &Props, catalog: &PreviewCatalog) -> Element {
  let resolved = match catalog.resolve(&state.choice.get()) {
    Ok(resolved) => resolved,
    Err(_) => return unavailable(ctx, state, props, catalog),
  };
  let Some(composition) = preview::compose(props, state, &resolved) else {
    return unavailable(ctx, state, props, catalog);
  };
  let selected = state
    .source
    .get()
    .0
    .or_else(|| sources::edited_index(&composition, props.entry.as_ref().map(|e| e.id)))
    .unwrap_or(0)
    .min(composition.sections().len().saturating_sub(1));
  let header = header(ctx, state, props, &composition);
  let sidebar = sidebar(ctx, state, props, &resolved, &composition);
  let footer = footer(ctx, props, &resolved, &composition, selected);
  let mut contents = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .padding(Padding::symmetric(
      ExtraSpacing::Tabs.role(),
      ExtraSpacing::Inset.role(),
    ));
  for (index, section) in composition.sections().iter().enumerate() {
    contents = contents.child(block(
      ctx,
      (props, composition.event()),
      &resolved,
      section,
      index,
      selected == index,
    ));
  }
  Column::new()
    .id("instruction-full-viewer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .on_key_down({
      let panel = props.panel.clone();
      move |event: lurq::app::events::KeyboardEvent| {
        if event.key == "Escape" {
          panel.set(Panel::Preview);
          event.prevent_default();
        }
      }
    })
    .child(header)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(sidebar)
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .height(Dimension::Pct(100.0))
            .background(ExtraColor::CodeBackground.role())
            .child(ScrollVertical::new(contents).flex(1.0).min_height(Dimension::Px(0.0)))
            .child(footer),
        ),
    )
    .into()
}
fn header(ctx: &mut Ctx, state: &Panels, props: &Props, composition: &Composition) -> Element {
  let back = props.panel.clone();
  let arrow = IconButton::new("instruction-viewer-return", Icon::ArrowLeft, ctx.t("instructions.back"))
    .small()
    .on_click(move || back.set(Panel::Preview))
    .render(ctx);
  let panel = props.panel.clone();
  let rule = props.panel.clone();
  let name = props
    .entry
    .as_ref()
    .map_or_else(|| ctx.t("instructions.new"), |entry| entry.name.clone());
  let crumbs = breadcrumb(
    ctx,
    "instruction-viewer-crumbs",
    CrumbSeparator::Chevron,
    vec![
      Crumb::link(ctx.t("instructions.title"), move || panel.set(Panel::Preview)),
      Crumb::link(name, move || rule.set(Panel::Preview)),
      Crumb::here(ctx.t("instructions.effective")).title(),
    ],
  );
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADER_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Tabs.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(arrow)
    .child(Row::new().flex(1.0).child(crumbs))
    .child(icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted))
    .child(Text::new(&ctx.t("instructions.readOnly")).variant(TypographyStyle::Caption))
    .child(preview::copy_button(ctx, state, props, composition))
    .into()
}
fn sidebar(ctx: &Ctx, state: &Panels, props: &Props, resolved: &Resolved<'_>, composition: &Composition) -> Element {
  Column::new()
    .width(Dimension::Px(INDEX_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Sm)
    .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&ctx.t(preview::mode_key(props))).variant(instructions::overline()))
    .child(
      Text::new(&ctx.t_args(
        "instructions.viewerContext",
        [
          ("project", props.project.open.project.name().as_str()),
          ("task", &resolved.task.task.key().to_string()),
          ("role", resolved.role()),
          ("step", resolved.step.name.as_str()),
          (
            "event",
            &words::event(
              ctx,
              if state.event.get().0 == orchester_core::resource::InjectionEvent::Start {
                Events::Start
              } else {
                Events::Resume
              },
            ),
          ),
        ],
      ))
      .id("instruction-viewer-context")
      .variant(TypographyStyle::Description),
    )
    .child(
      ScrollVertical::new(sources::list(
        ctx,
        state,
        composition,
        props.entry.as_ref().map(|e| e.id),
        true,
      ))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .width(Dimension::Pct(100.0)),
    )
    .child(note(ctx, Icon::Info, "instructions.viewerNote"))
    .child(note(ctx, Icon::ShieldCheck, "instructions.secretNote"))
    .into()
}
fn note(ctx: &Ctx, mark: Icon, key: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(icon(mark, IconSize::Xs, PaletteColor::TextMuted))
    .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption).flex(1.0))
    .into()
}
fn block(
  ctx: &mut Ctx,
  (props, event): (&Props, orchester_core::resource::InjectionEvent),
  resolved: &Resolved<'_>,
  section: &PromptSection,
  index: usize,
  selected: bool,
) -> Element {
  let (title, meta) = block_heading(ctx, event, section, index);
  let entry = props.entry.as_ref().expect("valid composition has an edited entry");
  let ranges = resolved.source_ranges(
    section,
    EditedInstruction {
      id: entry.id,
      name: &entry.name,
      slug: &entry.slug,
      body: &entry.body,
      version: InstructionVersion::Draft {
        based_on: entry.based_on(),
      },
    },
  );
  let text = ResolvedSource {
    id: format!("instruction-viewer-text-{index}").into(),
    text: section.text.clone().into(),
    values: ranges,
  }
  .render(ctx);
  let block = Column::new()
    .id(format!("instruction-viewer-source-{index}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Inset.role(),
      ExtraSpacing::Loose.role(),
    ))
    .spacing(ExtraSpacing::Snug.role())
    .rounded(ExtraRadius::Tile.role());
  let block = if selected {
    block
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
  } else {
    block
  };
  block
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(
          Text::new(&title)
            .variant(instructions::overline())
            .color(if selected {
              PaletteColor::TextPrimary
            } else {
              PaletteColor::TextMuted
            })
            .flex(1.0),
        )
        .child(Text::new(&meta).variant(TypographyStyle::Caption)),
    )
    .child(text)
    .into()
}
fn footer(ctx: &Ctx, props: &Props, resolved: &Resolved<'_>, composition: &Composition, selected: usize) -> Element {
  let variables = composition
    .sections()
    .get(selected)
    .into_iter()
    .flat_map(|section| section.variables.iter())
    .filter(|variable| resolved.value(**variable).is_some())
    .copied()
    .collect::<std::collections::BTreeSet<_>>();
  let back = props.panel.clone();
  let summary = ctx.t_args(
    if variables.len() == 1 {
      "instructions.viewerFooter"
    } else {
      "instructions.viewerFooterMany"
    },
    [
      ("source", (selected + 1).to_string()),
      ("total", composition.sections().len().to_string()),
      ("count", variables.len().to_string()),
    ],
  );
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .align_items(Alignment::Center)
    .padding(Padding::horizontal(ExtraSpacing::Tabs.role()))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfacePanel)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&summary)
        .id("instruction-viewer-summary")
        .variant(TypographyStyle::Caption),
    )
    .with_children(variables.iter().next().and_then(|v| resolved.value(*v)).map(|value| {
      Text::new(&value)
        .variant(instructions::variable_value())
        .background(PaletteColor::SurfaceRaised)
    }))
    .child(
      Text::new(&ctx.t("instructions.resolvedVariable"))
        .variant(TypographyStyle::Caption)
        .flex(1.0),
    )
    .child(
      Row::new()
        .height(Dimension::Px(KEYCAP_HEIGHT))
        .align_items(Alignment::Center)
        .padding(Padding::horizontal(ExtraSpacing::Pair.role()))
        .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
        .rounded(lurq::app::theme::RadiusSize::Sm)
        .child(Text::new(&ctx.t("instructions.esc")).variant(instructions::viewer_keycap())),
    )
    .child(
      crate::ui::button::ActionButton::new(
        "instruction-viewer-back",
        crate::ui::button::ButtonKind::Ghost,
        ctx.t("instructions.back"),
      )
      .on_click(move || back.set(Panel::Preview)),
    )
    .into()
}
fn block_heading(
  ctx: &Ctx,
  event: orchester_core::resource::InjectionEvent,
  section: &PromptSection,
  index: usize,
) -> (std::sync::Arc<str>, std::sync::Arc<str>) {
  let kind = words::part(ctx, section.source.part());
  let version = match section.source {
    PromptSource::Instruction {
      version: InstructionVersion::Draft { based_on },
      ..
    } => ctx.t_args(
      "instructions.version",
      [("version", based_on.map_or(1, |v| v.get().saturating_add(1)).to_string())],
    ),
    _ => sources::version(ctx, &section.source),
  };
  let title = ctx.t_args(
    "instructions.viewerSource",
    [
      ("number", (index + 1).to_string()),
      ("kind", kind.to_uppercase()),
      ("name", section.name.clone()),
      ("version", version.to_string()),
    ],
  );
  let event = words::event(
    ctx,
    if event == orchester_core::resource::InjectionEvent::Resume {
      Events::Resume
    } else {
      Events::Start
    },
  );
  let meta = match section.source {
    PromptSource::Instruction {
      version: InstructionVersion::Draft { based_on: Some(v) },
      ..
    } => ctx.t_args(
      "instructions.viewerDraftEvent",
      [("event", event.to_string()), ("version", v.get().to_string())],
    ),
    _ => ctx.t_args(
      "instructions.viewerSourceEvent",
      [("kind", kind.to_string()), ("event", event.to_string())],
    ),
  };

  (title, meta)
}
fn unavailable(ctx: &Ctx, state: &Panels, props: &Props, catalog: &PreviewCatalog) -> Element {
  let back = props.panel.clone();
  Column::new()
    .id("instruction-viewer-unavailable")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .padding(ExtraSpacing::Tabs.role())
    .spacing(SpacingSize::Md)
    .child(
      crate::ui::button::ActionButton::new(
        "instruction-viewer-back",
        crate::ui::button::ButtonKind::Ghost,
        ctx.t("instructions.back"),
      )
      .icon(Icon::ArrowLeft)
      .on_click(move || back.set(Panel::Preview)),
    )
    .child(preview::panel(ctx, state, props, catalog))
    .into()
}
