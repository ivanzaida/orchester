//! Ordered sources: in the preview a group of rows (`4 sources · assembly order` `hs1LQ` in 02b: *1 · Builder* over
//! *Agent role · v4*, the excluded ones after them), in the full viewer BYHVZ and S9rsK.
use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  run::{Composition, ExcludedInstruction, PromptSource},
};

use super::{panels::Panels, words};
use crate::{
  screens::hub::parts::{self, Entry},
  theme::instructions,
  ui::{
    pressable::Press,
    prompt_source::{PromptSourceRow, excluded_source},
  },
};

pub fn edited_index(composition: &Composition, edited: Option<ResourceId>) -> Option<usize> {
  composition.sections().iter().position(
    |section| matches!(section.source, PromptSource::Instruction { resource, .. } if Some(resource) == edited),
  )
}
/// The preview's sources as a group of rows.
fn group(ctx: &Ctx, state: &Panels, composition: &Composition, edited: Option<ResourceId>, selected: usize) -> Element {
  let mut rows = Vec::new();
  for (index, section) in composition.sections().iter().enumerate() {
    let this_rule = matches!(section.source, PromptSource::Instruction { resource, .. } if Some(resource) == edited);
    let kind = words::part(ctx, section.source.part());
    let version = version(ctx, &section.source);
    let hint = if version.is_empty() {
      kind
    } else {
      ctx.t_args(
        "instructions.sourceHint",
        [("kind", kind.to_string()), ("version", version.to_string())],
      )
    };
    let hint = if this_rule {
      ctx.t_args("instructions.thisRule", [("kind", hint)])
    } else {
      hint
    };
    let title = ctx.t_args(
      "instructions.sourceTitle",
      [("number", (index + 1).to_string()), ("name", source_name(ctx, section))],
    );
    let choose = state.source.clone();
    rows.push(
      Entry::new(format!("instruction-source-{index}"), title)
        .hint(hint)
        .chosen(selected == index)
        .in_place()
        .press(Press::run(move || {
          choose.set(orchester_workspace::instructions::form::Picked(Some(index)));
        }))
        .into(),
    );
  }
  for excluded in composition.excluded() {
    let (title, reason) = excluded_words(ctx, excluded);
    rows.push(
      Entry::new(format!("instruction-excluded-{}", excluded.resource), title)
        .hint(reason)
        .into(),
    );
  }
  let id = "instruction-sources";
  let head = ctx.t_args(
    "instructions.sources",
    [("count", composition.sections().len().to_string())],
  );
  parts::group(
    id,
    parts::head(id, &head, None, None),
    parts::boxed(&format!("{id}-rows"), rows),
  )
  .into()
}
/// An excluded rule's title and why it is left out.
fn excluded_words(ctx: &Ctx, excluded: &ExcludedInstruction) -> (Arc<str>, Arc<str>) {
  let title = ctx.t_args(
    "instructions.excludedVersion",
    [
      ("name", excluded.name.clone()),
      ("version", words::preview_version(ctx, excluded.version).to_string()),
    ],
  );
  let reason = if excluded.reason == orchester_core::resource::Exclusion::OtherRole {
    match excluded.body.targets() {
      orchester_core::resource::AgentTargets::Roles(roles) => ctx.t_args(
        "instructions.excludedRoles",
        [(
          "roles",
          roles.iter().map(|role| role.as_str()).collect::<Vec<_>>().join(", "),
        )],
      ),
      _ => words::excluded(ctx, excluded.reason),
    }
  } else {
    words::excluded(ctx, excluded.reason)
  };
  (title, reason)
}
pub fn list(ctx: &Ctx, state: &Panels, composition: &Composition, edited: Option<ResourceId>, full: bool) -> Element {
  let selected = state
    .source
    .get()
    .0
    .or_else(|| edited_index(composition, edited))
    .unwrap_or(0);
  if !full {
    return group(ctx, state, composition, edited, selected);
  }
  let mut list = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(crate::theme::ExtraSpacing::Hair.role())
    .child(
      Text::new(&ctx.t_args(
        "instructions.sourcesIncluded",
        [("count", composition.sections().len().to_string())],
      ))
      .variant(instructions::overline())
      .padding_bottom(SpacingSize::Sm),
    );
  for (index, section) in composition.sections().iter().enumerate() {
    let this_rule = matches!(section.source, PromptSource::Instruction { resource, .. } if Some(resource) == edited);
    let kind = words::part(ctx, section.source.part());
    let kind = if this_rule {
      ctx.t_args("instructions.thisRule", [("kind", kind)])
    } else {
      kind
    };
    let choose = state.source.clone();
    let row = PromptSourceRow {
      number: index + 1,
      kind: kind.to_string(),
      name: source_name(ctx, section),
      version: version(ctx, &section.source).to_string(),
      selected: selected == index,
    };
    list = list.child(row.render(&format!("instruction-source-{index}"), move || {
      choose.set(orchester_workspace::instructions::form::Picked(Some(index)))
    }));
  }
  for excluded in composition.excluded() {
    let (title, reason) = excluded_words(ctx, excluded);
    list = list.child(excluded_source(
      &format!("instruction-excluded-{}", excluded.resource),
      &title,
      &reason,
    ));
  }
  list.into()
}
pub fn version(ctx: &Ctx, source: &PromptSource) -> std::sync::Arc<str> {
  match source {
    PromptSource::AgentProfile { version, .. } | PromptSource::Step { version, .. } => {
      ctx.t_args("instructions.version", [("version", version.get().to_string())])
    }
    PromptSource::Instruction { version, .. } => words::preview_version(ctx, *version),
    PromptSource::Task { .. } => "".into(),
  }
}

fn source_name(ctx: &Ctx, section: &orchester_core::run::PromptSection) -> String {
  match &section.source {
    PromptSource::Step { pipeline, version, .. } => {
      let props = ctx.props::<super::panels::Props>();
      props.data.pipeline(*pipeline).map_or_else(
        || section.name.clone(),
        |pipeline| {
          ctx
            .t_args(
              "instructions.stepSource",
              [
                ("step", section.name.clone()),
                ("pipeline", pipeline.name.to_string()),
                ("version", version.get().to_string()),
              ],
            )
            .to_string()
        },
      )
    }
    PromptSource::Task { .. } => ctx
      .t_args("instructions.taskSource", [("task", section.name.clone())])
      .to_string(),
    _ => section.name.clone(),
  }
}
