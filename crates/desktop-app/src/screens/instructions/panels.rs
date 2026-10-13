//! Variables and effective prompt panels r8Ipz; full prompt viewer fTuZZ.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{InjectionEvent, Variable};
use orchester_workspace::{
  background,
  instructions::{
    data::{InstructionEntry, ProjectInstructions},
    form::{Events, Picked},
    preview::{PreviewCatalog, PreviewChoice},
  },
  messages,
  tasks::outside::Outside,
};

use super::{preview, variables};
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
  System,
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
  pub unavailable: Signal<super::fields::MissingVariables>,
  pub events: Signal<Events>,
  pub panel: Signal<Panel>,
  pub data: ProjectInstructions,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.entry == other.entry
      && self.valid == other.valid
      && self.data == other.data
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
}
impl Component for Panels {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      clipboard: ctx.use_context::<crate::ui::clipboard::Clipboard>().unwrap_or_default(),
      choice: ctx.signal(PreviewChoice::default()),
      event: ctx.signal(Picked(InjectionEvent::Start)),
      search: ctx.signal(String::new()),
      selected: ctx.signal(Picked(Variable::TaskTitle)),
      groups: ctx.signal(vec!["task".to_owned()]),
      copied: ctx.signal(Copied::Not),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let panel = props.panel.get();
    let changes = props.project.feed.outside.get();
    let root = props.project.open.repo_root.clone();
    let hold = crate::services::Services::of(ctx).hold;
    let catalog = ctx
      .future(
        (
          root,
          props.data.clone(),
          changes.of(Outside::Tasks),
          changes.of(Outside::Instructions),
        ),
        move |(root, ..)| {
          let hold = hold.clone();
          async move {
            hold.released().await;
            background::run(move || PreviewCatalog::load_in(&root))
              .await?
              .map_err(|error| messages::store_error(&error))
          }
        },
      )
      .state()
      .get();
    self.update_availability(catalog.data.as_ref(), &props);
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
      return preview::full(ctx, self, &props, &catalog);
    }
    let resolved = catalog.resolve(&self.choice.get());
    let content = match panel {
      Panel::Variables => {
        let search = self.search_field(ctx);
        frame = frame.child(search);
        variables::catalog(ctx, self, &props, resolved.as_ref().ok())
      }
      Panel::Preview => preview::panel(ctx, self, &props, &catalog),
      Panel::System => Text::new(&ctx.t("instructions.systemNote"))
        .variant(TypographyStyle::Description)
        .into(),
      Panel::Hidden | Panel::Full => Column::new().into(),
    };
    frame
      .child(
        ScrollVertical::new(content)
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .width(Dimension::Pct(100.0)),
      )
      .into()
  }
}
impl Panels {
  fn update_availability(&self, catalog: Option<&PreviewCatalog>, props: &Props) {
    let missing = catalog
      .and_then(|catalog| catalog.resolve(&self.choice.get()).ok())
      .map_or_else(
        || Variable::ALL.to_vec(),
        |resolved| {
          Variable::ALL
            .into_iter()
            .filter(|variable| resolved.value(*variable).is_none())
            .collect()
        },
      );
    if props.unavailable.get_untracked().0 != missing {
      props.unavailable.set(super::fields::MissingVariables(missing));
    }
  }
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
      (Panel::System, "instructions.system"),
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
      .child(segmented("instruction-panel-tabs", segments, Focus::Pointer))
      .child(cross)
      .into()
  }
}
