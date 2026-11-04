//! Runs (`02 · Runs` `b7Ya1u`, the Runs window `g406O`, `S02` `Ekiul`): Approvals (whether Autopilot is allowed, its
//! reviewer, and *Approved for every run* with a count that opens the project's rules, [`super::rules`]), Estimates
//! (*Let Orchester estimate story points*, a person's switch) and Limits (the run and daily budgets, the time limit and
//! how many agents and QA apps work at once), each saving as a person leaves a field. Approvals' foot says where a
//! profile chooses its approval mode, Limits' that a task and a pipeline step set their own. The variables moved to
//! Environment.
mod auto_mode;
mod estimates;
mod limits;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{
  SectionStatuses, TabProps,
  kit::{Group, SettingRow},
  page,
  work::{self, Read},
};
use crate::{
  queries::Kept,
  theme::ExtraSpacing,
  ui::skeleton::{self, fading},
};

/// The reviewer's connection, model and effort selects (`Reviewer` in `lUFjj`).
const REVIEWER: (f32, f32, f32) = (142.0, 116.0, 132.0);

pub(super) struct Runs {
  /// What the page's last save read back, until the settings' query moves on.
  kept: Kept<Read>,
  /// Auto mode's and the limits' save statuses.
  sections: SectionStatuses,
}

impl Component for Runs {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let page = ctx.props::<TabProps>().status.clone();
    Self {
      kept: ctx.props::<TabProps>().kept.clone(),
      sections: SectionStatuses::new(ctx, 2, &page),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let loaded = work::load(ctx, props.project.open.repo_root.clone());
    // Auto mode keeps what its save read once that save ended: nothing is kept while it saves.
    let Some(read) = loaded.shown(&self.kept, false) else {
      return page::scroll(work::pending(ctx, &loaded, bones));
    };
    // What Estimates names its estimator after: what the page shows, nothing while the settings cannot be read.
    let shown = loaded.state.error.is_none().then(|| read.0.clone());
    // A read that failed shows why with Try again in Auto mode's place, not the reviewer it last read.
    let auto = if loaded.state.error.is_some() {
      work::pending(ctx, &loaded, approval_bones)
    } else {
      ctx.mount_keyed::<auto_mode::AutoMode>(
        "settings-auto-mode",
        auto_mode::Props {
          tab: self.sections.props(&props, 0),
          settings: read.0,
          kept: self.kept.clone(),
        },
      )
    };
    let estimates = ctx.mount_keyed::<estimates::Estimates>(
      "project-settings-estimates",
      estimates::Props {
        tab: props.clone(),
        settings: shown,
        read_error: loaded.state.error.clone(),
      },
    );
    let limits = ctx.mount_keyed::<limits::Limits>("project-settings-limits", self.sections.props(&props, 1));
    page::scroll(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Roomy.role())
        .child(auto)
        .child(estimates)
        .child(limits),
    )
  }
}

/// Approvals' loading controls, also used while recovering from a rejected settings read.
fn approval_bones(ctx: &mut Ctx) -> Element {
  let reviewer = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(skeleton::select(ctx, REVIEWER.0, 76.0))
    .child(skeleton::select(ctx, REVIEWER.1, 60.0))
    .child(skeleton::select(ctx, REVIEWER.2, 70.0));
  let rows = [
    // Its hint says whether Autopilot is allowed: a bone until the setting is read.
    SettingRow::new(
      "settings-auto-allow-autopilot-reading",
      ctx.t("project_settings.allowAutopilot"),
    )
    .info(ctx.t("project_settings.allowAutopilotDetail"))
    .hint_reading(300.0)
    .control(skeleton::switch()),
    SettingRow::new("settings-auto-reviewer-reading", ctx.t("project_settings.reviewer"))
      .info(ctx.t("project_settings.reviewerDetail"))
      .hint(ctx.t("project_settings.reviewerHint"))
      .control(reviewer),
    SettingRow::new("settings-auto-rules-reading", ctx.t("project_settings.projectRules"))
      .info(ctx.t("project_settings.projectRulesDetail"))
      .hint_reading(266.0)
      .control(skeleton::count())
      .control(skeleton::chevron()),
  ]
  .map(|row| row.render(ctx));
  Group::new("settings-auto-reading", ctx.t("project_settings.auto"))
    .rows(rows)
    .foot(&ctx.t("project_settings.approvalsFoot"))
    .into()
}

/// The Runs tab while its settings are read (`10 · Project settings · Runs · loading` `lUFjj`): every group heading,
/// row label, help line and unit stays; the switches, the reviewer's connection, model and effort, the approved-rules
/// count and the budgets and limits are bones in their own fields, the lower groups fading.
fn bones(ctx: &mut Ctx) -> Column {
  let auto = approval_bones(ctx);
  let row = SettingRow::new(
    "settings-estimates-reading-row",
    ctx.t("project_settings.estimates.label"),
  )
  .hint(ctx.t("project_settings.estimates.hint"))
  .info(ctx.t("project_settings.estimates.info"))
  .control(skeleton::switch())
  .render(ctx);
  let estimates = Group::new("settings-estimates-reading", ctx.t("project_settings.estimates.title")).row(row);
  let groups: Vec<Column> = vec![
    Column::new().width(Dimension::Pct(100.0)).child(auto),
    Column::new().width(Dimension::Pct(100.0)).child(estimates),
    Column::new().width(Dimension::Pct(100.0)).child(limits::bones(ctx)),
  ];
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Roomy.role())
    .with_children(fading(groups))
}
