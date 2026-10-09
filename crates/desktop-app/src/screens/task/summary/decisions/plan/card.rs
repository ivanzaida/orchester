//! The plan's card (`Plan review` `hLziL`): its title with who wrote it and when, the choice of the
//! step's versions (`Picker · v2 · latest` `rMaOi`), the plan as Markdown, what changed since the version
//! before from the person's note (`Changed` `t7skY`), and the task's acceptance criteria numbered
//! (`Criterion` `Ysu5i`). An earlier version says which one waits instead.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  run::{Answerer, Plan, PlanDecision},
  task::Task,
};
use orchester_workspace::{display, tasks::plan::PlanReview};

use super::{super::divider, Shown};
use crate::{
  theme::{DecisionText, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    card::{CardSurface, card},
    field::SelectField,
    icon::{Icon, icon},
    markdown::{LinkHandler, MarkdownView},
  },
};

/// The version choice's width (`rMaOi`, room for "v12 · changes requested").
const VERSION_WIDTH: f32 = 200.0;
/// The width of a criterion's number (`N2M6U`).
const NUMBER_WIDTH: f32 = 14.0;

pub struct PlanCard<'a> {
  pub review: &'a PlanReview,
  pub shown: &'a Plan,
  /// The task as its run was asked: its key and acceptance criteria.
  pub task: &'a Task,
  pub on_link: &'a LinkHandler,
}

impl PlanCard<'_> {
  pub fn render(&self, ctx: &mut Ctx, shown: &Signal<Shown>) -> Element {
    let text = self.plan_text(ctx);
    card("plan-review", CardSurface::Decision)
      .spacing(ExtraSpacing::Loose.role())
      .child(self.head(ctx, shown))
      .child(divider())
      .child(text)
      .with_children(self.change_line(ctx))
      .child(divider())
      .child(self.criteria(ctx))
      .into()
  }

  /// "Plan v2 · Add retry to webhook sender", "by Planner · 16:14", and the version choice.
  fn head(&self, ctx: &Ctx, shown: &Signal<Shown>) -> Element {
    let version = self.shown.version.to_string();
    let title = ctx.t_args(
      "decisions.plan.title",
      [
        ("version", version.as_str()),
        ("task", self.task.details().title.as_str()),
      ],
    );
    let by = ctx.t_args(
      "decisions.plan.by",
      [
        ("agent", &*self.review.agent),
        ("time", display::clock(self.shown.created_at).as_str()),
      ],
    );
    let label = ctx.t("decisions.plan.version");
    Row::new()
      .id("plan-review-head")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Map, IconSize::Nav, PaletteColor::TextSecondary))
      .child(
        Text::new(&title)
          .id("plan-review-title")
          .variant(DecisionText::CardTitle.role())
          .nowrap(),
      )
      .child(Text::new(&by).variant(ExtraTypography::Note.role()).nowrap())
      .child(Spacer::new().flex(1.0))
      .child(Text::new(&label).variant(ExtraTypography::Note.role()).nowrap())
      .child(
        SelectField::new("plan-review-version", label.clone(), shown.clone(), self.versions(ctx))
          .width(Dimension::Px(VERSION_WIDTH))
          .render(ctx),
      )
      .into()
  }

  /// The step's versions, newest first, each with where it stands: "v2 · latest", "v1 · changes requested".
  fn versions(&self, ctx: &Ctx) -> Vec<SelectOption<Shown>> {
    let waiting = self.review.waiting().version;
    self
      .review
      .plans()
      .iter()
      .rev()
      .map(|plan| {
        let version = plan.version.to_string();
        let (value, key) = if plan.version == waiting {
          (Shown(None), "decisions.plan.latest")
        } else {
          (Shown(Some(plan.version)), earlier_key(plan))
        };
        SelectOption::new(value, ctx.t_args(key, [("version", version.as_str())]))
      })
      .collect()
  }

  fn plan_text(&self, ctx: &mut Ctx) -> Element {
    if self.shown.text.trim().is_empty() {
      return Text::new(&ctx.t("decisions.plan.empty"))
        .id("plan-review-text")
        .variant(ExtraTypography::Note.role())
        .into();
    }
    Column::new()
      .id("plan-review-text")
      .width(Dimension::Pct(100.0))
      .child(
        MarkdownView::new("plan-review-markdown", self.shown.text.as_str(), self.on_link.clone())
          .style(DecisionText::Plan.role(), PaletteColor::TextSecondary)
          .render(ctx),
      )
      .into()
  }

  /// What the shown version answers: the note the person (or automation) gave the version before it; an
  /// earlier version says which one waits.
  fn change_line(&self, ctx: &Ctx) -> Option<Element> {
    let waiting = self.review.waiting().version;
    if self.shown.version != waiting {
      let (version, latest) = (self.shown.version.to_string(), waiting.to_string());
      let text = ctx.t_args(
        "decisions.plan.earlier",
        [("version", version.as_str()), ("latest", latest.as_str())],
      );
      return Some(changed(&text));
    }
    let before = self
      .review
      .plans()
      .iter()
      .rev()
      .find(|plan| plan.version < self.shown.version)?;
    let review = before.review.as_ref()?;
    let PlanDecision::ChangesRequested { note } = &review.decision else {
      return None;
    };
    let key = match review.by {
      Answerer::Person => "decisions.plan.changedFromYourNote",
      Answerer::Automation => "decisions.plan.changedFromAutomation",
    };
    let version = before.version.to_string();
    Some(changed(
      &ctx.t_args(key, [("version", version.as_str()), ("note", note.as_str())]),
    ))
  }

  /// "Acceptance criteria of KON-12" and the criteria, numbered.
  fn criteria(&self, ctx: &Ctx) -> Element {
    let key = self.task.key().to_string();
    let criteria = &self.task.details().acceptance_criteria;
    let head = Text::new(&ctx.t_args("decisions.plan.criteria", [("task", key.as_str())]))
      .id("plan-review-criteria-head")
      .variant(DecisionText::Subhead.role());
    let rows: Vec<Element> = if criteria.is_empty() {
      vec![
        Text::new(&ctx.t("decisions.plan.noCriteria"))
          .variant(ExtraTypography::Note.role())
          .into(),
      ]
    } else {
      (1..)
        .zip(criteria)
        .map(|(number, text)| criterion(number, text))
        .collect()
    };
    Column::new()
      .id("plan-review-criteria")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(head)
      .with_children(rows)
      .into()
  }
}

/// Where an earlier version stands, as the choice reads it.
fn earlier_key(plan: &Plan) -> &'static str {
  match plan.review.as_ref().map(|review| &review.decision) {
    Some(PlanDecision::ChangesRequested { .. }) => "decisions.plan.changesRequested",
    Some(PlanDecision::Approved) => "decisions.plan.approved",
    None => "decisions.plan.notDecided",
  }
}

fn criterion(number: usize, text: &str) -> Element {
  Row::new()
    .id(format!("plan-review-criterion-{number}"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(
      Text::new(&number.to_string())
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(NUMBER_WIDTH)),
    )
    .child(
      Text::new(text)
        .variant(DecisionText::Criterion.role())
        .flex(1.0)
        .selectable(true),
    )
    .into()
}

/// A line on `surface_raised` with the compare glyph (`Changed` `t7skY`).
fn changed(text: &str) -> Element {
  Row::new()
    .id("plan-review-changed")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Tile.role())
    .child(icon(Icon::GitCompare, IconSize::Base, PaletteColor::TextMuted))
    .child(Text::new(text).variant(TypographyStyle::Description).flex(1.0))
    .into()
}
