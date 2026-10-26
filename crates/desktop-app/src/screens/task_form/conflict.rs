//! A newer version of the task met under edits (`Edit task · conflict · newer version` `PenUu`): a
//! notice of who saved it, when the server names them, else neutral words; then each field both versions
//! changed to different values, with the person's version and the newer one side by side to choose
//! from. Fields only one version changed need no choice: they are kept as that version has them
//! (`tasks::conflict`). Saving the chosen versions is made on the newer revision, so a change saved after
//! it is met again.

use std::{collections::BTreeMap, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  task::{
    TaskDetails, TaskLimits,
    content::{Choice, ContentField},
  },
  tokens::compact_tokens,
};
use orchester_workspace::{
  budget::whole_minutes,
  display,
  tasks::{conflict::EditConflict, data::PipelineChoice},
};

use super::plan::choice_label;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    form_section::section,
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
    pressable::{Focus, pressable},
    toggle::{Toggle, ToggleKind},
  },
};

/// A newer version met under edits, waiting for the person's choices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict(pub Arc<EditConflict>);

impl lurq::app::component::DevtoolsInspectable for Conflict {}

/// The version the person chose for each conflicting field; a field not in it is not chosen yet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choices(pub BTreeMap<ContentField, Choice>);

impl lurq::app::component::DevtoolsInspectable for Choices {}

impl Conflict {
  /// The fields to choose for, in the form's order.
  pub fn fields(&self) -> Vec<ContentField> {
    self.0.merge.conflicts()
  }

  /// The fields still to choose for.
  pub fn unchosen(&self, choices: &Choices) -> Vec<ContentField> {
    self
      .fields()
      .into_iter()
      .filter(|field| !choices.0.contains_key(field))
      .collect()
  }

  /// What saving the chosen versions stores; `None` until every field is chosen.
  pub fn resolved(&self, choices: &Choices) -> Option<TaskDetails> {
    self.unchosen(choices).is_empty().then(|| {
      self
        .0
        .resolved(|field| choices.0.get(&field).copied().unwrap_or_default())
    })
  }
}

/// The i18n key of a field's name.
pub const fn field_key(field: ContentField) -> &'static str {
  match field {
    ContentField::Title => "task_form.fields.title.label",
    ContentField::Brief => "task_form.fields.brief.label",
    ContentField::Criteria => "task_form.fields.criteria.label",
    ContentField::Pipeline => "task_form.fields.pipeline.label",
    ContentField::Limits => "task_form.fields.limits.label",
  }
}

const fn field_id(field: ContentField) -> &'static str {
  match field {
    ContentField::Title => "title",
    ContentField::Brief => "brief",
    ContentField::Criteria => "criteria",
    ContentField::Pipeline => "pipeline",
    ContentField::Limits => "limits",
  }
}

/// The page's parts in place of the form's sections: the notice, the fields that differ and how many
/// need no choice.
pub fn conflict_parts(
  ctx: &mut Ctx,
  conflict: &Conflict,
  choices: &Signal<Choices>,
  pipelines: Option<&[PipelineChoice]>,
) -> Vec<Element> {
  let chosen = choices.get();
  let fields = conflict.fields();
  let rows: Vec<Element> = fields
    .iter()
    .enumerate()
    .map(|(index, field)| {
      let last = index + 1 == fields.len();
      field_row(ctx, conflict, (*field, last), (&chosen, choices), pipelines)
    })
    .collect();
  let meta = ctx.t_args(
    if fields.len() == 1 {
      "task_form.conflict.meta.one"
    } else {
      "task_form.conflict.meta.other"
    },
    [
      ("count", fields.len().to_string()),
      ("chosen", (fields.len() - conflict.unchosen(&chosen).len()).to_string()),
    ],
  );
  let title = ctx.t("task_form.conflict.section");
  vec![
    notice(ctx, conflict),
    section("task-form-conflict-fields", &title, Some(&meta), rows).into(),
    same_line(ctx, ContentField::ALL.len() - fields.len()),
  ]
}

/// Whose the newer version is: the person the server names, or neutral words.
fn theirs_name(ctx: &Ctx, conflict: &Conflict) -> Arc<str> {
  conflict.0.by.as_deref().map_or_else(
    || ctx.t("task_form.conflict.theirs"),
    |name| ctx.t_args("task_form.conflict.theirsBy", [("name", name)]),
  )
}

fn notice(ctx: &Ctx, conflict: &Conflict) -> Element {
  let key = conflict.0.current.key().to_string();
  let title = match conflict.0.by.as_deref() {
    Some(name) => ctx.t_args("task_form.conflict.noticeBy", [("name", name), ("key", key.as_str())]),
    None => ctx.t_args("task_form.conflict.notice", [("key", key.as_str())]),
  };
  Notice::new("task-form-conflict-notice", NoticeTone::Warning, title)
    .icon(Icon::Users)
    .body(ctx.t("task_form.conflict.noticeBody"))
    .into()
}

fn field_row(
  ctx: &mut Ctx,
  conflict: &Conflict,
  (field, last): (ContentField, bool),
  (chosen, choices): (&Choices, &Signal<Choices>),
  pipelines: Option<&[PipelineChoice]>,
) -> Element {
  let choice = chosen.0.get(&field).copied();
  let state = match choice {
    Some(Choice::Mine) => Text::new(&ctx.t("task_form.conflict.chosenMine"))
      .variant(TypographyStyle::Caption)
      .into(),
    Some(Choice::Theirs) => {
      let words = conflict.0.by.as_deref().map_or_else(
        || ctx.t("task_form.conflict.chosenNewer"),
        |name| ctx.t_args("task_form.conflict.chosenTheirs", [("name", name)]),
      );
      Text::new(&words).variant(TypographyStyle::Caption).into()
    }
    None => pick(ctx),
  };
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t(field_key(field))).variant(ExtraTypography::Strong.role()))
    .child(Spacer::new().flex(1.0))
    .child(state);
  let merge = &conflict.0.merge;
  let mine = value_text(ctx, field, merge.mine(), pipelines);
  let theirs = value_text(ctx, field, merge.theirs(), pipelines);
  let theirs_meta = conflict.0.at.map(|at| {
    ctx
      .t_args("task_form.conflict.savedAt", [("time", display::clock(at))])
      .to_string()
  });
  let versions = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(card(
      &Card {
        id: format!("task-form-conflict-{}-mine", field_id(field)),
        who: ctx.t("task_form.conflict.mine"),
        meta: Some(ctx.t("task_form.conflict.notSaved").to_string()),
        value: mine,
        on: choice == Some(Choice::Mine),
      },
      choose(choices, field, Choice::Mine),
    ))
    .child(card(
      &Card {
        id: format!("task-form-conflict-{}-theirs", field_id(field)),
        who: theirs_name(ctx, conflict),
        meta: theirs_meta,
        value: theirs,
        on: choice == Some(Choice::Theirs),
      },
      choose(choices, field, Choice::Theirs),
    ));
  let row = Column::new()
    .id(format!("task-form-conflict-{}", field_id(field)))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .child(head)
    .child(versions);
  if last {
    row.into()
  } else {
    row
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .into()
  }
}

/// *Choose one* (`Pick` `GsQC3`): a field not chosen yet.
fn pick(ctx: &Ctx) -> Element {
  Row::new()
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Hand, IconSize::Sm, PaletteColor::Warning))
    .child(
      Text::new(&ctx.t("task_form.conflict.chooseOne"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::Warning),
    )
    .into()
}

fn choose(choices: &Signal<Choices>, field: ContentField, choice: Choice) -> impl Fn() + Send + Sync + Clone + 'static {
  let choices = choices.clone();
  move || {
    choices.update(|choices| {
      choices.0.insert(field, choice);
    });
  }
}

/// One version of a field.
struct Card {
  id: String,
  who: Arc<str>,
  meta: Option<String>,
  value: String,
  on: bool,
}

/// A version to choose (`Your version` `RdXUb`, `Sam's version` `B9rce`): its radio, whose it is and
/// when it was saved, and the field's value; the chosen one on `accent_muted` in an `accent` stroke.
fn card(card: &Card, choose: impl Fn() + Send + Sync + Clone + 'static) -> Element {
  let radio = Toggle::new(
    format!("{}-radio", card.id),
    ToggleKind::Radio,
    card.on,
    card.who.clone(),
  )
  .hide_label()
  .on_change({
    let choose = choose.clone();
    move |_| choose()
  });
  let head = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(radio)
    .child(Text::new(&card.who).variant(TypographyStyle::Label))
    .with_children(
      card
        .meta
        .as_deref()
        .map(|meta| Text::new(meta).variant(TypographyStyle::Caption)),
    );
  let (fill, stroke) = if card.on {
    (PaletteColor::AccentMuted, PaletteColor::Accent)
  } else {
    (PaletteColor::SurfaceInput, PaletteColor::Border)
  };
  // The card is pressed with the pointer; its radio is the keyboard's stop, so the chosen card has one
  // outline, never a focus ring beside it.
  Focus::Pointer
    .apply(pressable(choose))
    .id(card.id.clone())
    .flex(1.0)
    .describe("checked", if card.on { "true" } else { "false" })
    .describe("a11y_name", card.who.clone())
    .padding(Padding::all(SpacingSize::Md))
    .rounded(RadiusSize::Md)
    .background(fill)
    .border(Border::inside(BorderSize::Sm, stroke))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(head)
        .child(
          Text::new(&card.value)
            .id(format!("{}-value", card.id))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextPrimary),
        ),
    )
    .into()
}

/// A field's value as a version has it, in words.
fn value_text(ctx: &Ctx, field: ContentField, details: &TaskDetails, pipelines: Option<&[PipelineChoice]>) -> String {
  match field {
    ContentField::Title => details.title.as_str().to_owned(),
    ContentField::Brief if details.brief.trim().is_empty() => ctx.t("task_form.conflict.empty").to_string(),
    ContentField::Brief => details.brief.clone(),
    ContentField::Criteria if details.acceptance_criteria.is_empty() => {
      ctx.t("task_form.conflict.noCriteria").to_string()
    }
    ContentField::Criteria => (1..)
      .zip(&details.acceptance_criteria)
      .map(|(number, criterion)| {
        ctx
          .t_args(
            "task_form.conflict.criterion",
            [("number", number.to_string()), ("text", criterion.clone())],
          )
          .to_string()
      })
      .collect::<Vec<_>>()
      .join("\n"),
    ContentField::Pipeline => pipeline_text(ctx, details, pipelines),
    ContentField::Limits => limits_text(ctx, details.limits),
  }
}

/// "1.4M tokens · 30 minutes", one of them "no token limit" / "no time limit", or "No limits" for a task
/// without either.
fn limits_text(ctx: &Ctx, limits: TaskLimits) -> String {
  if limits == TaskLimits::NONE {
    return ctx.t("task_form.conflict.noLimits").to_string();
  }
  let budget = limits.budget().map_or_else(
    || ctx.t("task_form.conflict.noBudget"),
    |budget| ctx.t_args("task_form.conflict.budget", [("tokens", compact_tokens(budget.get()))]),
  );
  let time = limits.max_duration().map_or_else(
    || ctx.t("task_form.conflict.noTimeLimit"),
    |limit| {
      ctx.t_args(
        "task_form.conflict.timeLimit",
        [("minutes", whole_minutes(limit).to_string())],
      )
    },
  );
  ctx
    .t_args(
      "task_form.conflict.limits",
      [("budget", budget.to_string()), ("time", time.to_string())],
    )
    .to_string()
}

fn pipeline_text(ctx: &Ctx, details: &TaskDetails, pipelines: Option<&[PipelineChoice]>) -> String {
  let Some(id) = details.pipeline else {
    return ctx.t("task_form.conflict.noPipeline").to_string();
  };
  pipelines
    .and_then(|pipelines| pipelines.iter().find(|choice| choice.id == id))
    .map_or_else(
      || ctx.t("task_form.conflict.unpublishedPipeline").to_string(),
      |choice| choice_label(ctx, choice).to_string(),
    )
}

/// How many content fields need no choice (`Same` `YTSin`).
fn same_line(ctx: &Ctx, count: usize) -> Element {
  let key = if count == 1 {
    "task_form.conflict.same.one"
  } else {
    "task_form.conflict.same.other"
  };
  Row::new()
    .id("task-form-conflict-same")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Equal, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t_args(key, [("count", count.to_string())]))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .into()
}
