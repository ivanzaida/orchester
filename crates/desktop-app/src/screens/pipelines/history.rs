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
use orchester_workspace::{background, messages, pipelines::history::VersionHistory};

use super::{
  parts,
  state::{Editing, State},
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
  pub writing: bool,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.writing == other.writing
  }
}
impl DevtoolsInspectable for Props {}
pub struct History {
  retry: lurq::core::Signal<u64>,
}
impl Component for History {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    ctx.props::<Props>().state.update(|s| {
      s.comparing = false;
      s.restore_requested = false;
    });
    Self { retry: ctx.signal(0) }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    panel(ctx, &props.project, &props.state, props.writing, &self.retry)
  }
}
fn panel(
  ctx: &mut Ctx,
  project: &ShownProject,
  state: &State,
  writing: bool,
  retry: &lurq::core::Signal<u64>,
) -> Element {
  let editing = state.get();
  let loaded = ctx
    .future(
      (
        project.open.repo_root.clone(),
        editing.opened.id,
        editing.epoch,
        retry.get(),
      ),
      |(root, id, ..)| async move {
        background::run(move || VersionHistory::load_in(&root, id).map_err(|e| messages::store_error(&e))).await?
      },
    )
    .state()
    .get();
  let mut body = parts::panel_body();
  match loaded.data.filter(|_| loaded.error.is_none()) {
    Some(history) => {
      if history.versions().is_empty() {
        body = body.child(parts::note(&ctx.t("pipelines.historyEmpty")));
      }
      if editing.opened.draft_saved_at.is_some() || editing.dirty() || !editing.errors.is_empty() {
        body = body.child(draft_row(ctx, state));
      }
      for version in history.versions() {
        body = body.child(version_row(
          ctx,
          state,
          version,
          history.latest().is_some_and(|v| v.number == version.number),
        ));
      }
      if let Some(version) = editing.viewing_version.and_then(|n| history.version(n)) {
        body = details(ctx, state, version, body, writing);
      }
    }
    None => {
      if loaded.error.is_none() {
        body = body.child(loading("pipeline-history-loading", &ctx.t("pipelines.reading")));
      }
    }
  }
  if let Some(error) = loaded.error {
    body = body.child(crate::ui::field::field_error(
      "pipeline-history-error",
      &message_text(ctx, &error),
    ));
    let retry = retry.clone();
    body = body.child(
      ActionButton::new(
        "pipeline-history-retry",
        ButtonKind::Secondary,
        ctx.t("pipelines.reload"),
      )
      .on_click(move || retry.update(|n| *n += 1)),
    );
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
fn restore_button(ctx: &Ctx, state: &State, writing: bool) -> ActionButton {
  let editing = state.get();
  let restore = state.clone();
  let button = ActionButton::new(
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
  .bounded_label()
  .icon(crate::ui::icon::Icon::RotateCcw)
  .on_click(move || {
    if writing {
      return;
    }
    let editing = restore.get_untracked();
    if editing.opened.draft_saved_at.is_some() || editing.dirty() || !editing.errors.is_empty() {
      restore.update(|s| s.restore_requested = true);
    } else {
      restore.update(Editing::restore);
      editing.panels.to_draft();
    }
  });
  if writing { button.disabled() } else { button }
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
      parts::caption(&ctx.t_args(
        if count == 1 {
          "pipelines.comparisonCountOne"
        } else {
          "pipelines.comparisonCount"
        },
        [
          ("count", count.to_string()),
          ("target", comparison_title(ctx, editing).to_string()),
        ],
      ))
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
    if version.changes.is_none() {
      "pipelines.historyFirstMeta"
    } else {
      "pipelines.historyMeta"
    },
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
      // Shown at once; the route, which says what is viewed, follows.
      target.update(|s| {
        s.viewing = Some((*snapshot).clone());
        s.viewing_version = Some(number);
        s.comparing = false;
        s.restore_requested = false;
        s.epoch += 1;
      });
      target.get_untracked().panels.view(number);
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
    s.restore_requested = false;
    s.epoch += 1;
  });
  state.get_untracked().panels.to_draft();
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
      ActionButton::new(
        "pipeline-back-draft",
        ButtonKind::Secondary,
        ctx.t("pipelines.returnDraft"),
      )
      .icon(crate::ui::icon::Icon::ChevronLeft)
      .on_click(move || back(&target)),
    )
    .into()
}
fn details(
  ctx: &Ctx,
  state: &State,
  version: &orchester_workspace::pipelines::history::PublishedVersion,
  mut body: Column,
  writing: bool,
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
  let changes = version.changes.as_ref().map(|c| c.changes.clone()).unwrap_or_default();
  if version.changes.is_none() {
    body = body.child(parts::note(&ctx.t("pipelines.firstVersion")));
  }
  for change in &changes {
    body = body.child(parts::change_row(ctx, change));
  }
  if editing.comparing {
    body = body.child(crate::ui::inspector::section_title(
      "pipeline-comparison-changes",
      &comparison_title(ctx, &editing),
      None,
    ));
    for change in orchester_core::resource::changes(&version.body, &editing.opened.body) {
      body = body.child(parts::change_row(ctx, &change));
    }
  }
  if editing.restore_requested {
    return body.child(super::restore::question(ctx, state, writing));
  }
  body = body.child(
    Row::new()
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(SpacingSize::Sm)
      .child(compare_button(ctx, state))
      .child(restore_button(ctx, state, writing)),
  );
  if editing.dirty() || !editing.errors.is_empty() {
    body = body.child(parts::note(&ctx.t("pipelines.compareUnavailable")));
  } else if no_comparison_draft(&editing) {
    body = body.child(crate::ui::inspector::note(
      "pipeline-history-no-draft",
      crate::ui::icon::Icon::Lock,
      &ctx.t_args(
        "pipelines.historyNoDraft",
        [(
          "version",
          orchester_workspace::pipelines::write::next_version(&editing.opened)
            .map_or_else(String::new, |v| v.get().to_string()),
        )],
      ),
    ));
  }
  body = body.child(crate::ui::notice::handling_notice(
    "history-pinned",
    crate::ui::icon::Icon::Info,
    &ctx.t("pipelines.pinnedHint"),
  ));
  body
}
fn comparison_title(ctx: &Ctx, editing: &super::state::Editing) -> std::sync::Arc<str> {
  let (key, version) = if editing.opened.draft_saved_at.is_some() {
    (
      "pipelines.comparedSavedDraft",
      orchester_workspace::pipelines::write::next_version(&editing.opened),
    )
  } else {
    (
      "pipelines.comparedPublished",
      editing.opened.latest.as_ref().map(|v| v.number),
    )
  };
  ctx.t_args(
    key,
    [("version", version.map_or_else(String::new, |v| v.get().to_string()))],
  )
}
fn draft_row(ctx: &Ctx, state: &State) -> Element {
  let editing = state.get();
  let target = state.clone();
  crate::ui::pressable::pressable(move || back(&target))
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
        .child(parts::caption(&super::summary::draft(ctx, &editing)))
        .with_children(editing.opened.draft_saved_at.map(|at| {
          parts::caption(&ctx.t_args(
            "pipelines.draftSavedAt",
            [("age", super::library::age(ctx, at).to_string())],
          ))
        })),
    )
    .into()
}
fn no_comparison_draft(editing: &super::state::Editing) -> bool {
  editing.opened.draft_saved_at.is_none() && editing.viewing_version == editing.opened.latest.as_ref().map(|v| v.number)
}

fn compare_button(ctx: &Ctx, state: &State) -> ActionButton {
  let editing = state.get();
  let compare = state.clone();
  let button = ActionButton::new(
    "pipeline-compare",
    ButtonKind::Secondary,
    ctx.t(if editing.comparing {
      "pipelines.hideComparison"
    } else if editing.opened.draft_saved_at.is_some() || no_comparison_draft(&editing) {
      "pipelines.compare"
    } else {
      "pipelines.comparePublished"
    }),
  )
  .icon(crate::ui::icon::Icon::GitCompare)
  .on_click(move || compare.update(|s| s.comparing = !s.comparing));
  if editing.dirty() || !editing.errors.is_empty() || no_comparison_draft(&editing) {
    button.disabled()
  } else {
    button
  }
}
