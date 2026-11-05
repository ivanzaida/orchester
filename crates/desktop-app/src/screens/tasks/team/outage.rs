//! A team project's Tasks list while its team data cannot be read (`06 · Tasks · team data unavailable` `GR3RS`): the
//! state's card under the header (`State · team data unavailable` `l6EMj`: a 36 tile with the 18 `cloud-off`, the title
//! in 15/600, why in 12.5 `text_secondary` naming the server by its host, *Try again*), 40 under the header; then, 24
//! under it, what runs on this computer (`On this computer` `XTBXg`: its heading and count, a 40 high row per run with
//! its step and run number and how many of its work reports wait in the outbox to be sent, the line saying runs keep
//! going), read from this computer's runs, never from the team's tasks. The same card says why when this computer is
//! signed out of the server, the person is no longer a member, or the server refused the read.

use std::path::{Path, PathBuf};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::RunId, run::TaskPhase};
use orchester_store::{ProjectDb, WorkReportState};
use orchester_workspace::{
  background,
  tasks::{
    data::ProjectTasks,
    list::{Latest, LatestRun},
  },
};

use super::super::shared::Shared;
use crate::{
  screens::settings::orchester::moment,
  team::TeamFailure,
  theme::{ExtraSpacing, ExtraTypography, HubText, IconSize, TasksSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    state::EmptyState,
  },
};

/// A run's row (`KON-12` `FNPd2`).
const ROW_HEIGHT: f32 = 40.0;

/// The card and what runs on this computer, in the project's checkout at `root`; `retry` reads the team's data again.
pub(in crate::screens::tasks) fn outage(
  ctx: &mut Ctx,
  (failure, shared, project): (&TeamFailure, Option<&Shared>, &str),
  (root, local): (&Path, &ProjectTasks),
  retry: impl Fn() + Send + Sync + 'static,
) -> Column {
  let server = shared.map_or_else(String::new, host);
  let server = server.as_str();
  let (title, body) = match failure {
    TeamFailure::Unavailable { since } => (
      ctx.t("tasks.team.unavailable.title"),
      ctx.t_args(
        "tasks.team.unavailable.body",
        [("server", server.to_owned()), ("time", moment(ctx, *since))],
      ),
    ),
    TeamFailure::SignedOut => (
      ctx.t("tasks.team.unavailable.title"),
      ctx.t_args("tasks.team.signedOut", [("server", server), ("project", project)]),
    ),
    TeamFailure::NotMember => (
      ctx.t("tasks.team.unavailable.title"),
      ctx.t_args("tasks.team.notMember", [("server", server), ("project", project)]),
    ),
    TeamFailure::Refused => (
      ctx.t("tasks.team.refused.title"),
      ctx.t_args("tasks.team.refused.body", [("server", server)]),
    ),
  };
  let again = ActionButton::new("tasks-team-try-again", ButtonKind::Primary, ctx.t("common.tryAgain"))
    .icon(Icon::RefreshCw)
    .on_click(retry);
  let card = EmptyState::new("tasks-team-unavailable", Icon::CloudOff, title, body)
    .wide()
    .action(again);
  Column::new()
    .id("tasks-team-outage")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(TasksSpacing::OutageTop.role()))
    .spacing(SpacingSize::Lg)
    .describe("state", "unavailable")
    .child(card)
    .with_children(running(ctx, root, local))
}

/// The server as the outage names it: its host, with the port when it is not the scheme's (`orchester.acme.dev`); its
/// name once it is no longer in the list.
fn host(shared: &Shared) -> String {
  shared.sharing.server.as_ref().map_or_else(
    || shared.sharing.server_name().to_owned(),
    |(_, address)| address.authority(),
  )
}

/// *Running on this computer* with each run at work here and the line saying runs keep going; `None` without one.
fn running(ctx: &mut Ctx, root: &Path, local: &ProjectTasks) -> Option<Column> {
  let at_work: Vec<_> = local
    .tasks
    .iter()
    .filter_map(|task| match local.phases.get(&task.id()) {
      Some(Latest::Run(latest)) if matches!(latest.phase, TaskPhase::Running | TaskPhase::Waiting) => {
        Some((task, latest))
      }
      _ => None,
    })
    .collect();
  if at_work.is_empty() {
    return None;
  }
  let waiting = waiting_reports(ctx, root, at_work.iter().map(|(_, latest)| latest.id).collect());
  let rows: Vec<Element> = at_work
    .iter()
    .map(|(task, latest)| {
      let reports = waiting
        .iter()
        .find(|(run, _)| *run == latest.id)
        .map_or(0, |(_, count)| *count);
      let key = task.key().to_string();
      row(ctx, (&key, task.details().title.as_str()), latest, reports)
    })
    .collect();
  let heading = Row::new()
    .id("tasks-team-running")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("tasks.team.running")).variant(ExtraTypography::Strong.role()))
    .child(
      Text::new(&rows.len().to_string())
        .id("tasks-team-running-count")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    );
  let note = Row::new()
    .id("tasks-team-outage-note")
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("tasks.team.outageNote"))
        .variant(HubText::TeamLine.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(heading)
      .with_children(rows)
      .child(note),
  )
}

/// One run at work here: its loader, key, title, where the run is (`03 Implement · run 8`) and, while some wait,
/// how many of its work reports wait to be sent (`Reports` `kJpXu`: a 13 `cloud-upload` 6 from the words, in
/// `warning`).
fn row(ctx: &Ctx, (key, title): (&str, &str), latest: &LatestRun, reports: usize) -> Element {
  let run = latest.number.get().to_string();
  let step = latest.step.as_ref().map_or_else(
    || ctx.t_args("tasks.team.run", [("number", run.clone())]),
    |step| {
      ctx.t_args(
        "tasks.team.runStep",
        [
          ("number", format!("{:02}", step.number)),
          ("step", step.name.to_string()),
          ("run", run.clone()),
        ],
      )
    },
  );
  let waiting = (reports > 0).then(|| {
    let words = match reports {
      1 => ctx.t("tasks.team.reportsWaiting.one"),
      count => ctx.t_args("tasks.team.reportsWaiting.other", [("count", count.to_string())]),
    };
    Row::new()
      .id(format!("tasks-team-running-{key}-reports"))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(icon(Icon::CloudUpload, IconSize::Base, PaletteColor::Warning))
      .child(
        Text::new(&words)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::Warning)
          .nowrap(),
      )
  });
  Row::new()
    .id(format!("tasks-team-running-{key}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::LoaderCircle, IconSize::Md, PaletteColor::TextSecondary))
    .child(Text::new(key).variant(ExtraTypography::MonoKey.role()).nowrap())
    .child(Text::new(title).variant(ExtraTypography::Strong.role()).nowrap())
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&step)
        .id(format!("tasks-team-running-{key}-step"))
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .with_children(waiting)
    .into()
}

/// How many work reports of each run wait in the outbox to be sent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct WaitingReports(Vec<(RunId, usize)>);

impl lurq::app::component::DevtoolsInspectable for WaitingReports {}

/// How many work reports of each of `runs` wait in the outbox of the project at `root`, read off the render thread;
/// none while they are read. Reports that cannot be read are left unsaid: the rows still say which runs go on.
fn waiting_reports(ctx: &mut Ctx, root: &Path, runs: Vec<RunId>) -> Vec<(RunId, usize)> {
  ctx
    .future(
      (root.to_owned(), runs),
      |(root, runs): (PathBuf, Vec<RunId>)| async move {
        match background::run(move || pending(&root, &runs)).await {
          Ok(Ok(waiting)) => Ok(waiting),
          Ok(Err(error)) => {
            tracing::warn!(%error, "the work reports waiting to be sent cannot be read");
            Err(())
          }
          Err(_) => Err(()),
        }
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_default()
    .0
}

/// The pending reports of each of `runs`; a run that is no team run has none.
fn pending(root: &Path, runs: &[RunId]) -> Result<WaitingReports, orchester_store::StoreError> {
  let db = ProjectDb::open(root)?;
  let counted = runs.iter().map(|&run| {
    let reports = db.work_reports(run)?;
    let waiting = reports
      .iter()
      .filter(|report| report.state == WorkReportState::Pending)
      .count();
    Ok((run, waiting))
  });
  counted.collect::<Result<_, _>>().map(WaitingReports)
}
