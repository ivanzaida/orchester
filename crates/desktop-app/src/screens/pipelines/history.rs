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
use orchester_workspace::{background, messages, pipelines::history::VersionHistory};

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
              .child(
                Text::new(
                  &ctx.t_args(
                    "pipelines.draftVersion",
                    [(
                      "version",
                      orchester_workspace::pipelines::write::next_version(&editing.opened)
                        .map_or_else(String::new, |v| v.get().to_string()),
                    )],
                  ),
                )
                .variant(crate::theme::ExtraTypography::Strong.role()),
              )
              .child(parts::caption(&editing.opened.draft_saved_at.map_or_else(
                || ctx.t("pipelines.noDraft"),
                |at| {
                  ctx.t_args(
                    "pipelines.draftSavedAt",
                    [("age", super::library::age(ctx, at).to_string())],
                  )
                },
              ))),
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
  parts::panel(
    ctx,
    &ctx.t("pipelines.historyTitle"),
    &ctx.t("pipelines.history"),
    body.into(),
    None,
    Some(state.clone()),
  )
}
fn restore_button(ctx: &Ctx, state: &State) -> ActionButton {
  let editing = state.get();
  let restore = state.clone();
  ActionButton::new(
    "pipeline-restore",
    ButtonKind::Secondary,
    ctx.t_args(
      "pipelines.restoreVersion",
      [(
        "version",
        editing
          .viewing_version
          .map_or_else(String::new, |v| v.get().to_string()),
      )],
    ),
  )
  .icon(crate::ui::icon::Icon::RotateCcw)
  .on_click(move || {
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
  })
}
fn comparison_tag(ctx: &Ctx, editing: &super::state::Editing) -> Row {
  let count = editing.viewing.as_ref().map_or(0, |before| {
    orchester_core::resource::changes(before, &editing.opened.body)
      .iter()
      .filter_map(|change| match change {
        orchester_core::resource::PipelineChange::Step { key, .. } => Some(key),
        orchester_core::resource::PipelineChange::Description => None,
      })
      .collect::<std::collections::BTreeSet<_>>()
      .len()
  });
  Row::new()
    .padding(lurq::node::padding::Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Xs)
    .rounded(crate::theme::ExtraRadius::Full.role())
    .background(PaletteColor::InfoMuted)
    .child(crate::ui::icon::icon(
      crate::ui::icon::Icon::GitCompare,
      crate::theme::IconSize::Xs,
      crate::theme::ExtraColor::Selection.role(),
    ))
    .child(
      parts::caption(&ctx.t_args("pipelines.comparisonCount", [("count", count.to_string())]))
        .color(crate::theme::ExtraColor::Selection.role()),
    )
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
    head = head.child(crate::ui::chip::TagChip::new(
      "pipeline-latest-badge",
      ctx.t("pipelines.latest"),
    ));
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
      .width(Dimension::Pct(100.0))
      .child(
        head
          .child(lurq::components::Spacer::new().flex(1.0))
          .child(crate::ui::icon::icon(
            if selected {
              crate::ui::icon::Icon::ChevronDown
            } else {
              crate::ui::icon::Icon::ChevronRight
            },
            crate::theme::IconSize::Sm,
            PaletteColor::TextMuted,
          )),
      )
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
    .with_children(editing.comparing.then(|| comparison_tag(ctx, &editing)))
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
    .child(crate::ui::inspector::section_title(
      "pipeline-version-changes",
      &ctx.t_args("pipelines.changesIn", [("version", version.number.get().to_string())]),
      None,
    ))
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
    body = body.child(parts::change_row(ctx, change));
  }
  let compare = state.clone();
  body = body.child(
    Row::new()
      .spacing(SpacingSize::Sm)
      .child(
        ActionButton::new(
          "pipeline-compare",
          ButtonKind::Secondary,
          ctx.t(if editing.comparing {
            "pipelines.hideComparison"
          } else {
            "pipelines.compare"
          }),
        )
        .icon(crate::ui::icon::Icon::GitCompare)
        .on_click(move || compare.update(|s| s.comparing = !s.comparing)),
      )
      .child(restore_button(ctx, state)),
  );
  body = body.child(crate::ui::notice::handling_notice(
    "history-pinned",
    crate::ui::icon::Icon::Info,
    &ctx.t("pipelines.pinnedHint"),
  ));
  body
}
