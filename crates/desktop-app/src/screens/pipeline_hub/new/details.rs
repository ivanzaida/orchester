//! *Pipeline* in a new pipeline's panel (`Pipeline` `f6sZV` in P14c `g09dL`): *Description*, a box, and *Slug*, made
//! from the name and not typed (*Made from the name. Fixed once saved.*). A slug another pipeline has, archived or not,
//! says whose it is in `danger` with *Open …* at the row's end (P14a `d4f9Vi`), which leaves for that pipeline, asking
//! first while anything is typed.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::dimension::Dimension,
};
use orchester_core::id::ProjectId;

use crate::{
  routes::{PipelinesFilter, PipelinesView, ProjectPage, Route},
  screens::{
    local_hub::leave::Leave,
    pipeline_hub::model::Entry,
    team_hub::rows::{self, Sub},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::Icon,
  },
};

/// What the group is drawn from.
pub(super) struct Details<'a> {
  pub project: ProjectId,
  pub description: &'a Signal<String>,
  /// The slug the name makes; `None` while it makes none.
  pub slug: Option<&'a str>,
  /// The pipeline that already has it.
  pub taken_by: Option<&'a Entry>,
  /// What a press that leaves the panel does: it asks first while anything is typed.
  pub leave: &'a Leave,
  pub locked: bool,
}

pub(super) fn group(ctx: &mut Ctx, details: &Details<'_>) -> Column {
  let rows = vec![description(ctx, details), slug(ctx, details)];
  rows::section(
    "pipeline-new-details",
    &ctx.t("pipeline_new.details.title"),
    Vec::new(),
    rows,
  )
}

fn description(ctx: &mut Ctx, details: &Details<'_>) -> Row {
  let label = ctx.t("pipeline_new.details.description");
  let mut input = FieldInput::new("pipeline-new-description", label.clone(), details.description.clone())
    .placeholder(ctx.t("pipeline_new.details.descriptionNone"));
  if details.locked {
    input = input.locked();
  }
  rows::row("pipeline-new-description-row", &label, None, input.render(ctx))
}

/// *Slug*: the slug the name makes (`—` while it makes none), and whose it is when it is taken.
fn slug(ctx: &Ctx, details: &Details<'_>) -> Row {
  let sub = match details.taken_by {
    Some(entry) => Sub {
      words: ctx.t_args("pipeline_new.details.slugTaken", [("name", entry.name.to_string())]),
      color: PaletteColor::Danger,
    },
    None => Sub::quiet(ctx.t("pipeline_new.details.slugSub")),
  };
  let value = match details.slug {
    Some(slug) => rows::mono("pipeline-new-slug", slug),
    None => rows::mono("pipeline-new-slug", &ctx.t("pipeline_new.details.noSlug")).color(PaletteColor::TextMuted),
  };
  let open = details.taken_by.map(|entry| {
    // An archived pipeline is listed only under Archived.
    let filter = if entry.archived.is_some() {
      PipelinesFilter::Archived
    } else {
      PipelinesFilter::All
    };
    let route = Route::Project(
      details.project,
      ProjectPage::pipelines(PipelinesView::new(filter, Some(entry.id))),
    );
    ActionButton::new(
      "pipeline-new-slug-open",
      ButtonKind::Ghost,
      ctx.t_args("pipeline_new.details.open", [("name", entry.name.to_string())]),
    )
    .icon(Icon::ArrowUpRight)
    .press(details.leave.press(route))
  });
  let value = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(value)
    .with_children(open);
  rows::row(
    "pipeline-new-slug-row",
    &ctx.t("pipeline_new.details.slug"),
    Some(sub),
    value,
  )
}
