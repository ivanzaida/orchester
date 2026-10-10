//! Version history beside the read-only graph, OchSR; published versions never mutate.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  node::{Element, border::Border, dimension::Dimension},
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
      let draft = state.clone();
      body = body.child(
        crate::ui::pressable::pressable(move || back(&draft))
          .id("pipeline-history-draft")
          .width(Dimension::Pct(100.0))
          .padding(SpacingSize::Sm)
          .child(
            Column::new()
              .spacing(SpacingSize::Xs)
              .child(Text::new(&ctx.t("pipelines.draftLabel")).variant(crate::theme::ExtraTypography::Strong.role()))
              .child(parts::caption(&ctx.t("pipelines.saved"))),
          ),
      );
      for version in history.versions() {
        body = body.child(version_row(
          ctx,
          state,
          version,
          history.latest().is_some_and(|v| v.number == version.number),
        ));
      }
      if let Some(version) = editing.viewing_version.and_then(|n| history.version(n)) {
        body = details(ctx, state, version, body);
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
    Some(state.clone()),
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
        "pipeline-history-back",
        ButtonKind::Secondary,
        ctx.t("pipelines.returnDraft"),
      )
      .on_click(move || {
        back.update(|s| {
          s.viewing = None;
          s.viewing_version = None;
          s.comparing = false;
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
            s.comparing = false;
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
fn version_row(
  ctx: &Ctx,
  state: &State,
  version: &orchester_workspace::pipelines::history::PublishedVersion,
  latest: bool,
) -> Element {
  let selected = state.get().viewing_version == Some(version.number);
  let target = state.clone();
  let snapshot = version.body.clone();
  let number = version.number;
  let mut head = Row::new().spacing(SpacingSize::Sm).child(
    Text::new(&ctx.t_args("pipelines.version", [("version", number.get().to_string())]))
      .variant(crate::theme::ExtraTypography::Strong.role()),
  );
  if latest {
    head = head.child(parts::caption(&ctx.t("pipelines.latest")).color(PaletteColor::Success));
  }
  let meta = ctx.t_args(
    "pipelines.historyMeta",
    [
      ("age", super::library::age(ctx, version.published_at).to_string()),
      (
        "changes",
        version.changes.as_ref().map_or(0, |c| c.changes.len()).to_string(),
      ),
      ("runs", version.runs.len().to_string()),
    ],
  );
  crate::ui::pressable::pressable(move || {
    if !selected {
      target.update(|s| {
        s.viewing = Some((*snapshot).clone());
        s.viewing_version = Some(number);
        s.comparing = false;
        s.epoch += 1;
      });
    }
  })
  .id(format!("pipeline-version-{}", number.get()))
  .width(Dimension::Pct(100.0))
  .padding(SpacingSize::Sm)
  .rounded(RadiusSize::Md)
  .background(if selected {
    crate::theme::ExtraColor::Hover.role()
  } else {
    PaletteColor::SurfacePanel
  })
  .child(
    Column::new()
      .spacing(SpacingSize::Xs)
      .child(head)
      .child(parts::caption(&meta)),
  )
  .into()
}
pub fn back(state: &State) {
  state.update(|s| {
    s.viewing = None;
    s.viewing_version = None;
    s.comparing = false;
    s.panel = Panel::Pipeline;
    s.epoch += 1;
  });
}
pub fn toolbar(ctx: &Ctx, state: &State) -> Element {
  let editing = state.get();
  let target = state.clone();
  Row::new()
    .padding(SpacingSize::Xs)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .child(parts::caption(
      &ctx.t_args(
        "pipelines.viewing",
        [(
          "version",
          editing
            .viewing_version
            .map_or_else(String::new, |v| v.get().to_string()),
        )],
      ),
    ))
    .with_children(editing.comparing.then(|| parts::caption(&ctx.t("pipelines.comparing"))))
    .child(
      ActionButton::new("pipeline-back-draft", ButtonKind::Ghost, ctx.t("pipelines.returnDraft"))
        .on_click(move || back(&target)),
    )
    .into()
}
fn details(
  ctx: &Ctx,
  state: &State,
  version: &orchester_workspace::pipelines::history::PublishedVersion,
  mut body: Column,
) -> Column {
  let editing = state.get();
  body = body
    .child(parts::section(ctx, "pipelines.changes"))
    .child(parts::caption(
      &ctx.t_args("pipelines.runs", [("count", version.runs.len().to_string())]),
    ));
  let changes = if editing.comparing {
    orchester_core::resource::changes(&version.body, &editing.opened.body)
  } else {
    version.changes.as_ref().map(|c| c.changes.clone()).unwrap_or_default()
  };
  if changes.is_empty() {
    body = body.child(parts::note(&ctx.t(if editing.comparing {
      "pipelines.noDraft"
    } else {
      "pipelines.firstVersion"
    })));
  }
  for change in &changes {
    body = body.child(parts::note(&message_text(ctx, &describe::change(change))));
  }
  let compare = state.clone();
  body = body.child(
    ActionButton::new("pipeline-compare", ButtonKind::Secondary, ctx.t("pipelines.compare"))
      .on_click(move || compare.update(|s| s.comparing = !s.comparing)),
  );
  body = body.child(crate::ui::notice::handling_notice(
    "history-pinned",
    crate::ui::icon::Icon::Info,
    &ctx.t("pipelines.pinnedHint"),
  ));
  body
}
