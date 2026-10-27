//! Beside a skill's page (`Side` `jGRYJ` in 03b): who it is loaded for, its versions, where it was imported from and
//! the active runs that hold it, each a group of the hub's rows.
use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::InstructionScope;
use orchester_workspace::skills::data::SkillEntry;

use super::{PageProps, words};
use crate::{
  screens::hub::parts::{self, Entry},
  theme::HubSpacing,
  ui::{icon::Icon, message::message_text},
};

/// The side of `entry`'s page; a new skill has none yet.
pub fn panel(ctx: &Ctx, page: &PageProps, entry: Option<&SkillEntry>) -> Element {
  let Some(entry) = entry else {
    return Element::new();
  };
  let mut side = Column::new()
    .id("skill-inspector")
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::SideSections.role())
    .child(group(ctx, ("skill-loaded", "skills.loadedFor"), vec![loaded_for(ctx, page, entry)]))
    .child(group(ctx, ("skill-versions", "skills.versions"), versions(ctx, entry)));
  if let Some(origin) = origin(ctx, entry) {
    side = side.child(origin);
  }
  side
    .child(group(ctx, ("skill-active", "skills.activeRuns"), active(ctx, page, entry)))
    .into()
}

/// A side group: its head, then `rows` in a box.
fn group(ctx: &Ctx, (id, title): (&str, &str), rows: Vec<Element>) -> Column {
  parts::group(
    id,
    parts::head(id, &ctx.t(title), None, None),
    parts::boxed(&format!("{id}-rows"), rows),
  )
}

/// Who gets it (`Builder` `vEZ0o`): the project, the role, the pipeline or the step its scope names, and what that
/// means.
fn loaded_for(ctx: &Ctx, p: &PageProps, e: &SkillEntry) -> Element {
  let Some(option) = p.data.scopes.iter().find(|o| &o.scope == e.body.scope()) else {
    return Entry::new("skill-loaded-scope", words::scope(ctx, &p.data, e.body.scope())).into();
  };
  let removed = || ctx.t("skills.scopes.removed");
  let (label, kind, detail): (Arc<str>, &str, Arc<str>) = match &option.scope {
    InstructionScope::Project => (
      p.project.open.project.name().as_str().into(),
      "skills.project",
      ctx.t("skills.scopeMenu.everyAgent"),
    ),
    InstructionScope::AgentRole { role } => (
      role.as_str().into(),
      "skills.role",
      ctx.t("skills.scopeMenu.profileAgents"),
    ),
    InstructionScope::Pipeline { .. } => (
      option.pipeline.clone().unwrap_or_else(removed),
      "skills.pipeline",
      ctx.t("skills.scopeMenu.agentSteps"),
    ),
    InstructionScope::Step { .. } => (
      option.step.clone().unwrap_or_else(removed),
      "skills.step",
      option.pipeline.clone().unwrap_or_else(removed),
    ),
  };
  let hint = ctx.t_args(
    "skills.loadedHint",
    [("kind", ctx.t(kind).to_string()), ("detail", detail.to_string())],
  );
  Entry::new("skill-loaded-for", label).hint(hint).into()
}

/// *Runs use* and *Draft* (`gmdI4`, `b1P0cc`).
fn versions(ctx: &Ctx, e: &SkillEntry) -> Vec<Element> {
  let published = e.state.published().map_or_else(
    || ctx.t("skills.none"),
    |v| ctx.t_args("skills.version.published", [("version", v.get().to_string())]),
  );
  let draft = if e.state.has_draft() {
    words::version(ctx, e.state)
  } else {
    ctx.t("skills.none")
  };
  vec![
    Entry::new("skill-runs-use", ctx.t("skills.runsUse")).hint(published).into(),
    Entry::new("skill-draft", ctx.t("skills.draft")).hint(draft).into(),
  ]
}

/// Where it was imported from and when, and that the repository's copy and this one are apart (`v3bfi`).
fn origin(ctx: &Ctx, e: &SkillEntry) -> Option<Column> {
  let origin = e.origin.as_ref()?;
  let source = origin.source.as_str();
  let when = message_text(
    ctx,
    &orchester_workspace::display::elapsed(orchester_core::time::Timestamp::now(), origin.imported_at),
  );
  let rows = vec![
    Entry::new("skill-origin-source", ctx.t("skills.importedFrom"))
      .hint(source)
      .into(),
    Entry::new("skill-origin-date", ctx.t("skills.importedOn"))
      .hint(when)
      .into(),
  ];
  Some(group(ctx, ("skill-origin", "skills.origin"), rows).child(parts::note(
    "skill-origin-note",
    Icon::GitBranch,
    &ctx.t_args("skills.originNote", [("source", source)]),
  )))
}

/// A row per active run that holds the skill (`KON-16 · 03 Implement on v3`), or that none does.
fn active(ctx: &Ctx, p: &PageProps, e: &SkillEntry) -> Vec<Element> {
  let rows = p
    .data
    .active
    .iter()
    .filter(|u| u.skill == e.id)
    .enumerate()
    .map(|(i, u)| {
      let label = ctx.t_args(
        "skills.activeUse",
        [
          ("task", u.task.to_string()),
          (
            "step",
            u.step
              .as_ref()
              .map_or_else(|| ctx.t("skills.none").to_string(), ToString::to_string),
          ),
          ("version", u.version.get().to_string()),
        ],
      );
      Entry::new(format!("skill-active-runs-{i}"), label).into()
    })
    .collect::<Vec<Element>>();
  if rows.is_empty() {
    vec![Entry::new("skill-active-runs", ctx.t("skills.noActiveRuns")).into()]
  } else {
    rows
  }
}
