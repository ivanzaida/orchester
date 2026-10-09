//! Import resources (06.04 `W4zGKx`, 06.05 `QqnDd`, 06.06 `hF664`; dialog `x2PZm`, `tSRGr`):
//! choose a project and the resources to copy, then how everything they refer to is resolved in
//! this project, and import them as local drafts. The dialog opens over the page it was opened
//! from ([`crate::navigation::import`]); once the copies exist, the first one opens.

mod labels;
mod mapping_row;
mod resolve;
mod resource_row;
mod select;

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::{IdempotencyKey, ProjectId, ResourceId},
  import::{Choices, Content, CopiedResource, ImportBatch, Pick, Plan},
};
use orchester_store::RegisteredProject;

use super::{ProjectNav, instructions::copy::tab_of};
use crate::{
  activity::{OperationSpec, Target, operations},
  background,
  import::data::{self, Catalogs, Sources},
  messages::{self, Message},
  navigation::{
    Navigation,
    import::{self as import_query, ImportTab},
    route::{AgentSelection, Page, PipelineView, ToolSelection},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::{ActionButton, SKIPPED_BY_TAB},
    form::form,
    icon::Icon,
  },
  workspace::OpenProject,
};

/// Dialog, header and footer sizes and the button height (`x2PZm`, `v635F`, `qSGs5`, `WLeOy`).
const DIALOG_WIDTH: f32 = 1040.0;
const DIALOG_HEIGHT: f32 = 864.0;
const HEADER_HEIGHT: f32 = 82.0;
const FOOTER_HEIGHT: f32 = 76.0;
const BUTTON_HEIGHT: f32 = 36.0;

/// The dialog over the page the path names, while the path carries it; nothing otherwise.
pub(super) fn dialog(ctx: &mut Ctx, project: &OpenProject) -> Element {
  ctx.mount_keyed::<ImportLayer>("import-layer", project.clone())
}

/// The modal layer of Import resources. It is open while the path carries the dialog; Escape
/// closes it like every other dialog, which leaves the dialog's history entry.
pub struct ImportLayer {
  open: Signal<bool>,
}

impl Component for ImportLayer {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let open = ctx.signal(false);
    ctx.watch(&open, move |open| {
      if !*open && nav.import_shown() {
        nav.close_import();
      }
    });
    Self { open }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let tab = import_query::tab(&ctx.route_path());
    // Follows the path, so that only Escape turns it off while the path still carries the dialog.
    if self.open.get_untracked() != tab.is_some() {
      self.open.set(tab.is_some());
    }
    let Some(tab) = tab else {
      return Modal::new(Column::new()).open_when(false);
    };
    let props = ImportProps {
      project: ctx.props::<OpenProject>().clone(),
      tab,
    };
    let dialog = ctx.mount_keyed::<ImportDialog>("import-resources", props);
    Modal::new(backdrop(ctx, dialog))
      .target(ModalTarget::Root)
      .open(self.open.clone())
  }
}

#[derive(Clone, PartialEq)]
pub struct ImportProps {
  project: OpenProject,
  /// The tab the dialog opened on.
  tab: ImportTab,
}

impl lurq::app::component::DevtoolsInspectable for ImportProps {}

/// Select resources (06.04), then resolve what they refer to (06.05, 06.06).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
  Select,
  Resolve,
}

impl lurq::app::component::DevtoolsInspectable for Stage {}

/// The project resources are imported from; `None` until the projects were read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceChoice(Option<ProjectId>);

impl lurq::app::component::DevtoolsInspectable for SourceChoice {}

/// The selected resources and the content each copy takes, in the order they were selected.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Picked(Vec<(ResourceId, Pick)>);

impl lurq::app::component::DevtoolsInspectable for Picked {}

/// The user's choices for what the selection refers to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ChoiceState(Choices);

impl lurq::app::component::DevtoolsInspectable for ChoiceState {}

/// The source project and what both projects hold, once read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Loaded(Option<Catalogs>);

impl lurq::app::component::DevtoolsInspectable for Loaded {}

/// What opens once the copies exist.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Imported(Page);

impl lurq::app::component::DevtoolsInspectable for Imported {}

type ImportArgs = (PathBuf, ImportBatch, OperationSpec);
type ImportAction = FutureAction<ImportArgs, Imported, Message>;

/// Where the source stands: not chosen or not read yet, not readable, or read.
enum Source<'a> {
  Pending,
  Failed(Message),
  Ready(&'a Catalogs),
}

pub struct ImportDialog {
  source: Signal<SourceChoice>,
  stage: Signal<Stage>,
  /// The tab chosen in the dialog; `None` shows the one it opened on.
  tab: Signal<Option<ImportTab>>,
  search: Signal<String>,
  picked: Signal<Picked>,
  choices: Signal<ChoiceState>,
  /// Identifies this import, so retrying after a failure creates nothing twice.
  key: IdempotencyKey,
  /// Focused when the dialog opens, so Tab starts at its first control.
  frame: ElementRef,
}

impl Component for ImportDialog {
  type Props = ImportProps;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let action = import_action(ctx);
    ctx.watch(&action.state(), move |state| {
      if let Some(Imported(page)) = &state.data {
        nav.resources_changed();
        nav.finish(*page);
      }
    });
    let dialog = Self {
      source: ctx.signal(SourceChoice(None)),
      stage: ctx.signal(Stage::Select),
      tab: ctx.signal(None),
      search: ctx.signal(String::new()),
      picked: ctx.signal(Picked::default()),
      choices: ctx.signal(ChoiceState::default()),
      key: IdempotencyKey::generate(),
      frame: ElementRef::new(),
    };
    // Resources and choices belong to the project they were read from.
    let (picked, choices, stage) = (dialog.picked.clone(), dialog.choices.clone(), dialog.stage.clone());
    ctx.watch(&dialog.source, move |_| {
      picked.set(Picked::default());
      choices.set(ChoiceState::default());
      stage.set(Stage::Select);
    });
    ctx.focus(&dialog.frame);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ImportProps>().clone();
    // Futures are matched by call order: the action comes first, as in `create`.
    let action = import_action(ctx);
    let sources = self.sources(ctx, &props.project);
    let chosen = self.chosen(sources.as_ref());
    let loaded = self.load(ctx, &props.project, chosen.as_ref());
    let source = match &loaded {
      Err(error) => Source::Failed(error.clone()),
      Ok(Some(catalogs)) => Source::Ready(catalogs),
      Ok(None) => Source::Pending,
    };
    // Resolving needs what both projects hold; until it is read again, selecting shows.
    let (title, body, footer) = match (self.stage.get(), &source) {
      (Stage::Resolve, Source::Ready(catalogs)) => {
        let plan = self.plan(catalogs);
        (
          ctx.t("projectImport.resolveTitle"),
          self.resolve_body(ctx, catalogs, &plan),
          self.resolve_footer(ctx, &props.project, catalogs, &plan, &action),
        )
      }
      _ => (
        ctx.t("projectImport.title"),
        self.select_body(ctx, &props, sources.as_ref(), chosen.as_ref(), &source),
        self.select_footer(ctx, &source),
      ),
    };
    let dialog = Column::new()
      .id("import-dialog")
      .ref_element(self.frame.clone())
      .focusable(true)
      .tab_index(SKIPPED_BY_TAB)
      .width(Dimension::Px(DIALOG_WIDTH))
      .height(Dimension::Px(DIALOG_HEIGHT))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .clip()
      .child(header(ctx, &title))
      .child(body)
      .child(footer);
    form(None, dialog)
  }
}

impl ImportDialog {
  /// The projects to import from: every other project of the workspace.
  fn sources(&self, ctx: &mut Ctx, project: &OpenProject) -> Result<Option<Sources>, Message> {
    let workspace = ctx
      .use_context::<Navigation>()
      .expect("the root provides Navigation")
      .workspace()
      .clone();
    let current = project.project.id();
    let load = ctx.future(current, move |current| {
      let workspace = workspace.clone();
      async move {
        background::run(move || data::sources(&workspace, current))
          .await?
          .map_err(|error| {
            tracing::error!(%error, "cannot read the workspace's projects");
            messages::store_error(&error)
          })
      }
    });
    let state = load.state().get();
    match (state.data, state.error) {
      (_, Some(error)) => Err(error),
      (data, None) => Ok(data),
    }
  }

  /// The project chosen to import from: the most recently opened one until the user picks.
  fn chosen(&self, sources: Result<&Option<Sources>, &Message>) -> Option<RegisteredProject> {
    let Ok(Some(Sources(projects))) = sources else {
      return None;
    };
    let SourceChoice(chosen) = self.source.get();
    match chosen.and_then(|id| projects.iter().find(|project| project.id == id)) {
      Some(project) => Some(project.clone()),
      None => {
        let first = projects.first()?;
        self.source.set(SourceChoice(Some(first.id)));
        Some(first.clone())
      }
    }
  }

  /// What the chosen project and this one hold. Data of a project chosen before is not shown
  /// while the chosen one is read.
  fn load(
    &self,
    ctx: &mut Ctx,
    project: &OpenProject,
    chosen: Option<&RegisteredProject>,
  ) -> Result<Option<Catalogs>, Message> {
    let deps = (chosen.cloned(), project.repo_root.clone());
    let load = ctx.future(deps, |(chosen, target)| async move {
      let Some(chosen) = chosen else {
        return Ok(Loaded(None));
      };
      let root = chosen.repo_root.clone();
      background::run(move || data::load(&root, &target))
        .await?
        .map(|catalogs| Loaded(Some(catalogs)))
        .map_err(|error| {
          tracing::error!(%error, source = %chosen.repo_root.display(), "cannot read the projects to import");
          data::load_error(&error, chosen.name.as_str())
        })
    });
    let state = load.state().get();
    let pending = state.is_pending();
    let current = |catalogs: &Catalogs| Some(catalogs.source.project().id()) == chosen.map(|chosen| chosen.id);
    match (state.data, state.error) {
      (_, Some(error)) if !pending => Err(error),
      (Some(Loaded(Some(catalogs))), _) if current(&catalogs) => Ok(Some(catalogs)),
      _ => Ok(None),
    }
  }

  fn plan(&self, catalogs: &Catalogs) -> Plan {
    let ChoiceState(choices) = self.choices.get();
    let Picked(picked) = self.picked.get();
    Plan::new(&catalogs.source, &catalogs.target, &picked, &choices)
  }

  fn tab(&self, props: &ImportProps) -> ImportTab {
    self.tab.get().unwrap_or(props.tab)
  }

  fn close(ctx: &mut Ctx) -> impl Fn() + Send + Sync + 'static {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    move || nav.close_import()
  }
}

/// Title, the two stages and Close (`v635F`).
fn header(ctx: &mut Ctx, title: &str) -> Element {
  let close = ImportDialog::close(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADER_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Sm)
        .child(
          Text::new(title)
            .variant(ExtraTypography::DialogTitle.role())
            .id("import-title"),
        )
        .child(Text::new(&ctx.t("projectImport.stages")).variant(TypographyStyle::Caption)),
    )
    .child(
      ActionButton::new(ctx.t("common.close"))
        .icon(Icon::X)
        .id("import-close")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(close)
        .render(ctx),
    )
    .into()
}

/// The dialog's body (`sxeK1`, `X03Af`): its padding, less the scroll gutter at the right, where
/// the body's viewport keeps its thumb (design doc, "Scroll gutter").
fn body() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding_vertical(ExtraSpacing::Rail.role())
    .padding_left(ExtraSpacing::Rail.role())
    .padding_right(SpacingSize::Md)
}

/// `content` of the body outside its viewport, ending where the viewport's content ends: before
/// the gutter, as wide as a list's side padding (`space-stack`). It does not clip, so the focus
/// ring of a control in it, such as a kind tab, reaches into the body's padding.
fn beside_gutter(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_right(ExtraSpacing::Stack.role())
    .overflow_visible()
    .child(content)
    .into()
}

/// The action bar at the bottom of the dialog (`qSGs5`, `SxdEQ`).
fn footer_bar(children: Vec<Element>) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(children)
    .into()
}

/// The page that shows `copy` once it exists.
fn page_of(copy: &CopiedResource) -> Page {
  let id = copy.resource().id();
  match copy.content() {
    Content::Pipeline(_) => Page::Pipeline(id, PipelineView::Draft),
    Content::AgentProfile(_) => Page::Agents(AgentSelection::Profile(id)),
    Content::Instruction(instruction) => Page::Instructions(tab_of(instruction.scope()), Some(id)),
    Content::Tool(_) => Page::Tools(ToolSelection::Tool(id)),
  }
}

/// Where the status bar's event about `copy` leads.
fn target_of(repo_root: &std::path::Path, copy: &CopiedResource) -> Target {
  let id = copy.resource().id();
  match copy.content() {
    Content::Pipeline(_) => operations::pipeline(repo_root, id),
    Content::AgentProfile(_) => operations::profile(repo_root, id),
    Content::Instruction(_) => operations::instruction(repo_root, id),
    Content::Tool(_) => operations::tool(repo_root, id),
  }
}

/// Writes the batch, reported in the status bar, and names the page of its first copy.
fn import_action(ctx: &mut Ctx) -> ImportAction {
  let activity = crate::activity::of(ctx);
  ctx.future_action(move |(repo_root, batch, spec): ImportArgs| {
    let activity = activity.clone();
    async move {
      let first = batch
        .copies()
        .first()
        .expect("a batch is built only from a plan that copies something");
      let (page, target) = (page_of(first), target_of(&repo_root, first));
      let work = async move {
        background::run(move || data::import_into(&repo_root, &batch))
          .await?
          .map_err(|error| {
            tracing::error!(%error, "cannot import the resources");
            data::import_error(&error)
          })
      };
      activity.track_to(spec, work, move |_| Some(target)).await?;
      Ok(Imported(page))
    }
  })
}
