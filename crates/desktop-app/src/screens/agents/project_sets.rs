//! *Set by the project* under Connection enforcement (`Set by the project` `EqldX` in S14 a `o9GsNS`,
//! `docs/design/settings.md`): what the project decides for every profile, read-only — Autopilot, Sources, the
//! repository's agent files, the code index, Memory, Environment and Limits — each row opening the Project settings
//! tab that changes it. Read off the UI thread with the reads those tabs use; a read that fails says so in its row.
use std::path::Path;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{limits::ProjectLimits, memory::MemoryLimits, tokens::compact_tokens};
use orchester_workspace::{
  background,
  budget::whole_minutes,
  environment, memory,
  messages::{self, Message},
  project_settings::{self, Settings},
  sources,
  tasks::outside::Outside,
};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, ProjectSettingsTab, Route, SecretsView},
  screens::hub::parts::{Entry, boxed, group, head, note},
  ui::{icon::Icon, message::message_text, pressable::Press},
};

#[derive(Clone, PartialEq)]
pub struct Props {
  pub project: ShownProject,
}
impl DevtoolsInspectable for Props {}

/// What the rows show, each read on its own: one that fails leaves the others.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read {
  settings: Result<Settings, Message>,
  /// How many sources the project lists.
  sources: Result<usize, Message>,
  memory: Result<MemoryLimits, Message>,
  /// How many variables the project's environment adds.
  environment: Result<usize, Message>,
}
impl DevtoolsInspectable for Read {}

impl Read {
  fn load(root: &Path) -> Self {
    Self {
      settings: project_settings::load_in(root).map_err(|e| messages::store_error(&e)),
      sources: sources::project::load_in(root)
        .map(|listed| listed.len())
        .map_err(|e| messages::store_error(&e)),
      memory: memory::settings::load_in(root)
        .map(|settings| settings.limits)
        .map_err(|e| messages::store_error(&e)),
      environment: environment::load_in(root)
        .map(|variables| variables.iter().count())
        .map_err(|e| messages::store_error(&e)),
    }
  }
}

pub struct ProjectSets;

impl Component for ProjectSets {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<Props>().project.clone();
    let read = read(ctx, &project);
    let navigation = Navigation::of(ctx);
    let id = project.open.project.id();
    let settings = |tab| Press::Open(navigation.to(Route::Project(id, ProjectPage::Settings(tab))));
    let environment = Press::Open(navigation.to(Route::Project(id, ProjectPage::Secrets(SecretsView::default()))));
    let (runs, context) = (ProjectSettingsTab::Runs, ProjectSettingsTab::Context);
    let rows = [
      ("autopilot", settings(runs)),
      ("sources", settings(context)),
      ("agentFiles", settings(context)),
      ("codeIndex", settings(context)),
      ("memory", settings(context)),
      ("environment", environment),
      ("limits", settings(runs)),
    ]
    .map(|(key, press)| {
      let hint = read.as_ref().map(|read| hint(read, key));
      row(ctx, (key, project.open.project.name().as_str()), hint, press)
    });
    Column::new()
      .id("agent-project-sets")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(group(
        "agent-project-sets-group",
        head(
          "agent-project-sets-group",
          &ctx.t("agents.projectSets.title"),
          None,
          None,
        ),
        boxed("agent-project-sets-rows", rows),
      ))
      .child(note(
        "agent-project-sets-note",
        Icon::Info,
        &ctx.t("agents.projectSets.note"),
      ))
  }
}

/// The project's settings as read last, `None` before the first read ends. Changes made outside the screens to what
/// the rows show read them again.
fn read(ctx: &mut Ctx, project: &ShownProject) -> Option<Read> {
  let outside = project.feed.outside.get();
  let dep = [
    Outside::AutoMode,
    Outside::Limits,
    Outside::CodeIndex,
    Outside::ProjectSettings,
  ]
  .map(|kind| outside.of(kind));
  let hold = crate::services::Services::of(ctx).hold;
  let state = ctx
    .future((project.open.repo_root.clone(), dep), move |(root, _)| {
      tracing::trace!(target: crate::app_log::READ_LOG, read = "project sets");
      let hold = hold.clone();
      async move {
        hold.released().await;
        background::run(move || Read::load(&root)).await
      }
    })
    .state()
    .get();
  // The read itself could not run: every row says why.
  state.data.or_else(|| {
    state.error.map(|error| Read {
      settings: Err(error.clone()),
      sources: Err(error.clone()),
      memory: Err(error.clone()),
      environment: Err(error),
    })
  })
}

/// What a row's hint says: its key and the values it names, or why its setting could not be read.
type Hint = Result<(&'static str, Vec<(&'static str, String)>), Message>;

/// The hint of the row named `key`.
fn hint(read: &Read, key: &str) -> Hint {
  let plain = |key| (key, Vec::new());
  match key {
    "autopilot" => read.settings.as_ref().map(|s| {
      plain(if s.allow_autopilot.is_on() {
        "agents.projectSets.autopilot.allowed"
      } else {
        "agents.projectSets.autopilot.notAllowed"
      })
    }),
    "sources" => read.sources.as_ref().map(|count| match *count {
      0 => plain("agents.projectSets.sources.none"),
      count => ("agents.projectSets.sources.some", vec![("count", count.to_string())]),
    }),
    "agentFiles" => read.settings.as_ref().map(|s| {
      plain(if s.agent_files.is_on() {
        "agents.projectSets.agentFiles.on"
      } else {
        "agents.projectSets.agentFiles.off"
      })
    }),
    "codeIndex" => read.settings.as_ref().map(|s| {
      plain(if s.code_index.is_on() {
        "agents.projectSets.codeIndex.on"
      } else {
        "agents.projectSets.codeIndex.off"
      })
    }),
    "memory" => read.memory.as_ref().map(|limits| memory(*limits)),
    "environment" => read.environment.as_ref().map(|count| match *count {
      0 => plain("agents.projectSets.environment.none"),
      1 => plain("agents.projectSets.environment.one"),
      count => (
        "agents.projectSets.environment.other",
        vec![("count", count.to_string())],
      ),
    }),
    _ => read.settings.as_ref().map(|s| limits(s.limits)),
  }
  .map_err(Clone::clone)
}

fn memory(limits: MemoryLimits) -> (&'static str, Vec<(&'static str, String)>) {
  let key = if limits.records() == 1 {
    "agents.projectSets.memory.one"
  } else {
    "agents.projectSets.memory.other"
  };
  (
    key,
    vec![
      ("records", limits.records().to_string()),
      ("tokens", compact_tokens(u64::from(limits.tokens()))),
    ],
  )
}

/// A new run's budget and time limit, as Runs › Limits sets them.
fn limits(limits: ProjectLimits) -> (&'static str, Vec<(&'static str, String)>) {
  let budget = limits.default_run_budget().map(|budget| compact_tokens(budget.get()));
  let minutes = limits
    .default_time_limit()
    .map(|limit| whole_minutes(limit).to_string());
  let key = match (&budget, &minutes) {
    (None, None) => "agents.projectSets.limits.none",
    (Some(_), None) => "agents.projectSets.limits.budget",
    (None, Some(_)) => "agents.projectSets.limits.time",
    (Some(_), Some(_)) => "agents.projectSets.limits.both",
  };
  (
    key,
    vec![
      ("budget", budget.unwrap_or_default()),
      ("minutes", minutes.unwrap_or_default()),
    ],
  )
}

/// The row named `key` in `project` saying `hint`, *Reading…* while it is `None`.
fn row(ctx: &Ctx, (key, project): (&str, &str), hint: Option<Hint>, press: Press) -> Element {
  let entry = Entry::new(
    format!("agent-project-sets-{key}"),
    ctx.t(&format!("agents.projectSets.{key}.title")),
  );
  let entry = match hint {
    None => entry.hint(ctx.t("agents.projectSets.reading")),
    Some(Ok((said, mut args))) => {
      args.push(("project", project.to_owned()));
      entry.hint(ctx.t_args(said, args))
    }
    Some(Err(error)) => entry.hint_in(
      ctx.t_args("agents.projectSets.failed", [("reason", message_text(ctx, &error))]),
      PaletteColor::Warning,
    ),
  };
  entry.press(press).elsewhere().into()
}
