//! Variables and effective prompt panels r8Ipz; full prompt viewer fTuZZ.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{InjectionEvent, Variable};
use orchester_workspace::instructions::{
  data::{InstructionEntry, ProjectInstructions},
  form::{Events, Picked},
  preview::PreviewChoice,
};

use super::{
  availability::{Availability, CatalogState},
  preview, variables,
};
use crate::{
  project::ShownProject,
  theme::ExtraSpacing,
  ui::{
    field::{FieldInput, field_error},
    icon::Icon,
    icon_button::IconButton,
    message::message_text,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

// Variables panel r8Ipz and prompt viewer's sources rail.
pub const PANEL_WIDTH: f32 = 340.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
  Hidden,
  Variables,
  Preview,
  Full,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyReceipt {
  pub composition: orchester_core::run::Composition,
  pub choice: PreviewChoice,
  pub event: InjectionEvent,
  pub entry: Option<InstructionEntry>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Copied {
  Not,
  Done(CopyReceipt),
  Failed(CopyReceipt),
}
impl DevtoolsInspectable for Copied {}
impl DevtoolsInspectable for Panel {}
#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub entry: Option<InstructionEntry>,
  pub valid: bool,
  pub input: crate::ui::prompt_input::PromptInput,
  pub frozen: Signal<bool>,
  pub events: Signal<Events>,
  /// What the preview is for; the editor resolves it too.
  pub choice: Signal<PreviewChoice>,
  pub catalog: CatalogState,
  pub availability: Availability,
  pub panel: Signal<Panel>,
  pub data: ProjectInstructions,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.entry == other.entry
      && self.valid == other.valid
      && self.data == other.data
      && self.catalog == other.catalog
      && self.availability == other.availability
      && self.panel.id() == other.panel.id()
  }
}
impl DevtoolsInspectable for Props {}
pub struct Panels {
  pub clipboard: crate::ui::clipboard::Clipboard,
  pub choice: Signal<PreviewChoice>,
  pub event: Signal<Picked<InjectionEvent>>,
  pub search: Signal<String>,
  pub selected: Signal<Picked<Variable>>,
  pub groups: Signal<Vec<String>>,
  pub copied: Signal<Copied>,
  pub source: Signal<Picked<Option<usize>>>,
}
impl Component for Panels {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      clipboard: ctx.use_context::<crate::ui::clipboard::Clipboard>().unwrap_or_default(),
      choice: ctx.props::<Props>().choice.clone(),
      event: ctx.signal(Picked(InjectionEvent::Start)),
      search: ctx.signal(String::new()),
      selected: ctx.signal(Picked(Variable::TaskTitle)),
      groups: ctx.signal(vec!["task".to_owned()]),
      copied: ctx.signal(Copied::Not),
      source: ctx.signal(Picked(None)),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let panel = props.panel.get();
    let catalog = props.catalog.clone();
    if panel == Panel::Hidden {
      return Column::new().into();
    }
    let head = self.header(ctx, &props);
    let mut frame = Column::new()
      .id("instruction-panel")
      .width(Dimension::Px(PANEL_WIDTH))
      .height(Dimension::Pct(100.0))
      .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
      .spacing(SpacingSize::Sm)
      .child(head);
    if let Some(error) = catalog.error.clone().filter(|_| catalog.is_rejected()) {
      return frame
        .child(field_error("instruction-preview-error", &message_text(ctx, &error)))
        .into();
    }
    let Some(catalog) = catalog.data else {
      return frame
        .child(crate::ui::state::loading(
          "instruction-preview-loading",
          &ctx.t("instructions.previewReading"),
        ))
        .into();
    };
    if panel == Panel::Full {
      return super::viewer::full(ctx, self, &props, &catalog);
    }
    let resolved = catalog.resolve(&self.choice.get());
    let content = match panel {
      Panel::Variables => {
        if let Ok(resolved) = &resolved {
          frame = frame.child(variables::context(ctx, &props, resolved));
        }
        frame = frame.child(self.search_field(ctx));
        variables::catalog(ctx, self, &props, resolved.as_ref().ok())
      }
      Panel::Preview => preview::panel(ctx, self, &props, &catalog),
      Panel::Hidden | Panel::Full => Column::new().into(),
    };
    frame
      .child(content)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(crate::ui::icon::icon(
            if panel == Panel::Preview {
              Icon::ShieldCheck
            } else {
              Icon::Info
            },
            crate::theme::IconSize::Xs,
            PaletteColor::TextMuted,
          ))
          .child(
            Text::new(&ctx.t(if panel == Panel::Preview {
              "instructions.secretNote"
            } else if props.availability.unavailable.contains(&self.selected.get().0) {
              "instructions.noEmptyValue"
            } else {
              "instructions.missingNote"
            }))
            .variant(TypographyStyle::Caption)
            .flex(1.0),
          ),
      )
      .into()
  }
}
impl Panels {
  fn search_field(&self, ctx: &mut Ctx) -> Element {
    FieldInput::new(
      "instruction-variable-search",
      ctx.t("instructions.search"),
      self.search.clone(),
    )
    .search()
    .placeholder(ctx.t("instructions.search"))
    .render(ctx)
  }
  fn header(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let segments = [
      (Panel::Variables, "instructions.variables"),
      (Panel::Preview, "instructions.preview"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (panel, key))| {
      let chosen = props.panel.clone();
      Segment::new(
        format!("instruction-panel-{index}"),
        SegmentFace::Text(ctx.t(key)),
        props.panel.get() == panel,
        move || chosen.set(panel),
      )
    })
    .collect::<Vec<_>>();
    let close = props.panel.clone();
    let cross = IconButton::new("instruction-panel-close", Icon::X, ctx.t("instructions.closePanel"))
      .inspector()
      .on_click(move || close.set(Panel::Hidden))
      .render(ctx);
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .child(
        Row::new()
          .flex(1.0)
          .child(segmented("instruction-panel-tabs", segments, Focus::Pointer)),
      )
      .child(cross)
      .into()
  }
}
