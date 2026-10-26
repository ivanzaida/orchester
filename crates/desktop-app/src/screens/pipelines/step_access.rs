//! The step column's Gate and access group (`Gate and access` `EQ6y1` in `IsU5u`): *Wait for your approval*, and
//! for an agent's step its repository access (*Read-only* / *Can write*, which only a profile that can write
//! offers), *Merge the base branch when it moves* for a step that can write, and its Sources and Permissions, each
//! on a page of its own. A tool step keeps only the approval.
use lurq::{
  app::ctx::Ctx,
  components::Row,
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  permission::StepCommands,
  resource::{RepositoryAccess, Step, StepKind, takes_base_moves, takes_base_moves_by_default},
};
use orchester_workspace::pipelines::narrowing;

use super::{
  state::{StepPage, View},
  step_fields::Props,
};
use crate::{
  theme::IconSize,
  ui::{
    icon::{Icon, icon},
    pressable::{Focus, Press},
    row_group::{GroupRow, RowGroup},
    tabs::{Segment, SegmentFace, segmented},
    toggle::{Toggle, ToggleKind},
    tooltip::Tooltip,
  },
};

pub fn group(ctx: &mut Ctx, props: &Props) -> Element {
  let step = &props.step;
  let mut group = RowGroup::new("step-access-group", ctx.t("pipelines.gateAccess")).row(gate(ctx, props));
  let StepKind::Agent {
    profile, repository, ..
  } = &step.kind
  else {
    return group.into();
  };
  group = group.row(access(ctx, props, (*profile, *repository)));
  if *repository == RepositoryAccess::Write {
    group = group.row(base_moves(ctx, props));
  }
  group
    .row(page_row(ctx, props, StepPage::Sources, sources_hint(ctx, props)))
    .row(page_row(ctx, props, StepPage::Permissions, permissions_hint(ctx, props)))
    .into()
}

fn switch(
  ctx: &Ctx,
  props: &Props,
  (id, label): (&str, &str),
  on: bool,
  change: impl Fn(bool) + Send + Sync + 'static,
) -> GroupRow {
  let row = GroupRow::new(format!("{id}-row"), ctx.t(label));
  if props.viewing {
    return row.value(&ctx.t(if on { "pipelines.on" } else { "pipelines.off" }));
  }
  let toggle = Toggle::new(id.to_owned(), ToggleKind::Switch, on, ctx.t(label)).hide_label();
  row.end(if props.locked { toggle } else { toggle.on_change(change) })
}

fn gate(ctx: &Ctx, props: &Props) -> GroupRow {
  let (edit, key) = (props.state.clone(), props.step.key.clone());
  switch(ctx, props, ("step-gate", "pipelines.gate"), props.step.approval_gate, move |on| {
    edit.update(|s| s.change(&key, |step| step.approval_gate = on));
  })
}

/// *Read-only* / *Can write*; *Can write* answers only when the step's profile can write or the step does already.
fn access(
  ctx: &Ctx,
  props: &Props,
  (profile, repository): (Option<orchester_core::id::ResourceId>, RepositoryAccess),
) -> GroupRow {
  let row = GroupRow::new("step-access", ctx.t("pipelines.access"));
  let write = repository == RepositoryAccess::Write;
  if props.viewing {
    return row.value(&ctx.t(if write { "pipelines.canWrite" } else { "pipelines.readOnly" }));
  }
  let editing = props.state.get();
  let choice = profile.and_then(|id| editing.opened.assignables.profile(id));
  let writable = !props.locked && (choice.is_some_and(narrowing::can_write) || write);
  let segments = [(false, "pipelines.readOnly"), (true, "pipelines.canWrite")].map(|(wanted, label)| {
    let (edit, key) = (props.state.clone(), props.step.key.clone());
    Segment::new(
      if wanted { "step-write" } else { "step-read" },
      SegmentFace::Text(ctx.t(label)),
      write == wanted,
      move || {
        if writable {
          edit.update(|s| set_access(s, &key, wanted));
        }
      },
    )
  });
  row.end(segmented("step-access-choice", segments, Focus::Keyboard))
}

fn set_access(editing: &mut super::state::Editing, key: &orchester_core::resource::StepKey, write: bool) {
  editing.change(key, |step| {
    if let StepKind::Agent { repository, .. } = &mut step.kind {
      *repository = if write {
        RepositoryAccess::Write
      } else {
        RepositoryAccess::ReadOnly
      };
    }
  });
}

/// *Merge the base branch when it moves* (ORC-65) for a writing step, as the pipeline's rule decides it: its own
/// choice, else the default. A choice the default already makes is not kept, so the step goes on following the rule;
/// the (i) says the rule.
fn base_moves(ctx: &mut Ctx, props: &Props) -> GroupRow {
  let editing = props.state.get();
  let shown = editing.shown();
  let by_default = takes_base_moves_by_default(shown.steps(), &props.step);
  let (edit, key) = (props.state.clone(), props.step.key.clone());
  let row = switch(
    ctx,
    props,
    ("step-base-moves", "pipelines.baseMoves"),
    takes_base_moves(shown.steps(), &props.step.key),
    move |on| {
      edit.update(|s| {
        s.change(&key, |step| {
          if let StepKind::Agent { permissions, .. } = &mut step.kind {
            permissions.takes_base_moves = (on != by_default).then_some(on);
          }
        });
      });
    },
  );
  let info = Row::new()
    .id("step-base-moves-note")
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Sm, lurq::app::theme::PaletteColor::TextMuted));
  row.tag(Tooltip::new("step-base-moves-hint", ctx.t("pipelines.baseMovesHint")).wrap(ctx, info))
}

/// A row that opens one of the step's pages.
fn page_row(ctx: &Ctx, props: &Props, page: StepPage, hint: String) -> GroupRow {
  let (id, label) = match page {
    StepPage::Sources => ("step-open-sources", "pipelines.sources"),
    StepPage::Permissions => ("step-open-permissions", "pipelines.permissions"),
    StepPage::Instructions => ("step-instructions-row", "pipelines.instructions"),
  };
  let (views, key) = (props.state.get().views, props.step.key.clone());
  GroupRow::new(id, ctx.t(label))
    .hint(hint)
    .opens_page(Press::run(move || views.choose(View::StepPage(key.clone(), page))))
}

/// The step's own sources and those it inherits from the pipeline (the project's are counted on their page).
fn sources_hint(ctx: &Ctx, props: &Props) -> String {
  let shown = props.state.get().shown();
  ctx
    .t_args(
      "pipelines.sourcesSection.counts",
      [
        ("own", shown.sources().step(&props.step.key).len().to_string()),
        ("inherited", shown.sources().pipeline().len().to_string()),
      ],
    )
    .to_string()
}

/// Whether the step narrows its profile's permissions.
pub(super) fn permissions_hint(ctx: &Ctx, props: &Props) -> String {
  let editing = props.state.get();
  let StepKind::Agent {
    profile, permissions, ..
  } = &props.step.kind
  else {
    return String::new();
  };
  let Some(choice) = profile.and_then(|id| editing.opened.assignables.profile(id)) else {
    return ctx.t("pipelines.narrowHint").to_string();
  };
  let narrowed = *permissions != narrowing::inherit_profile(permissions);
  ctx
    .t_args(
      if narrowed {
        "pipelines.narrowerThan"
      } else {
        "pipelines.permissionsFrom"
      },
      [("profile", choice.name.as_ref())],
    )
    .to_string()
}

/// The commands a step allows, as its command picker says them.
pub(super) fn command_summary(ctx: &Ctx, step: &Step) -> String {
  if let StepKind::Agent { permissions, .. } = &step.kind
    && let StepCommands::Only(only) = &permissions.commands
  {
    return ctx
      .t_args(
        "pipelines.narrowedCommands",
        [("count", only.entries().len().to_string())],
      )
      .to_string();
  }
  ctx.t("pipelines.inherit").to_string()
}
