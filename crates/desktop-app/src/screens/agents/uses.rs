//! *Used in pipelines* beside a profile's page (`Used in pipelines` `q84k9m` in 01c): one row per step with its
//! pipeline and version, the model it runs, its permissions and its runs, each with its own Open pipeline, which says
//! why it waits while the profile has a change or a question pending; the counts and what publishing changes under
//! them.
use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::agents::usage::PipelineUse;

use super::editor::Props;
use crate::{
  navigation::Navigation,
  routes::{PipelineView, ProjectPage, Route},
  screens::hub::parts::{Entry, boxed, group, head, note, quiet},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

pub fn render(ctx: &mut Ctx, props: &Props, blocked: Option<&'static str>) -> Element {
  let profile = props.entry.as_ref().map(|e| e.id);
  let steps: Vec<&PipelineUse> = props
    .listed
    .usage
    .steps
    .iter()
    .filter(|s| Some(s.profile) == profile)
    .collect();
  let pipelines = profile.map_or(0, |id| props.listed.usage.count(id));
  let rows: Vec<Element> = if steps.is_empty() {
    vec![Entry::new("agent-uses-none", ctx.t("agents.noUses")).into()]
  } else {
    steps
      .iter()
      .enumerate()
      .map(|(index, step)| use_row(ctx, props, (index, step), blocked))
      .collect()
  };
  let mut section = Column::new()
    .id("agent-uses")
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .child(group(
      "agent-uses-group",
      head(
        "agent-uses-group",
        &ctx.t("agents.usedTitle"),
        Some(&pipelines.to_string()),
        None,
      ),
      boxed("agent-uses-rows", rows),
    ));
  if !steps.is_empty() {
    let counts = ctx.t_args(
      "agents.useCounts",
      [
        ("pipelines", pipelines.to_string()),
        ("steps", steps.len().to_string()),
        (
          "overrides",
          steps.iter().filter(|s| s.model_overridden).count().to_string(),
        ),
      ],
    );
    section = section.child(quiet(&counts).id("agent-use-counts"));
  }
  section = section.child(note("agent-uses-note", Icon::Info, &ctx.t("agents.usesNote")));
  if let Some(reason) = blocked {
    section = section.child(note("agent-uses-blocked", Icon::Lock, &ctx.t(reason)));
  }
  section.into()
}

fn use_row(
  ctx: &mut Ctx,
  props: &Props,
  (index, step): (usize, &PipelineUse),
  blocked: Option<&'static str>,
) -> Element {
  let position = format!("{:02}", step.position);
  let title = ctx.t_args(
    if step.draft {
      "agents.useDraftTitle"
    } else {
      "agents.useTitle"
    },
    [
      ("pipeline", step.name.to_string()),
      ("version", step.version.to_string()),
      ("position", position),
      ("step", step.step.to_string()),
    ],
  );
  Entry::new(format!("agent-use-{index}"), title)
    .hint(detail(ctx, step))
    .end(open_pipeline(ctx, props, (index, step), blocked))
    .into()
}

/// `claude-opus-5-5 · From the profile · Same as the profile · 1 running`.
fn detail(ctx: &Ctx, step: &PipelineUse) -> Arc<str> {
  let model = step
    .model
    .as_ref()
    .map_or_else(|| ctx.t("agents.chooseConnection"), |m| m.as_str().into());
  let runs = if step.draft {
    ctx.t("agents.draftNoRuns")
  } else if step.running == 0 {
    ctx.t("agents.noRuns")
  } else {
    ctx.t_args("agents.runningCount", [("count", step.running.to_string())])
  };
  ctx.t_args(
    "agents.useDetail",
    [
      ("model", model.to_string()),
      (
        "source",
        ctx
          .t(if step.model_overridden {
            "agents.override"
          } else {
            "agents.fromProfile"
          })
          .to_string(),
      ),
      (
        "permissions",
        ctx
          .t(if step.narrowed {
            "agents.narrowed"
          } else {
            "agents.samePermissions"
          })
          .to_string(),
      ),
      ("runs", runs.to_string()),
    ],
  )
}

fn open_pipeline(
  ctx: &mut Ctx,
  props: &Props,
  (index, step): (usize, &PipelineUse),
  blocked: Option<&'static str>,
) -> Element {
  let button = ActionButton::new(
    format!("agent-use-open-{index}"),
    ButtonKind::Ghost,
    ctx.t("agents.openPipeline"),
  )
  .icon(Icon::ArrowUpRight);
  match blocked {
    Some(reason) => button.unavailable(ctx.t(reason)).into(),
    None => {
      let navigation = Navigation::of(ctx);
      let route = Route::Project(
        props.project.open.project.id(),
        ProjectPage::Pipeline(step.pipeline, PipelineView::default()),
      );
      button.opens(navigation.to(route)).into()
    }
  }
}
