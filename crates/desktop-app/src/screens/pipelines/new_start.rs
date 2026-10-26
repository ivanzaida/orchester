//! New pipeline's Start from (`Start from` `t0qET` in `uHPNu`): the starter with its steps and counts, a copy of a
//! published pipeline chosen in a dropdown, or an empty pipeline, as radio rows under what the choice does.
use lurq::{
  app::ctx::Ctx,
  components::SelectOption,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::pipelines::{
  levels::levels,
  library::{LibraryEntry, VersionState},
};

use super::new_pipeline::{Begin, starter};
use crate::ui::{
  field::SelectField,
  pressable::Press,
  row_group::{GroupRow, RowGroup},
  toggle::radio_mark,
};

/// The copy dropdown's width (`Select` `gBMqg`).
const COPY_WIDTH: f32 = 220.0;

pub fn group(ctx: &mut Ctx, (start, copy): (&Signal<Begin>, &Signal<String>), copies: &[LibraryEntry]) -> Element {
  let chosen = start.get();
  let row = |id: &str, title: &str, begin: Begin| {
    let start = start.clone();
    GroupRow::new(id, ctx.t(title))
      .leading(radio_mark(chosen == begin))
      .press(Press::run(move || start.set(begin)))
  };
  let starter_row = row("pipeline-start-0", "pipelines.starter", Begin::Starter)
    .meta(ctx.t("pipelines.starterHint"))
    .hint(starter_chain(ctx));
  let select = SelectField::new(
    "pipeline-copy",
    ctx.t("pipelines.copy"),
    copy.clone(),
    copies
      .iter()
      .map(|p| {
        let version = match p.state {
          VersionState::Published(v) | VersionState::PublishedWithDraft(v) => v.get().to_string(),
          VersionState::DraftOnly => String::new(),
        };
        SelectOption::new(
          p.id.to_string(),
          ctx.t_args(
            "pipelines.copyOption",
            [("name", p.name.to_string()), ("version", version)],
          ),
        )
      })
      .collect(),
  )
  .placeholder(ctx.t("pipelines.choosePipeline"))
  .width(Dimension::Px(COPY_WIDTH))
  .render(ctx);
  let copy_row = row("pipeline-start-1", "pipelines.copy", Begin::Copy)
    .hint(ctx.t("pipelines.copyHint"))
    .end(select);
  let empty_row = row("pipeline-start-2", "pipelines.empty", Begin::Empty).hint(ctx.t("pipelines.emptyHint"));
  RowGroup::new("pipeline-start", ctx.t("pipelines.startFrom"))
    .lead(ctx.t("pipelines.startHint"))
    .row(starter_row)
    .row(copy_row)
    .row(empty_row)
    .into()
}

/// The starter's steps by stage, parallel ones joined (`Research → Plan → … → Run checks ∥ Review → …`).
fn starter_chain(ctx: &Ctx) -> String {
  let pipeline = starter(ctx).pipeline(&Default::default());
  let (then, parallel) = (ctx.t("pipelines.then"), ctx.t("pipelines.parallel"));
  levels(pipeline.steps())
    .iter()
    .map(|level| {
      level
        .iter()
        .map(|index| pipeline.steps()[*index].name.as_str().to_owned())
        .collect::<Vec<_>>()
        .join(&parallel)
    })
    .collect::<Vec<_>>()
    .join(&then)
}
