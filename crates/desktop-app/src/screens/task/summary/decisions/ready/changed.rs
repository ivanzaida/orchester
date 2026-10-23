//! What the run changed and what verified it, side by side (`Columns` `fwsep`). *What changed* (`Z5Kagi`):
//! the files of its diff from where its own work starts (its base, or the newer commit of its base branch its branch
//! took in) to its latest checkpoint with their added and removed lines
//! (`Changed file row` `dAwTi`), the first few of them and *Open in Changes*. *Verification* (`Jkat7`):
//! the commit its work ended at and the steps that passed after its work was first committed, each with
//! what it reported (`Check` `KBTiJ`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  diff::{FileChange, FileDiff},
  resource::{RepositoryAccess, StepKind},
  run::StepStatus,
};
use orchester_workspace::{
  background, display,
  messages::Message,
  tasks::{
    changes::{Checkpointed, RunRange, run_changes_in},
    detail::{RunView, StepView},
  },
};

use super::super::{TaskContext, step_label, tab_link};
use crate::{
  routes::TaskTab,
  services::Services,
  theme::{DecisionSpacing, DecisionText, ExtraSpacing, ExtraTypography, IconSize, TaskText},
  ui::{
    card::{CardSurface, card},
    icon::{Icon, icon},
    message::message_text,
  },
};

/// Files listed before *Open in Changes* (`Files` `Xxm0S`).
const SHOWN_FILES: usize = 5;
/// A file's row, and the width of how it changed (`dAwTi`, `F1t7T`).
const FILE_ROW_HEIGHT: f32 = 30.0;
const KIND_WIDTH: f32 = 12.0;

/// The run's checkpointed changes as read.
#[derive(Clone, Debug, PartialEq)]
pub struct Read(Checkpointed);

impl lurq::app::component::DevtoolsInspectable for Read {}

/// The run's checkpointed changes from where its own work starts, read off the UI thread once per base and
/// checkpoint; `None` while they are read.
pub fn read(ctx: &mut Ctx, task: &TaskContext) -> Option<Result<Checkpointed, Message>> {
  let programs = Services::of(ctx).programs;
  let range = task.detail.run.as_ref().map(|run| RunRange::of(&run.run));
  let deps = (task.project.open.repo_root.clone(), range);
  let load = ctx.future(deps, move |(root, range)| {
    let programs = programs.clone();
    async move {
      let Some(range) = range else {
        return Ok(Read(Checkpointed::NoCheckpoint));
      };
      background::run(move || run_changes_in(&root, &programs, &range))
        .await?
        .map(Read)
        .map_err(|error| {
          tracing::warn!(%error, "cannot read the run's changes");
          error.message()
        })
    }
  });
  let state = load.state().get();
  // A reading that failed keeps the data read before it: the failure is what stands.
  match (state.data, state.error) {
    (_, Some(error)) => Some(Err(error)),
    (Some(Read(read)), None) => Some(Ok(read)),
    (None, None) => None,
  }
}

/// *What changed* and *Verification* side by side; `delivered` are the rows the run's delivery adds under its
/// steps' checks, `head` the figure at *Verification*'s end in place of the commit the run's work ended at.
pub fn columns(
  ctx: &mut Ctx,
  task: &TaskContext,
  read: Option<&Result<Checkpointed, Message>>,
  (delivered, head): (Vec<Element>, Option<Arc<str>>),
) -> Element {
  let run = task.detail.run.as_ref();
  let verified = run.map(|run| verification(ctx, run, (delivered, head)));
  Row::new()
    .id("task-ready-columns")
    .width(Dimension::Pct(100.0))
    .spacing(DecisionSpacing::Columns.role())
    .align_items(Alignment::Start)
    .child(Column::new().flex(1.0).child(what_changed(ctx, task, read)))
    .with_children(verified.map(|verified| Column::new().flex(1.0).child(verified)))
    .into()
}

/// "4 files · +120 / −18" of what was read, once it was.
pub(in super::super) fn size(ctx: &Ctx, read: Option<&Result<Checkpointed, Message>>) -> Option<Arc<str>> {
  match read {
    Some(Ok(Checkpointed::Read { changes, .. })) => Some(size_line(
      ctx,
      changes.diff.files().len(),
      (changes.diff.additions(), changes.diff.deletions()),
    )),
    Some(Ok(Checkpointed::TooLarge { size, .. })) => Some(size_line(
      ctx,
      usize::try_from(size.files).unwrap_or(usize::MAX),
      (size.additions, size.deletions),
    )),
    _ => None,
  }
}

fn what_changed(ctx: &mut Ctx, task: &TaskContext, read: Option<&Result<Checkpointed, Message>>) -> Element {
  let size = size(ctx, read);
  let body: Element = match read {
    None => note(&ctx.t("decisions.ready.reading")),
    Some(Err(error)) => note(&message_text(ctx, error)),
    Some(Ok(Checkpointed::NoCheckpoint)) => note(&ctx.t("decisions.ready.noChanges")),
    Some(Ok(Checkpointed::TooLarge { .. })) => files_card(ctx, task, &[]),
    Some(Ok(Checkpointed::Read { changes, .. })) => files_card(ctx, task, changes.diff.files()),
  };
  section(ctx, ("task-ready-changes", "decisions.ready.whatChanged"), size, body)
}

/// "4 files · +120 / −18".
fn size_line(ctx: &Ctx, files: usize, (added, removed): (u32, u32)) -> Arc<str> {
  let key = if files == 1 {
    "decisions.ready.size.one"
  } else {
    "decisions.ready.size.other"
  };
  ctx.t_args(
    key,
    [
      ("files", files.to_string()),
      ("added", added.to_string()),
      ("removed", removed.to_string()),
    ],
  )
}

/// The first files and *Open in Changes* (`Files` `Xxm0S`).
fn files_card(ctx: &mut Ctx, task: &TaskContext, files: &[FileDiff]) -> Element {
  let link = Row::new()
    .padding(
      Padding::new()
        .top(ExtraSpacing::Snug.role())
        .left(ExtraSpacing::Loose.role()),
    )
    .child(tab_link(
      ctx,
      task,
      ("task-ready-open-changes", "decisions.ready.openInChanges"),
      TaskTab::Changes,
    ));
  card("task-ready-files", CardSurface::Outline)
    .padding(ExtraSpacing::Snug.role())
    .with_children(files.iter().take(SHOWN_FILES).map(file_row))
    .child(link)
    .into()
}

fn file_row(file: &FileDiff) -> Element {
  let kind = match file.change() {
    FileChange::Added => "A",
    FileChange::Deleted => "D",
    FileChange::Renamed { .. } => "R",
    _ => "M",
  };
  Row::new()
    .id(format!("task-ready-file-{}", file.path()))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FILE_ROW_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(
      Text::new(kind)
        .variant(DecisionText::ChangeKind.role())
        .width(Dimension::Px(KIND_WIDTH)),
    )
    .child(Text::new(file.path()).variant(TypographyStyle::Mono).flex(1.0).nowrap())
    .child(count(&format!("+{}", file.additions()), PaletteColor::Success))
    .child(count(&format!("\u{2212}{}", file.deletions()), PaletteColor::Danger))
    .into()
}

fn count(text: &str, color: PaletteColor) -> Element {
  Text::new(text)
    .variant(ExtraTypography::MonoNote.role())
    .color(color)
    .into()
}

/// *Verification*: the commit the run's work ended at (or `head`), the steps that passed after the work was
/// first committed, and what its delivery adds (`delivered`).
fn verification(ctx: &Ctx, run: &RunView, (delivered, head): (Vec<Element>, Option<Arc<str>>)) -> Element {
  let revision = head.or_else(|| {
    run.run.last_checkpoint().map(|checkpoint| {
      ctx.t_args(
        "decisions.ready.commit",
        [("revision", display::short_revision(checkpoint.revision().as_str()))],
      )
    })
  });
  let checks: Vec<Element> = verifying_steps(run)
    .into_iter()
    .map(|step| check(ctx, run, step))
    .chain(delivered)
    .collect();
  let body = if checks.is_empty() {
    note(&ctx.t("decisions.ready.noChecks"))
  } else {
    card("task-ready-checks", CardSurface::Outline)
      .padding(Padding::symmetric(
        ExtraSpacing::Loose.role(),
        ExtraSpacing::Snug.role(),
      ))
      .with_children(checks)
      .into()
  };
  section(
    ctx,
    ("task-ready-verification", "decisions.ready.verification"),
    revision,
    body,
  )
}

/// The steps that passed after the first step whose agent writes to the repository, the one that made the
/// work; every step that passed in a pipeline without one.
fn verifying_steps(run: &RunView) -> Vec<&StepView> {
  let body = run.run.snapshot().body();
  let writes = |step: &&StepView| {
    body.step(&step.key).is_some_and(|pinned| {
      matches!(
        pinned.kind,
        StepKind::Agent {
          repository: RepositoryAccess::Write,
          ..
        }
      )
    })
  };
  let after = run.steps.iter().find(writes).map_or(0, |step| step.number);
  run
    .steps
    .iter()
    .filter(|step| step.number > after && step.status == Some(StepStatus::Passed))
    .collect()
}

/// A step that passed with the first line of what it reported (`Check` `KBTiJ`).
fn check(ctx: &Ctx, run: &RunView, step: &StepView) -> Element {
  let said = step
    .execution
    .and_then(|execution| run.reported(execution))
    .and_then(|text| text.lines().map(str::trim).find(|line| !line.is_empty()))
    .map_or_else(|| ctx.t("decisions.ready.passed"), Arc::from);
  Row::new()
    .id(format!("task-ready-check-{}", step.key))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Fine.role(), ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(Icon::CircleCheck, IconSize::Base, PaletteColor::Success))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Hair.role())
        .child(Text::new(&step_label(ctx, step)).variant(TypographyStyle::Label))
        .child(
          Text::new(&said)
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::TextSecondary)
            .nowrap(),
        ),
    )
    .into()
}

/// A section with its title, a figure at its end, and its body.
pub(in super::super) fn section(ctx: &Ctx, (id, title): (&str, &str), figure: Option<Arc<str>>, body: Element) -> Element {
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t(title)).variant(TaskText::SectionTitle.role()))
    .child(Spacer::new().flex(1.0))
    .with_children(figure.map(|figure| {
      Text::new(&figure)
        .id(format!("{id}-figure"))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary)
        .nowrap()
    }));
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(head)
    .child(body)
    .into()
}

pub(in super::super) fn note(text: &str) -> Element {
  Text::new(text)
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
    .into()
}
