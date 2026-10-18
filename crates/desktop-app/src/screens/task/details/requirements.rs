//! The Details' requirements (`Details · complete requirements` `b1cCbt` in `UxsRH`): the version picker
//! (`Requirements version selector` `UkPgL`, a `Field/select` 280 wide) of the versions the task kept, where
//! the version shown comes from (`Snapshot provenance` `FKZ6U`), that the creation version was not recorded
//! when an older task shows another in its place (`Creation snapshot unavailable notice` `NeEwF` in
//! `R3Wkvd`), then the task's title, its description as Markdown and every acceptance criterion, numbered,
//! with their count. Everything is shown whole: the column scrolls.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::task::{
  TaskDetails,
  requirements::{CurrentStanding, Fallback, LatestRun, RequirementsSource, TaskRequirements},
};

use super::Version;
use crate::{
  screens::task::TaskContext,
  theme::{ExtraTypography, TaskSpacing, TaskText},
  ui::{
    chat::links,
    field::{SelectField, field},
    markdown::MarkdownView,
  },
};

/// The picker's width (`UkPgL`).
const PICKER_WIDTH: f32 = 280.0;

/// The column of the version `shown`, which `chosen` holds once picked; `narrow` in a narrow window.
pub fn column(
  ctx: &mut Ctx,
  task: &TaskContext,
  (shown, chosen): (RequirementsSource, &Signal<Option<Version>>),
  narrow: bool,
) -> Element {
  let requirements = &task.detail.requirements;
  let details = requirements
    .details(shown)
    .expect("the version shown is one the task kept");
  let side = if narrow { SpacingSize::Xl } else { SpacingSize::Section };
  let fallback = requirements
    .fallback(shown)
    .map(|fallback| fallback_notice(ctx, fallback));
  Column::new()
    .id("task-details-requirements")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(TaskSpacing::Panel.role())
        .left(side)
        .right(side)
        .bottom(TaskSpacing::Foot.role()),
    )
    .spacing(TaskSpacing::Block.role())
    .child(picker(ctx, requirements, chosen))
    .child(
      Text::new(&provenance(ctx, requirements, shown))
        .id("task-details-provenance")
        .variant(TaskText::DetailsNote.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(fallback)
    .child(title(ctx, details))
    .child(description(ctx, details))
    .child(criteria(ctx, details))
    .into()
}

/// The versions the task kept, the one shown chosen.
fn picker(ctx: &Ctx, requirements: &TaskRequirements, chosen: &Signal<Option<Version>>) -> Element {
  let options = requirements
    .sources()
    .into_iter()
    .map(|source| SelectOption::new(Some(Version(source)), source_label(ctx, requirements, source)))
    .collect();
  let label = ctx.t("details.source.label");
  let select = SelectField::new("task-details-source", label.clone(), chosen.clone(), options).render(ctx);
  Column::new()
    .width(Dimension::Px(PICKER_WIDTH))
    .max_width(Dimension::Pct(100.0))
    .child(field("task-details-source", Some(&label), select, None))
    .into()
}

fn source_label(ctx: &Ctx, requirements: &TaskRequirements, source: RequirementsSource) -> Arc<str> {
  match source {
    RequirementsSource::Creation => ctx.t("details.source.creation"),
    RequirementsSource::RunStart(_) => ctx.t_args("details.source.runStart", [("run", run_number(requirements))]),
    RequirementsSource::Current => ctx.t("details.source.current"),
  }
}

/// The number of the task's latest run, which the run-start version is of.
fn run_number(requirements: &TaskRequirements) -> String {
  match requirements.latest_run() {
    LatestRun::Run(run) => run.number.get().to_string(),
    LatestRun::None | LatestRun::Unknown => String::new(),
  }
}

/// Where the version `shown` comes from: read-only as created or as a run was given it, or the current task
/// and how it stands against its latest run.
fn provenance(ctx: &Ctx, requirements: &TaskRequirements, shown: RequirementsSource) -> Arc<str> {
  let run = |key: &str, number: u32| ctx.t_args(key, [("run", number.to_string())]);
  match shown {
    RequirementsSource::Creation => ctx.t("details.provenance.creation"),
    RequirementsSource::RunStart(_) => ctx.t_args("details.provenance.runStart", [("run", run_number(requirements))]),
    RequirementsSource::Current => match requirements.current_standing() {
      CurrentStanding::NoRun => ctx.t("details.provenance.currentNoRun"),
      CurrentStanding::EditedAfter(number) => run("details.provenance.currentEdited", number.get()),
      CurrentStanding::UnchangedSince(number) => run("details.provenance.currentUnchanged", number.get()),
      CurrentStanding::RunStartNotRecorded(number) => run("details.provenance.currentNotRecorded", number.get()),
      CurrentStanding::RunUnknown => ctx.t("details.provenance.currentRunUnknown"),
    },
  }
}

/// *Creation version not recorded*, and what is shown in its place.
fn fallback_notice(ctx: &Ctx, fallback: Fallback) -> Element {
  let explanation = match fallback {
    Fallback::RunStart => ctx.t("details.fallback.runStart"),
    Fallback::Current => ctx.t("details.fallback.current"),
  };
  Column::new()
    .id("task-details-fallback")
    .describe("role", "creation-not-recorded")
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(SpacingSize::Md))
    .spacing(SpacingSize::Xs)
    .background(PaletteColor::WarningMuted)
    .rounded(RadiusSize::Md)
    .child(
      Text::new(&ctx.t("details.fallback.title"))
        .id("task-details-fallback-title")
        .variant(TaskText::DetailsNoticeHeading.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Text::new(&explanation)
        .id("task-details-fallback-text")
        .variant(TaskText::DetailsBody.role())
        .color(PaletteColor::TextSecondary)
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// A section's heading over its content, `gap` apart.
fn section(id: &str, heading: &str, gap: SpacingSize, content: impl Into<Element>) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(gap)
    .child(
      Text::new(heading)
        .id(format!("{id}-head"))
        .variant(TaskText::DetailsHeading.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(content)
    .into()
}

fn title(ctx: &Ctx, details: &TaskDetails) -> Element {
  let title = Text::new(details.title.as_str())
    .id("task-details-title-text")
    .variant(TaskText::DetailsTitle.role())
    .width(Dimension::Pct(100.0))
    .selectable(true);
  section("task-details-title", &ctx.t("details.title"), SpacingSize::Sm, title)
}

/// The description whole, as Markdown; that there is none yet.
fn description(ctx: &mut Ctx, details: &TaskDetails) -> Element {
  let text: Element = if details.brief.trim().is_empty() {
    muted("task-details-description-text", &ctx.t("details.noDescription"))
  } else {
    let on_link = links(ctx);
    MarkdownView::new("task-details-description-text", details.brief.as_str(), on_link)
      .style(TaskText::DetailsBody.role(), PaletteColor::TextPrimary)
      .render(ctx)
  };
  let heading = ctx.t("details.description");
  section("task-details-description", &heading, SpacingSize::Md, text)
}

/// Every criterion, numbered, each over a hairline (`All acceptance criteria · unscored` `W7XXTD`).
fn criteria(ctx: &Ctx, details: &TaskDetails) -> Element {
  let criteria = &details.acceptance_criteria;
  let heading = ctx.t_args("details.criteria", [("count", criteria.len().to_string())]);
  let list: Element = if criteria.is_empty() {
    muted("task-details-criteria-none", &ctx.t("details.noCriteria"))
  } else {
    Column::new()
      .id("task-details-criteria-list")
      .width(Dimension::Pct(100.0))
      .with_children((1..).zip(criteria).map(|(number, text)| criterion(number, text)))
      .into()
  };
  section("task-details-criteria", &heading, SpacingSize::Sm, list)
}

/// "01" and the criterion's words (`Acceptance criterion 1` `ZT2hO`).
fn criterion(number: usize, text: &str) -> Element {
  Row::new()
    .id(format!("task-details-criterion-{number}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&format!("{number:02}"))
        .id(format!("task-details-criterion-{number}-number"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap(),
    )
    .child(
      Text::new(text)
        .id(format!("task-details-criterion-{number}-text"))
        .variant(TaskText::DetailsBody.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .selectable(true),
    )
    .into()
}

fn muted(id: &str, text: &str) -> Element {
  Text::new(text)
    .id(id.to_owned())
    .variant(TaskText::DetailsBody.role())
    .color(PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
    .into()
}
