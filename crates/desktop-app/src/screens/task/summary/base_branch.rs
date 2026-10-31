//! Properties › Base under Branch (`Base` in 03 `qg3JF`, every state in `PgNtj`): the branch the latest run started
//! from and where it came from, recorded when it started; before a run, what the next one takes. A line of a 12
//! `text_muted` glyph (`git-commit-horizontal` for the checked-out branch, else `git-branch`) and the branch in Geist
//! Mono 12 `text_secondary`, 6 apart, and under it, 2 apart, why in 11.5 `text_muted`: this task's, from its module,
//! checked out when the run started, the next run starts from it, or that the run keeps it while the task or its
//! module now says another. A run the runner refused for want of a branch says *No branch* in `warning`, and why.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ModuleId,
  integration::base::{BaseChoice, RunBaseSource, resolve},
};
use orchester_workspace::{display, tasks::data::ProjectTasks};

use super::properties::{Look, Setup, cut, run_setup, value};
use crate::{
  screens::task::{PageState, TaskContext, header::no_branch::NoBranch},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

const ID: &str = "task-property-base";

/// The row's value, as the state of the task's runs says.
pub fn base(ctx: &mut Ctx, task: &TaskContext, tasks: Option<&ProjectTasks>) -> Element {
  if let Some(refused) = NoBranch::shown(PageState::of(ctx).no_branch.get(), task) {
    let word = Text::new(&ctx.t("summary.properties.base.noBranch"))
      .id(format!("{ID}-branch"))
      .variant(TypographyStyle::Description)
      .color(PaletteColor::Warning)
      .nowrap();
    let why = ctx.t_args("summary.properties.base.noBranchWhy", [("head", &*refused.head)]);
    return shown((Icon::TriangleAlert, PaletteColor::Warning), word.into(), &why);
  }
  let next = next_base(task, tasks);
  match run_setup(task) {
    Setup::NotRecorded => value(ID, &ctx.t("summary.properties.notRecorded"), Look::Muted).into(),
    Setup::Run(run) => {
      let run = &run.run;
      let number = run.number().get().to_string();
      let source = run.base_source();
      let (glyph, branch) = match run.base_branch() {
        Some(branch) => (glyph_of(source), branch.to_string()),
        None => (
          Icon::GitCommitHorizontal,
          display::short_revision(run.base().as_str()).to_owned(),
        ),
      };
      let recorded = run.base_branch().filter(|_| source.set());
      let why = match next.filter(|(choice, _)| choice.branch() != recorded) {
        Some((choice, next_source)) => kept(ctx, &number, (&choice, next_source)),
        None => recorded_source(ctx, (source, run.base_branch().is_some()), &number, tasks),
      };
      let branch = cut(ctx, &format!("{ID}-branch"), &branch);
      shown((glyph, PaletteColor::TextMuted), branch, &why)
    }
    Setup::None => match next {
      Some((choice, source)) => upcoming(ctx, (&choice, source)),
      None => value(ID, &ctx.t("summary.properties.notKnown"), Look::Muted).into(),
    },
  }
}

/// The glyph and the branch on a line, and why under it.
fn shown((glyph, color): (Icon, PaletteColor), branch: Element, why: &str) -> Element {
  Column::new()
    .id(ID)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Sm, color))
        .child(branch),
    )
    .child(
      Text::new(why)
        .id(format!("{ID}-source"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

const fn glyph_of(source: RunBaseSource) -> Icon {
  match source {
    RunBaseSource::CheckedOut => Icon::GitCommitHorizontal,
    RunBaseSource::Task | RunBaseSource::Module { .. } | RunBaseSource::Project => Icon::GitBranch,
  }
}

/// What a new run of the task takes now, with its module's base branch as the project's tasks were read; `None` while
/// the module's is not known yet.
fn next_base(task: &TaskContext, tasks: Option<&ProjectTasks>) -> Option<(BaseChoice, RunBaseSource)> {
  let stored = &task.detail.task;
  let module = match stored.module() {
    Some(id) => Some(tasks?.modules.iter().find(|module| module.id() == id)?),
    None => None,
  };
  Some(resolve(
    stored.base(),
    module.map(|module| (module.id(), module.base())),
  ))
}

fn module_name(tasks: Option<&ProjectTasks>, id: ModuleId) -> Option<String> {
  tasks?
    .modules
    .iter()
    .find(|module| module.id() == id)
    .map(|module| module.name().as_str().to_owned())
}

/// Where the run's base branch came from, as it recorded when it started run `number`; `named` whether it recorded a
/// branch (a run recorded before Orchester kept it took a detached HEAD's).
fn recorded_source(
  ctx: &Ctx,
  (source, named): (RunBaseSource, bool),
  number: &str,
  tasks: Option<&ProjectTasks>,
) -> Arc<str> {
  match source {
    RunBaseSource::Task => ctx.t("summary.properties.base.task"),
    RunBaseSource::Module { module } => match module_name(tasks, module) {
      Some(name) => ctx.t_args("summary.properties.base.module", [("module", name)]),
      None => ctx.t("summary.properties.base.someModule"),
    },
    RunBaseSource::CheckedOut if named => ctx.t_args("summary.properties.base.checkedOut", [("run", number)]),
    RunBaseSource::CheckedOut => ctx.t_args("summary.properties.base.detached", [("run", number)]),
    RunBaseSource::Project => ctx.t_args("summary.properties.base.project", [("run", number)]),
  }
}

/// That run `number` keeps the branch it started from, while the next run takes `next` from where it comes from now.
fn kept(ctx: &Ctx, number: &str, (next, source): (&BaseChoice, RunBaseSource)) -> Arc<str> {
  let Some(branch) = next.branch() else {
    return ctx.t_args("summary.properties.base.keptCheckedOut", [("run", number)]);
  };
  let key = match source {
    RunBaseSource::Module { .. } => "summary.properties.base.keptModule",
    RunBaseSource::Task | RunBaseSource::CheckedOut | RunBaseSource::Project => "summary.properties.base.keptTask",
  };
  ctx.t_args(key, [("run", number), ("branch", branch.as_str())])
}

/// Before a run: the branch the next one takes and where it comes from, or the checked-out branch.
fn upcoming(ctx: &Ctx, (next, source): (&BaseChoice, RunBaseSource)) -> Element {
  let Some(branch) = next.branch() else {
    let words = value(
      &format!("{ID}-branch"),
      &ctx.t("summary.properties.base.nextCheckedOut"),
      Look::Secondary,
    );
    let why = ctx.t("summary.properties.base.nextTakesCheckedOut");
    return shown((Icon::GitCommitHorizontal, PaletteColor::TextMuted), words.into(), &why);
  };
  let key = match source {
    RunBaseSource::Module { .. } => "summary.properties.base.nextModule",
    RunBaseSource::Task | RunBaseSource::CheckedOut | RunBaseSource::Project => "summary.properties.base.nextTask",
  };
  let words = value(
    &format!("{ID}-branch"),
    &ctx.t_args(key, [("branch", branch.as_str())]),
    Look::Secondary,
  );
  let why = ctx.t("summary.properties.base.nextStarts");
  shown((Icon::GitBranch, PaletteColor::TextMuted), words.into(), &why)
}
