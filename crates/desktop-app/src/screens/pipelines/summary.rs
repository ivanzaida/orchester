//! The column with no step chosen (`06 · no step chosen` `bCyVI`): the pipeline's name and description, edited in
//! place, its slug and what starts it; its versions, what uses it and its steps; that tasks cannot use it before a
//! version is published (`06c` `T2SLl`), what its import did (`06b` `ky5kJ`); and the ways to its versions and
//! settings.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{Ref, Signal},
  node::{Element, dimension::Dimension},
};

use super::{
  parts,
  state::{Editing, State, View},
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  field::FieldInput,
  icon::Icon,
  notice::{Notice, NoticeTone},
  row_group::{GroupRow, RowGroup, note},
};

/// The name and description fields' width (`Input` `UFHJc`).
const FIELD_WIDTH: f32 = 260.0;

#[derive(Clone)]
pub struct SummaryProps {
  pub state: State,
  pub locked: bool,
}
impl PartialEq for SummaryProps {
  fn eq(&self, other: &Self) -> bool {
    self.locked == other.locked
  }
}
impl DevtoolsInspectable for SummaryProps {}

pub struct Summary {
  name: Signal<String>,
  description: Signal<String>,
  /// Whether the fields are locked, as last shown.
  locked: Ref<bool>,
}
impl Component for Summary {
  type Props = SummaryProps;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<SummaryProps>().clone();
    let editing = props.state.get();
    let summary = Self {
      name: ctx.signal(
        editing
          .inputs
          .get("pipeline-name")
          .cloned()
          .unwrap_or_else(|| editing.name.to_string()),
      ),
      description: ctx.signal(editing.shown().description().to_owned()),
      locked: ctx.create_ref(props.locked),
    };
    follow_identity(
      ctx,
      &props.state,
      (&summary.name, &summary.description),
      &summary.locked,
    );
    summary
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SummaryProps>().clone();
    self.locked.set(props.locked);
    let editing = props.state.get();
    let imported = imported_sections(ctx, &editing);
    let column = Column::new()
      .id("pipeline-summary")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(
        Text::new(&ctx.t("pipelines.pipelineSection"))
          .id("pipeline-summary-title")
          .variant(TypographyStyle::Title),
      );
    let unpublished = editing.opened.latest.is_none();
    // An imported pipeline not published yet shows only what its import did (`06b` `ky5kJ`); its name,
    // description and slug stay on the Settings tab.
    if unpublished && !imported.is_empty() {
      return column.with_children(imported).child(unpublished_notice(ctx));
    }
    let pipeline = self.pipeline_group(ctx, &props, &editing);
    column
      .with_children(imported)
      .child(pipeline)
      .child(versions_group(ctx, &editing))
      .child(if unpublished {
        unpublished_notice(ctx)
      } else {
        note("pipeline-versions-note", &ctx.t("pipelines.versionsExplanation")).into()
      })
      .child(links(ctx, &editing))
  }
}

impl Summary {
  fn pipeline_group(&self, ctx: &mut Ctx, props: &SummaryProps, editing: &Editing) -> RowGroup {
    let mut name = FieldInput::new("pipeline-summary-name", ctx.t("pipelines.name"), self.name.clone())
      .width(Dimension::Px(FIELD_WIDTH));
    if let Some(error) = editing.errors.get("pipeline-name") {
      name = name.error(parts::pipeline_name_error(ctx, error));
    }
    let name = if props.locked { name.locked() } else { name }.render(ctx);
    let description = FieldInput::new(
      "pipeline-description",
      ctx.t("pipelines.description"),
      self.description.clone(),
    )
    .compact_area()
    .placeholder(ctx.t("pipelines.descriptionPlaceholder"))
    .width(Dimension::Px(FIELD_WIDTH));
    let description = if props.locked {
      description.locked()
    } else {
      description
    }
    .render(ctx);
    RowGroup::new("pipeline-summary-pipeline", ctx.t("pipelines.pipelineSection"))
      .row(GroupRow::new("pipeline-summary-name-row", ctx.t("pipelines.name")).end(name))
      .row(GroupRow::new("pipeline-description-row", ctx.t("pipelines.description")).end(description))
      .row(GroupRow::new("pipeline-slug", ctx.t("pipelines.slug")).mono_value(&editing.opened.slug))
      .row(GroupRow::new("pipeline-starts", ctx.t("pipelines.starts")).value(&ctx.t("pipelines.onTaskStart")))
  }
}

/// That New task lists only published pipelines (`Notice` in `06c` and `06b`).
fn unpublished_notice(ctx: &Ctx) -> Element {
  Notice::new(
    "pipeline-unpublished-note",
    NoticeTone::Info,
    ctx.t("pipelines.notAvailableTitle"),
  )
  .flat()
  .body(ctx.t("pipelines.notAvailableDetail"))
  .into()
}

fn versions_group(ctx: &Ctx, editing: &Editing) -> RowGroup {
  let used = if editing.opened.used_by == 0 {
    ctx.t("pipelines.notUsed")
  } else {
    ctx.t_args("pipelines.tasks", [("count", editing.opened.used_by.to_string())])
  };
  RowGroup::new("pipeline-summary-versions", ctx.t("pipelines.versions"))
    .row(GroupRow::new("pipeline-published", ctx.t("pipelines.publishedLabel")).value(&published(ctx, editing)))
    .row(GroupRow::new("pipeline-draft-state", ctx.t("pipelines.draftLabel")).value(&draft(ctx, editing)))
    .row(GroupRow::new("pipeline-used-by", ctx.t("pipelines.usedBy")).value(&used))
    .row(GroupRow::new("pipeline-step-count", ctx.t("pipelines.steps")).value(&steps(ctx, &editing.shown())))
}

/// *Version history* and *Pipeline settings* (`Links` `vJyWZ`).
fn links(ctx: &Ctx, editing: &Editing) -> Row {
  let (versions, settings) = (editing.views.clone(), editing.views.clone());
  Row::new()
    .spacing(SpacingSize::Sm)
    .child(
      ActionButton::new(
        "pipeline-summary-history",
        ButtonKind::Secondary,
        ctx.t("pipelines.history"),
      )
      .icon(Icon::History)
      .on_click(move || versions.tab(View::Versions)),
    )
    .child(
      ActionButton::new(
        "pipeline-summary-settings",
        ButtonKind::Secondary,
        ctx.t("pipelines.settings"),
      )
      .icon(Icon::Settings)
      .on_click(move || settings.tab(View::Settings)),
    )
}

/// Takes what is typed into the pipeline's `name` and `description` into the draft, once per form, unless the
/// form is `locked` now. Shared by the summary and the Settings tab.
pub(super) fn follow_identity(
  ctx: &mut Ctx,
  state: &State,
  (name, description): (&Signal<String>, &Signal<String>),
  locked: &Ref<bool>,
) {
  let (rename, unlocked) = (state.clone(), locked.clone());
  ctx.watch(name, move |text| {
    if !unlocked.get() {
      rename.update(|s| {
        s.inputs.insert("pipeline-name".to_owned(), text.clone());
        s.field(
          "pipeline-name",
          orchester_workspace::pipelines::fields::parse_name(text),
          |s, name| s.name = name,
        );
      });
    }
  });
  let (state, locked) = (state.clone(), locked.clone());
  ctx.watch(description, move |text| {
    if !locked.get() {
      state.update(|s| s.edit.set_description(text));
    }
  });
}

/// What an imported pipeline's summary says of its import (`Imported` `W1I5r6` in `ky5kJ`); nothing for a pipeline
/// made here.
fn imported_sections(ctx: &mut Ctx, editing: &Editing) -> Vec<Element> {
  let (Some(imported), Some(project)) = (
    editing.opened.imported.clone(),
    ctx
      .use_context::<super::sources::EditorProject>()
      .map(|project| project.0),
  ) else {
    return Vec::new();
  };
  crate::screens::import::imported::sections(ctx, (project.project.id(), project.project.name().as_str()), &imported)
}

pub fn published(ctx: &Ctx, editing: &Editing) -> std::sync::Arc<str> {
  editing.opened.latest.as_ref().map_or_else(
    || ctx.t("pipelines.never"),
    |v| {
      ctx.t_args(
        "pipelines.publishedAt",
        [
          ("version", v.number.get().to_string()),
          ("age", super::library::age(ctx, v.at).to_string()),
        ],
      )
    },
  )
}
/// The draft with its number: *v4 · 2 changes since v3*, *Draft v1*, or that the next edit starts one.
pub fn draft(ctx: &Ctx, editing: &Editing) -> std::sync::Arc<str> {
  let number = orchester_workspace::pipelines::write::next_version(&editing.opened)
    .map_or_else(String::new, |v| v.get().to_string());
  if editing.opened.draft_saved_at.is_none() && !editing.unsaved() {
    return ctx.t_args("pipelines.noDraftNext", [("version", number)]);
  }
  if !editing.edit.steps().is_empty() && editing.opened.latest.is_some() && editing.edit.validate().is_err() {
    return ctx.t("pipelines.unsaved");
  }
  match draft_detail(ctx, editing) {
    Some(detail) => ctx.t_args(
      "pipelines.draftWith",
      [("version", number), ("detail", detail.to_string())],
    ),
    None => ctx.t_args("pipelines.draftVersion", [("version", number)]),
  }
}

/// What the draft holds, without its number: *no steps yet* or *2 changes since v3*; `None` for a first draft with
/// steps, one that does not validate, or one that changes nothing yet.
pub fn draft_detail(ctx: &Ctx, editing: &Editing) -> Option<std::sync::Arc<str>> {
  if editing.edit.steps().is_empty() {
    return Some(ctx.t("pipelines.noStepsYet"));
  }
  let latest = editing.opened.latest.as_ref()?;
  let body = editing.edit.validate().ok()?;
  let changes = orchester_core::resource::changes(&latest.body, &body).len();
  if changes == 0 {
    return None;
  }
  Some(ctx.t_args(
    if changes == 1 {
      "pipelines.oneChangeSince"
    } else {
      "pipelines.changesSince"
    },
    [
      ("count", changes.to_string()),
      ("base", latest.number.get().to_string()),
    ],
  ))
}
pub fn steps(ctx: &Ctx, shown: &orchester_core::resource::PipelineEdit) -> String {
  let gates = shown.steps().iter().filter(|s| s.approval_gate).count();
  if shown.steps().is_empty() {
    return ctx.t("pipelines.noSteps").to_string();
  }
  ctx
    .t_args(
      if gates == 1 {
        "pipelines.stepsOneGate"
      } else {
        "pipelines.stepsGates"
      },
      [("count", shown.steps().len().to_string()), ("gates", gates.to_string())],
    )
    .to_string()
}
