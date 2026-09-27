//! Instructions (05.01 `yRKeF`): the project's rules by scope tab (Project rules, Agent roles,
//! Pipeline overrides, Step overrides) beside the selected rule's editor and the preview of the
//! prompt it takes part in (`RGk5C`), with the resolved prompt (05.02 `fZa3J`) and the variable
//! catalog (05.03 `EfTiT`, 05.A `m2WjL`) as dialogs. Each instruction follows Draft saved →
//! Publish vN → Published vN; drafts are saved as they are edited.

mod copy;
mod editor;
mod fields;
mod frame;
mod library;
mod new_dialog;
mod preview;
mod variables;
mod viewer;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::{Column, Modal, ModalTarget},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ResourceId;

use self::{
  editor::InstructionEditor,
  frame::{Frame, frame},
  library::{Library, library},
  new_dialog::{NewInstructionDialog, NewInstructionProps},
};
use super::ProjectNav;
use crate::{
  background,
  instructions::data::{InstructionEntry, ProjectInstructions},
  messages::{self, Message},
  navigation::route::{InstructionsTab, Page, Route},
  ui::{backdrop::backdrop, button::ActionButton, icon::Icon, message_text, panel::RecoveryPanel},
  workspace::OpenProject,
};

/// Provided to the editor when it is mounted: what it edits and what it chooses from.
#[derive(Clone)]
pub struct EditorContext {
  pub project: OpenProject,
  pub data: ProjectInstructions,
  pub entry: InstructionEntry,
  pub tab: InstructionsTab,
  pub nav: ProjectNav,
  /// Bumped after a publish, to read the instructions again.
  pub revision: Signal<u64>,
  /// Whether the New instruction dialog is open.
  pub creating: Signal<bool>,
}

/// The instructions, and the revision they were read at.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Loaded {
  revision: u64,
  data: ProjectInstructions,
}

impl lurq::app::component::DevtoolsInspectable for Loaded {}

pub struct InstructionsScreen {
  revision: Signal<u64>,
  creating: Signal<bool>,
  /// Counts dialog openings, so every opening starts with an empty form.
  opened: Signal<u64>,
  nav: ProjectNav,
}

impl Component for InstructionsScreen {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let creating = ctx.signal(false);
    let opened = ctx.signal(0_u64);
    let count = opened.clone();
    ctx.watch(&creating, move |open| {
      if *open {
        count.update(|opened| *opened += 1);
      }
    });
    Self {
      revision: ctx.signal(0),
      creating,
      opened,
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    let deps = (open.repo_root.clone(), self.revision.get());
    let load = ctx.future(deps, |(repo_root, revision)| async move {
      background::run(move || ProjectInstructions::load_in(&repo_root))
        .await?
        .map(|data| Loaded { revision, data })
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's instructions");
          messages::store_error(&error)
        })
    });
    let (tab, selected) = match Route::current_page(ctx) {
      Some(Page::Instructions(tab, selected)) => (tab, selected),
      _ => (InstructionsTab::ProjectRules, None),
    };
    let state = load.state().get();
    let data = state.data.as_ref().map(|loaded| loaded.data.clone());
    let content = match (state.data, state.error) {
      (_, Some(error)) => self.page(ctx, &open, tab, None, failed(ctx, &error)),
      (None, None) => self.page(ctx, &open, tab, None, Column::new().into()),
      (Some(loaded), None) => self.loaded(ctx, &open, loaded, tab, selected),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(content)
      .child(self.dialog(ctx, &open, tab, data))
  }
}

impl InstructionsScreen {
  fn loaded(
    &self,
    ctx: &mut Ctx,
    open: &OpenProject,
    loaded: Loaded,
    tab: InstructionsTab,
    selected: Option<ResourceId>,
  ) -> Element {
    let in_tab = |entry: &&InstructionEntry| copy::tab_of(entry.body.scope()) == tab;
    let entry = selected
      .and_then(|id| loaded.data.entry(id))
      .or_else(|| loaded.data.entries.iter().find(in_tab))
      .cloned();
    let Some(entry) = entry else {
      let empty = empty(ctx, tab, &loaded.data, &self.creating);
      return self.page(ctx, open, tab, Some(&loaded.data), empty);
    };
    // A rule opened from elsewhere (the status bar) shows its own scope's tab.
    let tab = copy::tab_of(entry.body.scope());
    let key = format!("instruction-editor-{}-{}", entry.id, loaded.revision);
    ctx.provide(EditorContext {
      project: open.clone(),
      data: loaded.data,
      entry,
      tab,
      nav: self.nav.clone(),
      revision: self.revision.clone(),
      creating: self.creating.clone(),
    });
    ctx.mount_keyed::<InstructionEditor>(&key, ())
  }

  /// The page without an editor: loading, failed, or no rules in the tab yet.
  fn page(
    &self,
    ctx: &Ctx,
    open: &OpenProject,
    tab: InstructionsTab,
    data: Option<&ProjectInstructions>,
    content: Element,
  ) -> Element {
    let project = open.project.name().as_str();
    let list = data.map(|data| {
      library(
        ctx,
        &Library {
          project,
          data,
          tab,
          edited: None,
          nav: &self.nav,
          creating: &self.creating,
        },
      )
    });
    let body = Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .child(content);
    frame(
      ctx,
      Frame {
        project: open,
        tab,
        nav: &self.nav,
        trailing: Vec::new(),
        library: list,
        body: body.into(),
      },
    )
  }

  fn dialog(
    &self,
    ctx: &mut Ctx,
    open: &OpenProject,
    tab: InstructionsTab,
    data: Option<ProjectInstructions>,
  ) -> Element {
    let Some(data) = data.filter(|_| self.creating.get()) else {
      return Column::new().into();
    };
    let props = NewInstructionProps {
      project: open.clone(),
      tab,
      data,
      open: self.creating.clone(),
      revision: self.revision.clone(),
    };
    let key = format!("new-instruction-{}", self.opened.get());
    Modal::new(backdrop(ctx.mount_keyed::<NewInstructionDialog>(&key, props)))
      .target(ModalTarget::Root)
      .open(self.creating.clone())
      .into()
  }
}

fn failed(ctx: &Ctx, error: &Message) -> Element {
  RecoveryPanel::new(
    Icon::AlertCircle,
    &ctx.t("instructions.loadFailed"),
    &message_text(ctx, error),
  )
  .into()
}

/// No rules of the tab's scope yet. Agent role rules need a profile and overrides a pipeline; the
/// state says so instead of offering a rule that could not apply anywhere.
fn empty(ctx: &Ctx, tab: InstructionsTab, data: &ProjectInstructions, creating: &Signal<bool>) -> Element {
  let (title, detail) = match tab {
    InstructionsTab::ProjectRules => ("instructions.empty.projectTitle", "instructions.empty.projectDetail"),
    InstructionsTab::AgentRoles => ("instructions.empty.rolesTitle", "instructions.empty.rolesDetail"),
    InstructionsTab::PipelineOverrides => (
      "instructions.empty.pipelinesTitle",
      "instructions.empty.pipelinesDetail",
    ),
    InstructionsTab::StepOverrides => ("instructions.empty.stepsTitle", "instructions.empty.stepsDetail"),
  };
  let panel = RecoveryPanel::new(Icon::BookOpen, &ctx.t(title), &ctx.t(detail));
  match library::missing(tab, data) {
    Some(missing) => panel
      .detail(lurq::components::Text::new(&ctx.t(missing)).variant(crate::theme::ExtraTypography::Explanation.role())),
    None => {
      let creating = creating.clone();
      let action = ActionButton::new(ctx.t("instructions.new"))
        .primary()
        .icon(Icon::Plus)
        .id("empty-new-instruction")
        .on_click(move || creating.set(true))
        .render(ctx);
      panel.action(action)
    }
  }
  .into()
}
