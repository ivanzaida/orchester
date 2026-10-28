//! Variables and effective prompt panels r8Ipz; full prompt viewer fTuZZ.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{InjectionEvent, Variable};
use orchester_workspace::instructions::{
  data::{InstructionEntry, ProjectInstructions},
  form::{Events, Picked},
  preview::PreviewChoice,
};

use super::{
  availability::{Availability, CatalogState},
  preview,
  state::{Panel, SidePanel},
  variables,
};
use crate::{
  project::ShownProject,
  screens::hub::parts,
  theme::HubSpacing,
  ui::{
    field::{FieldInput, field_error},
    icon::Icon,
    icon_button::IconButton,
    message::message_text,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// The panel beside a rule (`Side` `CBvLu` in 02b), wider than the hub's side column.
const PANEL_WIDTH: f32 = 424.0;

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
  pub panel: SidePanel,
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
      && self.panel.same(&other.panel)
  }
}
impl DevtoolsInspectable for Props {}
pub struct Panels {
  pub clipboard: crate::ui::clipboard::Clipboard,
  pub choice: Signal<PreviewChoice>,
  pub event: Signal<Picked<InjectionEvent>>,
  pub search: Signal<String>,
  pub selected: Signal<Picked<Variable>>,
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
      .flex_shrink(0.0)
      .height(Dimension::Pct(100.0))
      .spacing(HubSpacing::SideSections.role())
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
        frame = frame.child(self.search_field(ctx));
        variables::catalog(ctx, self, &props, resolved.as_ref().ok())
      }
      Panel::Preview => preview::panel(ctx, self, &props, &catalog),
      Panel::Hidden | Panel::Full => Column::new().into(),
    };
    // What the runtime adds besides the rules (`l6yIDs` in 02b); in the catalog, how a missing value is handled.
    let note = ctx.t(if panel == Panel::Preview {
      "instructions.systemNote"
    } else if props.availability.unavailable.contains(&self.selected.get().0) {
      "instructions.noEmptyValue"
    } else {
      "instructions.missingNote"
    });
    frame
      .child(content)
      .child(parts::note("instruction-panel-note", Icon::Info, &note))
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
      (Panel::Preview, "preview", "instructions.preview"),
      (Panel::Variables, "variables", "instructions.variables"),
    ]
    .into_iter()
    .map(|(panel, name, key)| {
      let chosen = props.panel.clone();
      Segment::new(
        format!("instruction-panel-{name}"),
        SegmentFace::Text(ctx.t(key)),
        props.panel.get() == panel,
        move || chosen.choose(panel),
      )
    })
    .collect::<Vec<_>>();
    let close = props.panel.clone();
    let cross = IconButton::new("instruction-panel-close", Icon::X, ctx.t("instructions.closePanel"))
      .inspector()
      .on_click(move || close.close(Panel::Hidden))
      .render(ctx);
    // The switcher sits at the panel's end, beside Close (02b, 02d).
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(lurq::layout::Alignment::Center)
      .child(Row::new().flex(1.0))
      .child(segmented("instruction-panel-tabs", segments, Focus::Pointer))
      .child(cross)
      .into()
  }
}
