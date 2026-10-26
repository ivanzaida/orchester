//! The version of the requirements the brief and criteria show (`Version` `Oeeet` at the end of `Heading · Brief`
//! `AKzAZ`): a chip with the version's name and a chevron, 1 `border` stroke, radius 6, 2 over and under and 8 at its
//! sides; its menu offers the versions the task kept (as created, as the latest run started, the current task), and its
//! tooltip says where the version shown comes from. A creation version an older task did not record says so, with
//! what is shown in its place ([`fallback`]). The version shown is kept by the frame across tabs and readings of the
//! task; one a new reading no longer offers (the task's latest run changed) gives way to the default
//! (`TaskRequirements::shown`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::task::requirements::{CurrentStanding, Fallback, LatestRun, RequirementsSource, TaskRequirements};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    menu::{MenuItem, menu_at_end, menu_panel},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// The menu's width: room for *As run #12 started*.
const MENU_WIDTH: f32 = 200.0;

/// A version of the requirements as the page and its menu hold it: lurq's DevTools marker, which signal values need,
/// cannot be given to core's type here (orphan rule).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Version(pub RequirementsSource);

impl lurq::app::component::DevtoolsInspectable for Version {}

/// Where the version menu is open.
pub struct VersionMenu {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl VersionMenu {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }

  /// The chip of the version `shown` of `requirements`, which `chosen` holds once picked, with its menu while it is
  /// open.
  pub fn chip(
    &self,
    ctx: &mut Ctx,
    requirements: &TaskRequirements,
    (shown, chosen): (RequirementsSource, &Signal<Option<Version>>),
  ) -> Element {
    let label = source_label(ctx, requirements, shown);
    let open = self.open.clone();
    let chip = pressable(move || open.update(|open| *open = !*open))
      .id("task-brief-version")
      .ref_element(self.anchor.clone())
      .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Tight.role()))
      .spacing(SpacingSize::Xs)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .describe("a11y_name", label.clone())
      .child(
        Text::new(&label)
          .id("task-brief-version-label")
          .variant(ExtraTypography::Note.role())
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      )
      .child(icon(Icon::ChevronDown, IconSize::Sm, PaletteColor::TextMuted));
    let tip = provenance(ctx, requirements, shown);
    let chip = Tooltip::new("task-brief-version-tip", tip).wrap(ctx, chip);
    let menu = self.open.get().then(|| {
      let panel = self.panel(ctx, requirements, (shown, chosen));
      menu_at_end(self.anchor.clone(), self.open.clone(), panel)
    });
    Row::new()
      .flex_shrink(0.0)
      .align_items(Alignment::Center)
      .child(chip)
      .with_children(menu)
      .into()
  }

  /// The versions the task kept, the one shown chosen.
  fn panel(
    &self,
    ctx: &mut Ctx,
    requirements: &TaskRequirements,
    (shown, chosen): (RequirementsSource, &Signal<Option<Version>>),
  ) -> Element {
    let rows: Vec<Element> = requirements
      .sources()
      .into_iter()
      .enumerate()
      .map(|(index, source)| {
        let chosen = chosen.clone();
        MenuItem::new(
          format!("task-brief-version-{index}"),
          source_label(ctx, requirements, source),
          move || chosen.set(Some(Version(source))),
        )
        .chosen(source == shown)
        .render(ctx, &self.open)
      })
      .collect();
    menu_panel("task-brief-version-menu")
      .width(Dimension::Px(MENU_WIDTH))
      .with_children(rows)
      .into()
  }
}

/// A version's name: as created, as the latest run started, or the current task.
fn source_label(ctx: &Ctx, requirements: &TaskRequirements, source: RequirementsSource) -> Arc<str> {
  match source {
    RequirementsSource::Creation => ctx.t("summary.version.creation"),
    RequirementsSource::RunStart(_) => ctx.t_args("summary.version.runStart", [("run", run_number(requirements))]),
    RequirementsSource::Current => ctx.t("summary.version.current"),
  }
}

/// The number of the task's latest run, which the run-start version is of.
fn run_number(requirements: &TaskRequirements) -> String {
  match requirements.latest_run() {
    LatestRun::Run(run) => run.number.get().to_string(),
    LatestRun::Elsewhere(number) => number.get().to_string(),
    LatestRun::None | LatestRun::Unknown => String::new(),
  }
}

/// Where the version `shown` comes from: read-only as created or as a run was given it, or the current task and how
/// it stands against its latest run.
fn provenance(ctx: &Ctx, requirements: &TaskRequirements, shown: RequirementsSource) -> Arc<str> {
  let run = |key: &str, number: u32| ctx.t_args(key, [("run", number.to_string())]);
  match shown {
    RequirementsSource::Creation => ctx.t("summary.version.provenance.creation"),
    RequirementsSource::RunStart(_) => ctx.t_args(
      "summary.version.provenance.runStart",
      [("run", run_number(requirements))],
    ),
    RequirementsSource::Current => match requirements.current_standing() {
      CurrentStanding::NoRun => ctx.t("summary.version.provenance.currentNoRun"),
      CurrentStanding::EditedAfter(number) => run("summary.version.provenance.currentEdited", number.get()),
      CurrentStanding::UnchangedSince(number) => run("summary.version.provenance.currentUnchanged", number.get()),
      CurrentStanding::RunStartNotRecorded(number) => {
        run("summary.version.provenance.currentNotRecorded", number.get())
      }
      CurrentStanding::RunUnknown => ctx.t("summary.version.provenance.currentRunUnknown"),
      CurrentStanding::RunElsewhere(number) => run("summary.version.provenance.currentElsewhere", number.get()),
    },
  }
}

/// That the creation version of an older task was not recorded, and what is shown in its place: a line in `warning`
/// under the brief's heading.
pub fn fallback(ctx: &Ctx, requirements: &TaskRequirements, shown: RequirementsSource) -> Option<Element> {
  let explanation = match requirements.fallback(shown)? {
    Fallback::RunStart => ctx.t("summary.version.fallback.runStart"),
    Fallback::Current => ctx.t("summary.version.fallback.current"),
  };
  Some(
    Row::new()
      .id("task-brief-fallback")
      .describe("role", "creation-not-recorded")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Start)
      .child(icon(Icon::TriangleAlert, IconSize::Sm, PaletteColor::Warning))
      .child(
        Text::new(&ctx.t_args(
          "summary.version.fallback.line",
          [
            ("title", ctx.t("summary.version.fallback.title")),
            ("explanation", explanation),
          ],
        ))
        .id("task-brief-fallback-text")
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::Warning)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
      )
      .into(),
  )
}
