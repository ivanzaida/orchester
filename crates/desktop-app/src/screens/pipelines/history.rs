//! Version history beside the read-only graph, OchSR; published versions never mutate.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::PipelineEdit;
use orchester_workspace::{
  background, messages,
  pipelines::{describe, history::VersionHistory},
};

use super::{
  parts,
  state::{Panel, State},
};
use crate::{
  project::ShownProject,
  ui::{
    button::{ActionButton, ButtonKind},
    message::message_text,
    state::loading,
  },
};

#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub state: State,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
  }
}
impl DevtoolsInspectable for Props {}
pub struct History;
impl Component for History {
  type Props = Props;
  fn create(_: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    panel(ctx, &props.project, &props.state)
  }
}
fn panel(ctx: &mut Ctx, project: &ShownProject, state: &State) -> Element {
  let editing = state.get();
  let loaded = ctx
    .future(
      (project.open.repo_root.clone(), editing.opened.id, editing.epoch),
      |(root, id, _)| async move {
        background::run(move || VersionHistory::load_in(&root, id).map_err(|e| messages::store_error(&e))).await?
      },
    )
    .state()
    .get();
  let mut body = parts::panel_body();
  match loaded.data {
    Some(history) => {
      if history.versions().is_empty() {
        body = body.child(parts::note(&ctx.t("pipelines.historyEmpty")));
      }
      for version in history.versions() {
        let state = state.clone();
        let snapshot = version.body.clone();
        let number = version.number;
        let label = ctx.t_args("pipelines.version", [("version", version.number.get().to_string())]);
        let view = ActionButton::new(
          format!("pipeline-version-{}", version.number.get()),
          ButtonKind::Secondary,
          label,
        )
        .on_click(move || {
          state.update(|s| {
            s.viewing = Some((*snapshot).clone());
            s.viewing_version = Some(number);
            s.epoch += 1;
          })
        });
        body = body.child(view).child(parts::caption(
          &ctx.t_args("pipelines.runs", [("count", version.runs.len().to_string())]),
        ));
        if let Some(changes) = &version.changes {
          for change in &changes.changes {
            body = body.child(parts::note(&message_text(ctx, &describe::change(change))));
          }
        } else {
          body = body.child(parts::note(&ctx.t("pipelines.firstVersion")));
        }
      }
    }
    None => {
      body = body.child(loading("pipeline-history-loading", &ctx.t("pipelines.reading")));
    }
  }
  if let Some(error) = loaded.error {
    body = body.child(crate::ui::field::field_error(
      "pipeline-history-error",
      &message_text(ctx, &error),
    ));
  }
  let footer = footer(ctx, state);
  parts::panel(
    ctx,
    &ctx.t("pipelines.history"),
    &editing.opened.name,
    body.into(),
    Some(footer.into()),
  )
}
fn footer(ctx: &Ctx, state: &State) -> Column {
  let editing = state.get();
  let back = state.clone();
  let restore = state.clone();
  let mut footer = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .child(
      ActionButton::new(
        "pipeline-back-draft",
        ButtonKind::Secondary,
        ctx.t("pipelines.returnDraft"),
      )
      .on_click(move || {
        back.update(|s| {
          s.viewing = None;
          s.viewing_version = None;
          s.panel = Panel::Pipeline;
          s.epoch += 1;
        })
      }),
    );
  if editing.viewing.is_some() {
    footer = footer.child(
      ActionButton::new("pipeline-restore", ButtonKind::Secondary, ctx.t("pipelines.restore")).on_click(move || {
        restore.update(|s| {
          if let Some(body) = s.viewing.take() {
            s.edit = PipelineEdit::new(body);
            s.viewing_version = None;
            s.panel = Panel::Pipeline;
            s.epoch += 1;
            s.errors.clear();
            s.inputs.clear();
          }
        })
      }),
    );
  }
  footer
}
