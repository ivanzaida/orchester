//! Ordered sources in the preview and full viewer, using BYHVZ and S9rsK.
use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  run::{Composition, PromptSource},
};

use super::{panels::Panels, words};
use crate::{
  theme::instructions,
  ui::prompt_source::{PromptSourceRow, excluded_source},
};

pub fn edited_index(composition: &Composition, edited: Option<ResourceId>) -> Option<usize> {
  composition.sections().iter().position(
    |section| matches!(section.source, PromptSource::Instruction { resource, .. } if Some(resource) == edited),
  )
}
pub fn list(ctx: &Ctx, state: &Panels, composition: &Composition, edited: Option<ResourceId>, full: bool) -> Element {
  let selected = state
    .source
    .get()
    .0
    .or_else(|| edited_index(composition, edited))
    .unwrap_or(0);
  let mut list = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(crate::theme::ExtraSpacing::Hair.role())
    .child(
      Text::new(&ctx.t_args(
        if full {
          "instructions.sourcesIncluded"
        } else {
          "instructions.sources"
        },
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
