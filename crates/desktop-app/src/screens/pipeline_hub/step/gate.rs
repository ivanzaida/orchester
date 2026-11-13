//! *Gate and access* of an open step (`Gate and access` `vPjMJ` in P2b `EGEos`, P2d `D4MDQO`): *Wait for your
//! approval*; for an agent step its *Repository access*, *Merge the base branch when it moves* when it can write, its
//! *Sources* and its permissions ([`super::permissions`]). A tool step keeps only the approval. Every row changes the
//! edit the panel holds; while a save is under way the controls keep their place in the kit's disabled look (P2f
//! `Tv9hK`).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  resource::{RepositoryAccess, Step, StepKind, takes_base_moves, takes_base_moves_by_default},
};
use orchester_workspace::pipelines::{fields::AccessChoice, narrowing, sources::Listing};

use super::{Open, permissions};
use crate::{
  screens::{
    pipeline_hub::{
      pick::{self, Pick},
      sources::{self, Props as SourcesProps},
    },
    team_hub::rows::{self, Sub},
  },
  theme::{ExtraSpacing, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    toggle::{Toggle, ToggleKind},
    tooltip::Tooltip,
  },
};

/// The group, its rows in the design's order.
pub(super) fn group(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> Element {
  let mut rows = vec![approval_row(ctx, open, step)];
  if let StepKind::Agent {
    profile, repository, ..
  } = &step.kind
  {
    rows.push(access_row(ctx, open, step, (*profile, *repository)));
    if *repository == RepositoryAccess::Write {
      rows.push(base_moves_row(ctx, open, step));
    }
    rows.push(sources_row(ctx, open, step));
    rows.extend(permissions::rows(ctx, open, step));
  }
  rows::section(
    "pipeline-hub-step-gate",
    &ctx.t("pipeline_hub.step.gateGroup"),
    Vec::new(),
    rows,
  )
  .ref_element(open.gate.clone())
  .into()
}

/// What a switch does when it is pressed: it hears the value the press asks for.
pub(super) type OnSwitch = Box<dyn Fn(bool) + Send + Sync>;

/// A switch at the end of a row, named `label` for tools; without `change` it is fixed and faded.
pub(super) fn switch(id: &str, label: Arc<str>, on: bool, change: Option<OnSwitch>) -> Row {
  let toggle = Toggle::new(id.to_owned(), ToggleKind::Switch, on, label).hide_label();
  let toggle = match change {
    Some(change) => toggle.on_change(change),
    None => toggle,
  };
  Row::new().flex(1.0).justify(Justify::End).child(toggle)
}

fn approval_row(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> Row {
  let label = ctx.t("pipeline_hub.step.approval");
  let (edit, key) = (open.edit.clone(), step.key.clone());
  let change: Option<OnSwitch> = (!open.locked).then(|| {
    Box::new(move |on: bool| {
      edit.change(|current| {
        current.update(&key, |step| step.approval_gate = on);
      });
    }) as OnSwitch
  });
  let toggle = switch("pipeline-hub-step-approval", label.clone(), step.approval_gate, change);
  rows::row("pipeline-hub-step-approval-row", &label, None, toggle)
}

/// *Repository access*: *Read-only* or *Can write*, which only a profile that can write offers.
fn access_row(
  ctx: &mut Ctx,
  open: &Open<'_>,
  step: &Step,
  (profile, repository): (Option<ResourceId>, RepositoryAccess),
) -> Row {
  let label = ctx.t("pipeline_hub.step.access");
  let write = repository == RepositoryAccess::Write;
  let can_write = profile
    .and_then(|id| open.assignables.profile(id))
    .is_some_and(narrowing::can_write);
  let options = vec![
    ("read".to_owned(), ctx.t("pipeline_hub.step.readOnly").to_string()),
    ("write".to_owned(), ctx.t("pipeline_hub.step.canWrite").to_string()),
  ];
  let (edit, key) = (open.edit.clone(), step.key.clone());
  let on_pick: Pick = Arc::new(move |picked| {
    let choice = AccessChoice(if picked == "write" {
      RepositoryAccess::Write
    } else {
      RepositoryAccess::ReadOnly
    });
    edit.change(|current| {
      current.update(&key, |step| choice.apply(step));
    });
  });
  let select = pick::pick(
    ctx,
    pick::Props {
      id: format!("pipeline-hub-step-access-{}-{}", step.key, open.epoch),
      name: label.to_string(),
      value: if write { "write" } else { "read" }.to_owned(),
      options,
      placeholder: None,
      mono: false,
      locked: open.locked || !(can_write || write),
      choosing: false,
      on_pick,
    },
  );
  rows::row("pipeline-hub-step-access-row", &label, None, select)
}

/// *Merge the base branch when it moves* (ORC-65) for a step that can write, as the pipeline's rule decides it: the
/// step's own choice, else the rule's. A choice the rule already makes is not kept, so the step goes on following the
/// rule; the (i) says the rule.
fn base_moves_row(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> Row {
  let id = "pipeline-hub-step-base-moves-row";
  let label = ctx.t("pipeline_hub.stepEdit.baseMoves");
  let steps = open.current.steps();
  let by_default = takes_base_moves_by_default(steps, step);
  let (edit, key) = (open.edit.clone(), step.key.clone());
  let change: Option<OnSwitch> = (!open.locked).then(|| {
    Box::new(move |on: bool| {
      edit.change(|current| {
        current.update(&key, |step| {
          if let StepKind::Agent { permissions, .. } = &mut step.kind {
            permissions.takes_base_moves = (on != by_default).then_some(on);
          }
        });
      });
    }) as OnSwitch
  });
  let toggle = switch(
    "pipeline-hub-step-base-moves",
    label.clone(),
    takes_base_moves(steps, &step.key),
    change,
  );
  let info = Row::new()
    .id("pipeline-hub-step-base-moves-info")
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted));
  let key_line = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&label)
        .id(format!("{id}-label"))
        .variant(HubText::Status.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Tooltip::new(
        "pipeline-hub-step-base-moves-hint",
        ctx.t("pipeline_hub.stepEdit.baseMovesHint"),
      )
      .wrap(ctx, info),
    );
  rows::keyed(id, key_line, None, toggle)
}

/// *Sources*: what the step reads besides the pipeline's, as pills and *Add*, with how many are its own.
fn sources_row(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> Row {
  let sources = open.current.sources();
  let counts = ctx.t_args(
    "pipeline_hub.step.sourcesCounts",
    [
      ("own", sources.step(&step.key).len().to_string()),
      ("inherited", sources.pipeline().len().to_string()),
    ],
  );
  let value = sources::view(
    ctx,
    SourcesProps {
      root: open.project.open.repo_root.clone(),
      project: open.project.open.project.name().as_str().into(),
      listing: Listing::Step(step.key.clone()),
      current: open.current.clone(),
      edit: open.edit.clone(),
      locked: open.locked,
    },
  );
  rows::row(
    "pipeline-hub-step-sources-row",
    &ctx.t("pipeline_hub.step.sources"),
    Some(Sub::quiet(counts)),
    value,
  )
}
