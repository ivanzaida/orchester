//! Edit task when the save met a newer version (10.41 `RmDpQ`, decisions 17 and 18): a notice says
//! who saved it and that nothing of the person's was saved; each content field says whether it is
//! unchanged or who changed it; for each field both changed, the person's version and the saved one
//! side by side, the person's chosen at first. *Save with your version* saves the chosen versions on
//! top of the saved one, keeping its other changes; *Use …'s version* takes the saved one for every
//! field both changed. The person's text stays until they choose; the organization merged by itself.

use std::{collections::BTreeMap, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  task::{
    TaskDetails, TaskLimits,
    content::{Choice, ContentField, FieldChange},
  },
  tokens::compact_tokens,
};

use super::{NewTaskForm, Reseed, SaveAction, SaveArgs};
use crate::{
  budget::minutes_text,
  display,
  tasks::{
    cancel_run::Stop,
    conflict::EditConflict,
    data::{PipelineChoice, SaveTarget},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, gutter_scrollbar, gutter_width},
  ui::{
    button::ActionButton,
    field::{ValueTone, value_box},
    icon::{Icon, IconSize, icon},
    notice::Notice,
    option_card::OptionCard,
    property::property,
  },
  workspace::OpenProject,
};

/// The height of the panel's buttons (`Ny3Wz`).
const BUTTON_HEIGHT: f32 = 36.0;

/// The version of each field both changed the person chose; their own until they choose.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Choices(BTreeMap<ContentField, Choice>);

impl lurq::app::component::DevtoolsInspectable for Choices {}

impl Choices {
  fn of(&self, field: ContentField) -> Choice {
    self.0.get(&field).copied().unwrap_or_default()
  }
}

/// What a field of the conflict view shows.
struct Shown<'a> {
  conflict: &'a EditConflict,
  choices: &'a Choices,
  pipelines: Option<&'a [PipelineChoice]>,
}

impl NewTaskForm {
  /// The notice and the content fields, in place of the brief (`hkWbp` in 10.41).
  pub(super) fn conflict_view(
    &self,
    ctx: &Ctx,
    conflict: &EditConflict,
    pipelines: Option<&[PipelineChoice]>,
  ) -> Element {
    let choices = self.choices.get();
    let shown = Shown {
      conflict,
      choices: &choices,
      pipelines,
    };
    // Pipeline and limits read in the panel unless both changed them.
    let fields = ContentField::ALL.into_iter().filter(|field| {
      matches!(
        field,
        ContentField::Title | ContentField::Brief | ContentField::Criteria
      ) || conflict.merge.conflicts().contains(field)
    });
    let content = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(notice(ctx, conflict))
      .with_children(fields.map(|field| self.field_block(ctx, &shown, field)));
    // Its scroll gutter sits in the gap to the panel, as the brief's does (see `content_row`).
    ScrollVertical::new(content)
      .id("task-conflict")
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .scrollbar(gutter_scrollbar())
      .into()
  }

  /// A field's label and who changed it (`mtsFH`, `As0yQ`), over its value or both versions.
  fn field_block(&self, ctx: &Ctx, shown: &Shown<'_>, field: ContentField) -> Element {
    let conflict = shown.conflict;
    let meta = match conflict.merge.change(field) {
      FieldChange::Unchanged => ctx.t("tasks.conflict.unchanged"),
      FieldChange::Mine => ctx.t("tasks.conflict.changedByYou"),
      FieldChange::Theirs => by(
        ctx,
        conflict,
        "tasks.conflict.changedByThem",
        "tasks.conflict.changedElsewhere",
      ),
      FieldChange::Both => by(
        ctx,
        conflict,
        "tasks.conflict.changedByBoth",
        "tasks.conflict.changedByBothElsewhere",
      ),
    };
    let value = if conflict.merge.conflicts().contains(&field) {
      self.versions(ctx, shown, field)
    } else {
      let resolved = conflict.resolved(|field| shown.choices.of(field));
      field_value(ctx, shown, &resolved, field)
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(Text::new(&ctx.t(label_key(field))).variant(ExtraTypography::FormLabel.role()))
          .child(
            Text::new(&meta)
              .variant(TypographyStyle::Caption)
              .color(PaletteColor::TextMuted),
          ),
      )
      .child(value)
      .into()
  }

  /// The person's version and the saved one of `field`, side by side; the chosen one marked.
  fn versions(&self, ctx: &Ctx, shown: &Shown<'_>, field: ContentField) -> Element {
    let conflict = shown.conflict;
    let chosen = shown.choices.of(field);
    let card = |choice: Choice| {
      let (details, title, meta) = match choice {
        Choice::Mine => (
          conflict.merge.mine(),
          ctx.t("tasks.conflict.yours"),
          ctx.t("tasks.conflict.yoursMeta"),
        ),
        Choice::Theirs => (
          conflict.merge.theirs(),
          by(ctx, conflict, "tasks.conflict.theirs", "tasks.conflict.theirsElsewhere"),
          conflict.at.map_or_else(
            || ctx.t("tasks.conflict.saved"),
            |at| ctx.t_args("tasks.conflict.savedAt", [("time", display::clock(at).as_str())]),
          ),
        ),
      };
      let choices = self.choices.clone();
      let text = Arc::from(value_text(ctx, details, field, shown.pipelines));
      Column::new().flex(1.0).child(
        OptionCard::new(card_id(field, choice), title, text, chosen == choice)
          .meta(meta)
          .on_choose(move || {
            choices.update(|choices| {
              choices.0.insert(field, choice);
            });
          }),
      )
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Stretch)
      .child(card(Choice::Mine))
      .child(card(Choice::Theirs))
      .into()
  }

  /// The plan as it would be saved, *Save with your version*, *Use …'s version* and what saving
  /// does, in the plan's column.
  pub(super) fn conflict_panel(
    &self,
    ctx: &Ctx,
    conflict: &EditConflict,
    (save, project, pipelines): (&SaveAction, &OpenProject, Option<&[PipelineChoice]>),
  ) -> Element {
    let choices = self.choices.get();
    let resolved = conflict.resolved(|field| choices.of(field));
    let button = |label: Arc<str>, id: &'static str, all: Option<Choice>| {
      let resolve = self.resolve(conflict, save, project, all);
      let button = ActionButton::new(label)
        .id(id)
        .in_form()
        .fill_width()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(resolve);
      match all {
        None => button.primary().icon(Icon::Check),
        Some(_) => button,
      }
      .render(ctx)
    };
    Column::new()
      .id("task-conflict-panel")
      .width(Dimension::Px(super::plan::PLAN_WIDTH + gutter_width()))
      .height(Dimension::Pct(100.0))
      .padding_left(ExtraSpacing::Rail.role())
      .padding_right(Dimension::Px(gutter_width()))
      .spacing(SpacingSize::Md)
      .overflow_visible()
      .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(super::plan::plan_heading(ctx))
      .child(value_box(
        ctx,
        "task-conflict-pipeline",
        &pipeline_text(ctx, &resolved, pipelines),
        ValueTone::Value,
        None,
      ))
      .with_children(limit_properties(ctx, &resolved))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding_top(SpacingSize::Lg)
          .spacing(SpacingSize::Md)
          .overflow_visible()
          .child(button(ctx.t("tasks.conflict.saveYours"), "task-conflict-save", None))
          .child(button(
            by(ctx, conflict, "tasks.conflict.useTheirs", "tasks.conflict.useSaved"),
            "task-conflict-use-theirs",
            Some(Choice::Theirs),
          ))
          .child(
            Text::new(&by(
              ctx,
              conflict,
              "tasks.conflict.saveNote",
              "tasks.conflict.saveNoteElsewhere",
            ))
            .variant(ExtraTypography::Hint.role())
            .color(PaletteColor::TextMuted)
            .width(Dimension::Pct(100.0)),
          ),
      )
      .into()
  }

  /// Saves the person's choices, or `all` for every field both changed, on top of the saved
  /// version; the form's fields then hold what is saved.
  fn resolve(
    &self,
    conflict: &EditConflict,
    save: &SaveAction,
    project: &OpenProject,
    all: Option<Choice>,
  ) -> impl Fn() + Send + Sync + 'static {
    let (conflict, save, root) = (conflict.clone(), save.clone(), project.repo_root.clone());
    let (choices, shown, reseed) = (self.choices.clone(), self.conflict.clone(), self.reseed.clone());
    move || {
      let picked = choices.get_untracked();
      let details = conflict.resolved(|field| all.unwrap_or_else(|| picked.of(field)));
      reseed.set(Some(Reseed {
        details: details.clone(),
        organization: None,
      }));
      shown.set(None);
      choices.set(Choices::default());
      save.run(SaveArgs {
        repo_root: root.clone(),
        target: SaveTarget::Task(conflict.current.id(), conflict.current.revision()),
        base: conflict.current.details().clone(),
        details,
        organization: conflict.organization.clone(),
        stop: Stop::Unconfirmed,
      });
    }
  }
}

/// Who saved the newer version, when, and what both changed (`Wr0bq`).
fn notice(ctx: &Ctx, conflict: &EditConflict) -> Element {
  let key = conflict.current.key().to_string();
  let heading = match (&conflict.by, conflict.at) {
    (Some(name), Some(at)) => ctx.t_args(
      "tasks.conflict.heading",
      [
        ("name", &**name),
        ("key", key.as_str()),
        ("time", display::clock(at).as_str()),
      ],
    ),
    (Some(name), None) => ctx.t_args(
      "tasks.conflict.headingUntimed",
      [("name", &**name), ("key", key.as_str())],
    ),
    (None, _) => ctx.t_args("tasks.conflict.headingElsewhere", [("key", key.as_str())]),
  };
  let separator = ctx.t("tasks.conflict.fieldSeparator");
  let fields = conflict
    .merge
    .conflicts()
    .into_iter()
    .map(|field| ctx.t(name_key(field)).to_string())
    .collect::<Vec<_>>()
    .join(&separator);
  let detail = match (conflict.merge.kept().is_empty(), &conflict.by) {
    (true, _) => ctx.t_args("tasks.conflict.detail", [("fields", fields.as_str())]),
    (false, Some(name)) => ctx.t_args(
      "tasks.conflict.detailKept",
      [("fields", fields.as_str()), ("name", &**name)],
    ),
    (false, None) => ctx.t_args("tasks.conflict.detailKeptElsewhere", [("fields", fields.as_str())]),
  };
  Notice::new(Icon::GitCompareArrows, heading, detail)
    .emphasized()
    .id("task-conflict-notice")
    .into()
}

/// The text of `named` with the other person's name, or `elsewhere` for a local task.
fn by(ctx: &Ctx, conflict: &EditConflict, named: &str, elsewhere: &str) -> Arc<str> {
  match &conflict.by {
    Some(name) => ctx.t_args(named, [("name", &**name)]),
    None => ctx.t(elsewhere),
  }
}

/// A field as it is saved, when the person has nothing to choose there.
fn field_value(ctx: &Ctx, shown: &Shown<'_>, details: &TaskDetails, field: ContentField) -> Element {
  match field {
    ContentField::Title => value_box(
      ctx,
      "task-conflict-title",
      details.title.as_str(),
      ValueTone::Value,
      None,
    ),
    ContentField::Criteria => criteria_list(ctx, shown, details),
    _ => Text::new(&value_text(ctx, details, field, shown.pipelines))
      .variant(TypographyStyle::Body)
      .width(Dimension::Pct(100.0))
      .into(),
  }
}

/// The criteria (`pIwgS`), the ones the other person added marked as theirs.
fn criteria_list(ctx: &Ctx, shown: &Shown<'_>, details: &TaskDetails) -> Element {
  let merge = &shown.conflict.merge;
  if details.acceptance_criteria.is_empty() {
    return Text::new(&ctx.t("tasks.conflict.empty"))
      .variant(TypographyStyle::Body)
      .color(PaletteColor::TextMuted)
      .into();
  }
  let added = by(
    ctx,
    shown.conflict,
    "tasks.conflict.addedBy",
    "tasks.conflict.addedElsewhere",
  );
  Column::new()
    .id("task-conflict-criteria")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(details.acceptance_criteria.iter().map(|criterion| {
      let theirs =
        merge.theirs().acceptance_criteria.contains(criterion) && !merge.base().acceptance_criteria.contains(criterion);
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::Circle, IconSize::Option, PaletteColor::TextSecondary))
        .child(
          Text::new(criterion)
            .variant(TypographyStyle::Body)
            .color(PaletteColor::TextSecondary),
        )
        .with_children(theirs.then(|| {
          Row::new()
            .padding_horizontal(SpacingSize::Xs)
            .background(PaletteColor::SurfaceRaised)
            .rounded(RadiusSize::Sm)
            .child(
              Text::new(&added)
                .variant(ExtraTypography::Hint.role())
                .color(PaletteColor::TextMuted),
            )
        }))
    }))
    .into()
}

/// Run budget and time limit as they would be saved.
fn limit_properties(ctx: &Ctx, details: &TaskDetails) -> Vec<Element> {
  let limits = details.limits;
  if limits == TaskLimits::NONE {
    return vec![property(
      &ctx.t("tasks.conflict.fields.limits"),
      &ctx.t("tasks.conflict.noLimits"),
    )];
  }
  vec![
    property(&ctx.t("tasks.new.budget"), &budget_text(ctx, limits)),
    property(&ctx.t("tasks.new.timeLimit"), &time_text(ctx, limits)),
  ]
}

/// A task's budget as the conflict view shows it, or that it has no token limit.
fn budget_text(ctx: &Ctx, limits: TaskLimits) -> Arc<str> {
  limits.budget().map_or_else(
    || ctx.t("tokens.noBudget"),
    |budget| {
      ctx.t_args(
        "tasks.conflict.budget",
        [("tokens", compact_tokens(budget.get()).as_str())],
      )
    },
  )
}

/// A task's time limit as the conflict view shows it, or that it has no time limit.
fn time_text(ctx: &Ctx, limits: TaskLimits) -> Arc<str> {
  match limits.max_duration() {
    Some(_) => ctx.t_args(
      "tasks.conflict.timeLimit",
      [("minutes", minutes_text(limits.max_duration()).as_str())],
    ),
    None => ctx.t("tokens.noTimeLimit"),
  }
}

/// What `field` of `details` says, as text.
fn value_text(ctx: &Ctx, details: &TaskDetails, field: ContentField, pipelines: Option<&[PipelineChoice]>) -> String {
  let text = match field {
    ContentField::Title => details.title.as_str().to_owned(),
    ContentField::Brief => details.brief.clone(),
    ContentField::Criteria => details.acceptance_criteria.join("\n"),
    ContentField::Pipeline => pipeline_text(ctx, details, pipelines).to_string(),
    ContentField::Limits if details.limits == TaskLimits::NONE => ctx.t("tasks.conflict.noLimits").to_string(),
    ContentField::Limits => ctx
      .t_args(
        "tasks.conflict.limits",
        [
          ("budget", &*budget_text(ctx, details.limits)),
          ("time", &*time_text(ctx, details.limits)),
        ],
      )
      .to_string(),
  };
  if text.trim().is_empty() {
    ctx.t("tasks.conflict.empty").to_string()
  } else {
    text
  }
}

/// The pipeline `details` name: published here, not published, or none.
fn pipeline_text(ctx: &Ctx, details: &TaskDetails, pipelines: Option<&[PipelineChoice]>) -> Arc<str> {
  let Some(id) = details.pipeline else {
    return ctx.t("tasks.pipeline.notChosen");
  };
  match pipelines.and_then(|choices| choices.iter().find(|choice| choice.id == id)) {
    Some(choice) => ctx.t_args(
      "tasks.pipeline.published",
      [
        ("name", &*choice.name),
        ("version", choice.version.to_string().as_str()),
      ],
    ),
    None => ctx.t("tasks.pipeline.unavailable"),
  }
}

const fn label_key(field: ContentField) -> &'static str {
  match field {
    ContentField::Title => "tasks.new.taskTitle",
    ContentField::Brief => "tasks.new.brief",
    ContentField::Criteria => "tasks.new.criteria",
    ContentField::Pipeline => "tasks.conflict.fields.pipeline",
    ContentField::Limits => "tasks.conflict.fields.limits",
  }
}

/// The field as the notice names it.
const fn name_key(field: ContentField) -> &'static str {
  match field {
    ContentField::Title => "tasks.conflict.fieldNames.title",
    ContentField::Brief => "tasks.conflict.fieldNames.brief",
    ContentField::Criteria => "tasks.conflict.fieldNames.criteria",
    ContentField::Pipeline => "tasks.conflict.fieldNames.pipeline",
    ContentField::Limits => "tasks.conflict.fieldNames.limits",
  }
}

const fn card_id(field: ContentField, choice: Choice) -> &'static str {
  match (field, choice) {
    (ContentField::Title, Choice::Mine) => "task-conflict-title-mine",
    (ContentField::Title, Choice::Theirs) => "task-conflict-title-theirs",
    (ContentField::Brief, Choice::Mine) => "task-conflict-brief-mine",
    (ContentField::Brief, Choice::Theirs) => "task-conflict-brief-theirs",
    (ContentField::Criteria, Choice::Mine) => "task-conflict-criteria-mine",
    (ContentField::Criteria, Choice::Theirs) => "task-conflict-criteria-theirs",
    (ContentField::Pipeline, Choice::Mine) => "task-conflict-pipeline-mine",
    (ContentField::Pipeline, Choice::Theirs) => "task-conflict-pipeline-theirs",
    (ContentField::Limits, Choice::Mine) => "task-conflict-limits-mine",
    (ContentField::Limits, Choice::Theirs) => "task-conflict-limits-theirs",
  }
}
