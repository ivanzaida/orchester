//! *The Orchestrator* in Settings › Automation (`The Orchestrator` `P3S8cC` in `S11` `V8aAj`): one row, *What the
//! Orchestrator may do*, since each project sets its own switches in Project settings › Orchestrator (ORC-599). With a
//! project open (the tab's, as the sidebar shows it) the row's hint names that project's switches that are on, read off
//! the UI thread, and *Project settings →* at its end opens that project's Orchestrator tab; with none open the hint
//! only says each project sets its own and the link is left out. The group's foot says where the switches are set.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureState},
    theme::SpacingSize,
  },
  components::Text,
  layout::Alignment,
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background,
  messages::{self, Message},
  workspace::{OpenProject, OrchestratorConsent},
};

use crate::{
  navigation::Navigation,
  project::Projects,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  services::Services,
  theme::{ExtraColor, ExtraTypography, IconSize},
  ui::{
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
    setting_rows::{SettingGroup, SettingRow},
  },
};

const ID: &str = "settings-orchestrator-may-do";
const LABEL: &str = "settings.automation.orchestrator.label";
const EVERY_PROJECT: &str = "settings.automation.orchestrator.everyProject";

/// The group, under *Approvals and permissions*.
pub fn group(ctx: &mut Ctx) -> Element {
  let row = match open_project(ctx) {
    Some((project, open)) => ctx.mount_keyed::<ProjectRow>(&format!("orchestrator-{project}"), Props { project, open }),
    None => SettingRow::new(ID, ctx.t(LABEL)).hint(ctx.t(EVERY_PROJECT)).render(ctx),
  };
  SettingGroup::new("settings-orchestrator", ctx.t("settings.automation.orchestrator.title"))
    .row(row)
    .foot(&ctx.t("settings.automation.orchestrator.foot"))
    .into()
}

/// The tab's project, while it is open.
fn open_project(ctx: &mut Ctx) -> Option<(ProjectId, OpenProject)> {
  let project = Navigation::of(ctx).tabs().active().sidebar_project()?;
  let open = Projects::of(ctx).state(project)?.open().cloned()?;
  Some((project, open))
}

#[derive(Clone, PartialEq)]
struct Props {
  project: ProjectId,
  open: OpenProject,
}

impl DevtoolsInspectable for Props {}

/// The project's switches that are on, in the order Project settings › Orchestrator shows them.
#[derive(Clone, Debug, PartialEq)]
struct Allowed(Arc<[OrchestratorConsent]>);

impl DevtoolsInspectable for Allowed {}

/// The key of what the hint says of a switch that is on.
const fn does(consent: OrchestratorConsent) -> &'static str {
  match consent {
    OrchestratorConsent::PublishDrafts => "settings.automation.orchestrator.does.publishDrafts",
    OrchestratorConsent::AnswerAgents => "settings.automation.orchestrator.does.answerAgents",
    OrchestratorConsent::MergeRuns => "settings.automation.orchestrator.does.mergeRuns",
    OrchestratorConsent::MergeFinished => "settings.automation.orchestrator.does.mergeFinished",
    OrchestratorConsent::ContinueInterrupted => "settings.automation.orchestrator.does.continueInterrupted",
    OrchestratorConsent::WriteGit => "settings.automation.orchestrator.does.writeGit",
  }
}

struct ProjectRow;

impl Component for ProjectRow {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, open } = ctx.props::<Props>().clone();
    let read = allowed(ctx, &open);
    let name = open.project.name().as_str();
    // Until the switches are read, or when they cannot be, the hint says only what holds for every project.
    let hint = match &read.data {
      Some(Allowed(on)) if on.is_empty() => ctx.t_args("settings.automation.orchestrator.noneOn", [("project", name)]),
      Some(Allowed(on)) => {
        let separator = ctx.t("settings.automation.orchestrator.listSeparator");
        let switches = on
          .iter()
          .map(|consent| ctx.t(does(*consent)).to_string())
          .collect::<Vec<_>>()
          .join(&separator);
        ctx.t_args(
          "settings.automation.orchestrator.someOn",
          [("project", name), ("switches", &switches)],
        )
      }
      None => ctx.t(EVERY_PROJECT),
    };
    let problem = read.error.map(|reason| {
      let reason = message_text(ctx, &reason);
      Element::from(field_error(
        &format!("{ID}-error"),
        &ctx.t_args(
          "settings.automation.orchestrator.unreadable",
          [("project", name), ("reason", &*reason)],
        ),
      ))
    });
    SettingRow::new(ID, ctx.t(LABEL))
      .hint(hint)
      .control(link(ctx, project))
      .below_all(problem)
      .render(ctx)
  }
}

/// *Project settings* and its arrow in `accent_text` (`Link` `r4ecz`): the project's Project settings › Orchestrator.
fn link(ctx: &mut Ctx, project: ProjectId) -> Element {
  let words = ctx.t("settings.automation.orchestrator.open");
  let route = Route::Project(project, ProjectPage::Settings(ProjectSettingsTab::Orchestrator));
  opening(Navigation::of(ctx).to(route))
    .id(format!("{ID}-open"))
    .describe("a11y_name", &*words)
    .flex_shrink(0.0)
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(
      Text::new(&words)
        .id(format!("{ID}-open-text"))
        .variant(ExtraTypography::Lead.role())
        .color(ExtraColor::AccentText.role())
        .nowrap(),
    )
    .child(icon(Icon::ArrowRight, IconSize::Base, ExtraColor::AccentText.role()))
    .into()
}

/// The switches of the project `open` that are on, as stored.
fn allowed(ctx: &mut Ctx, open: &OpenProject) -> FutureState<Allowed, Message> {
  let services = Services::of(ctx);
  let (workspace, hold, root) = (services.workspace, services.hold, open.repo_root.clone());
  ctx
    .future((), move |()| {
      let (workspace, hold, root) = (workspace.clone(), hold.clone(), root.clone());
      async move {
        hold.released().await;
        background::run(move || {
          OrchestratorConsent::ALL
            .into_iter()
            .map(|consent| {
              workspace
                .orchestrator_allows(&root, consent)
                .map(|on| on.then_some(consent))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|on| Allowed(on.into_iter().flatten().collect()))
        })
        .await?
        .map_err(|error| {
          tracing::error!(%error, "what a project's Orchestrator may do cannot be read");
          messages::store_error(&error)
        })
      }
    })
    .state()
    .get()
}
