//! The Skills tab's list (`03 Skills` `w0NiG0`): how many skills and drafts, then a group per scope — the project,
//! agent roles, pipelines and steps — with a row per skill, and *Found in this repository* with the Repository agent
//! files switch and Import ([`super::repository::found`]). With no skill yet, a quiet row says what a skill is
//! (06b). The filter a route names keeps its group first. In a shared project the line over the list says it is the
//! team's, each row its publisher, and a member imports nothing (`Shared configuration · redesign`, 01 `cHvkL`).
use lurq::{
  app::{component::Component, ctx::Ctx},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::InstructionScope;
use orchester_workspace::{sharing::team::TeamConfiguration, skills::data::SkillEntry};

use super::{PageProps, repository, state::Panel, words};
use crate::{
  routes::SkillsFilter,
  screens::hub::{
    self, bones,
    parts::{Entry, Tone, boxed, group, head, note, status, tile},
    team,
  },
  theme::{HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::Press,
    skeleton::{self, Skeleton, fading},
  },
};

pub struct Library;
impl Component for Library {
  type Props = PageProps;
  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<PageProps>().clone();
    let team = team::of(&p.project);
    let entries = &p.data.listed.entries;
    let mut pane = Column::new()
      .id("skills-library")
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Groups.role())
      .with_children(team.as_ref().map(|team| team::line(ctx, "skills-team-line", team)));
    if entries.is_empty() {
      pane = pane.child(empty(ctx, &p, team::may_change(team.as_ref())));
    } else {
      let drafts = entries.iter().filter(|e| e.state.has_draft()).count();
      let skills = ctx.t_args(
        if entries.len() == 1 {
          "skills.skillOne"
        } else {
          "skills.skillMany"
        },
        [("count", entries.len().to_string())],
      );
      pane = pane.child(
        Text::new(&ctx.t_args(
          "skills.count",
          [("skills", skills.to_string()), ("drafts", drafts.to_string())],
        ))
        .id("skills-count")
        .variant(HubText::Hint.role()),
      );
      for filter in order(p.filter) {
        let of_scope: Vec<&SkillEntry> = entries.iter().filter(|e| accepts(filter, e.body.scope())).collect();
        if of_scope.is_empty() {
          continue;
        }
        pane = pane.child(scope_group(ctx, (&p, team.as_ref()), filter, &of_scope));
      }
    }
    // A member of a shared project imports none of the repository's files into the team's skills.
    if team::may_change(team.as_ref()) {
      pane = pane.child(repository::found(ctx, &p));
    }
    let foot = match &team {
      Some(team) => team::foot(ctx, "skills-versions-note", team),
      None => note("skills-versions-note", Icon::History, &ctx.t("skills.versionsNote")),
    };
    hub::list_body("skills-list", pane.child(foot))
  }
}

/// Each scope's skills while they are read: its heading's key, and each skill's name, scope line and version state.
const READING: [(&str, skeleton::RowBones); 3] = [
  ("skills.groups.project", &[(74.0, 328.0, 60.0), (70.0, 224.0, 64.0)]),
  ("skills.groups.roles", &[(110.0, 270.0, 70.0), (30.0, 232.0, 104.0)]),
  ("skills.groups.pipelines", &[(74.0, 246.0, 96.0)]),
];

/// The list while the skills are read (`06 Agents · Skills · loading` `sQ4sB`): the group headings, the skill tile's
/// icon and the repository's fixed words real; the summary line, the counts, each skill's name, scope line and version
/// state bones, the lower groups fading. *Reading skills…* (`skills.reading`) names it.
pub(super) fn reading(ctx: &mut Ctx) -> Element {
  let mut groups: Vec<Column> = READING
    .iter()
    .enumerate()
    .map(|(at, (key, skills))| {
      let id = format!("skills-reading-{at}");
      let rows = skills.iter().map(|(name, line, state)| -> Element {
        let lead = tile(Icon::GraduationCap, false).into();
        bones::entry(ctx, Some(lead), (*name, *line), vec![bones::status(ctx, *state).into()]).into()
      });
      group(
        &id,
        bones::head(ctx, &id, &ctx.t(key), Some(14.0)),
        boxed(&format!("{id}-rows"), rows),
      )
    })
    .collect();
  let files = Entry::new("skills-reading-files", ctx.t("skills.repositoryFiles"))
    .end(skeleton::switch())
    .end(skeleton::button(30.0))
    .into();
  let import = bones::entry(ctx, None, (146.0, 0.0), Vec::new())
    .id("skills-reading-import")
    .into();
  groups.push(group(
    "skills-reading-found",
    bones::head(
      ctx,
      "skills-reading-found",
      &ctx.t("skills.repositoryTitle"),
      Some(34.0),
    ),
    boxed("skills-reading-found-rows", [files, import]),
  ));
  let pane = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Groups.role())
    .child(skeleton::line(ctx, HubText::Hint.role(), 194.0))
    .with_children(fading(groups))
    .child(note(
      "skills-versions-note",
      Icon::History,
      &ctx.t("skills.versionsNote"),
    ));
  let body = Skeleton::new("skills-loading", ctx.t("skills.reading")).show(ctx, pane);
  hub::list_body("skills-reading-list", body)
}

/// The scopes' groups in the list's order, the one the route names first.
fn order(filter: SkillsFilter) -> Vec<SkillsFilter> {
  let mut order = vec![SkillsFilter::Project, SkillsFilter::Roles, SkillsFilter::Pipelines];
  if let Some(at) = order.iter().position(|f| *f == filter) {
    let first = order.remove(at);
    order.insert(0, first);
  }
  order
}

fn accepts(filter: SkillsFilter, scope: &InstructionScope) -> bool {
  match filter {
    SkillsFilter::All => true,
    SkillsFilter::Project => matches!(scope, InstructionScope::Project),
    SkillsFilter::Roles => matches!(scope, InstructionScope::AgentRole { .. }),
    SkillsFilter::Pipelines => matches!(scope, InstructionScope::Pipeline { .. } | InstructionScope::Step { .. }),
  }
}

fn scope_group(
  ctx: &Ctx,
  (p, team): (&PageProps, Option<&TeamConfiguration>),
  filter: SkillsFilter,
  entries: &[&SkillEntry],
) -> Element {
  let key = match filter {
    SkillsFilter::Project | SkillsFilter::All => "skills.groups.project",
    SkillsFilter::Roles => "skills.groups.roles",
    SkillsFilter::Pipelines => "skills.groups.pipelines",
  };
  let id = format!("skills-group-{}", filter.name());
  let rows = entries.iter().map(|entry| row(ctx, (p, team), entry));
  group(
    &id,
    head(&id, &ctx.t(key), Some(&entries.len().to_string()), None),
    boxed(&format!("{id}-rows"), rows),
  )
  .into()
}

fn row(ctx: &Ctx, (p, team): (&PageProps, Option<&TeamConfiguration>), entry: &SkillEntry) -> Element {
  let hint = ctx.t_args(
    "skills.rowLine",
    [
      ("scope", words::scope(ctx, &p.data, entry.body.scope()).to_string()),
      ("description", entry.body.description().to_owned()),
      ("files", words::files(ctx, entry.body.files().len()).to_string()),
    ],
  );
  let row = Entry::new(format!("skill-row-{}", entry.id), entry.name.clone())
    .tile(Icon::GraduationCap)
    .hint(hint);
  let row = match team.and_then(|team| Some((team, team.standing(entry.id)?))) {
    Some((team, standing)) => row
      .one_line()
      .end(team::publisher(
        ctx,
        &format!("skill-publisher-{}", entry.id),
        team,
        standing,
      ))
      .end(team::row_state(
        ctx,
        &format!("skill-version-{}", entry.id),
        standing,
        team::draft_on(entry.state),
      )),
    None => {
      let tone = match entry.state {
        orchester_workspace::agents::data::VersionState::Published(_) => Tone::Success,
        orchester_workspace::agents::data::VersionState::Draft { published: None } => Tone::Muted,
        _ => Tone::Warning,
      };
      row.end(status(
        &format!("skill-version-{}", entry.id),
        tone,
        &words::version(ctx, entry.state),
      ))
    }
  };
  row.press(Press::Open(p.view.panel.to(Panel::Edit(entry.id)))).into()
}

/// No skill yet (06b): what a skill is, with New skill; and when the repository has nothing to import either, a row
/// saying so in the same box (`Skills · nothing yet, repository has nothing to import` in `VcE5a`). A member of a
/// shared project adds none.
fn empty(ctx: &Ctx, p: &PageProps, may_change: bool) -> Element {
  let name = p.project.open.project.name().as_str();
  let row = Entry::new("skills-empty", ctx.t_args("skills.emptyTitle", [("project", name)]))
    .tile(Icon::GraduationCap)
    .hint(ctx.t_args("skills.emptyFirstDetail", [("project", name)]));
  let row = if may_change {
    row.end(
      ActionButton::new("skills-empty-new", ButtonKind::Secondary, ctx.t("skills.new"))
        .icon(Icon::Plus)
        .opens(p.view.panel.to(Panel::New)),
    )
  } else {
    row
  };
  let mut rows: Vec<Element> = vec![row.into()];
  if p.data.repository.items.is_empty() {
    rows.push(nothing_to_import(ctx));
  }
  group(
    "skills-group-empty",
    head("skills-group-empty", &ctx.t("hub.tabs.skills"), None, None),
    boxed("skills-group-empty-rows", rows),
  )
  .into()
}

/// The repository has no agent files to import (06b): a quiet row in place of Import.
pub(super) fn nothing_to_import(ctx: &Ctx) -> Element {
  Entry::new("skills-nothing-to-import", ctx.t("skills.nothingTitle"))
    .tile(Icon::FolderX)
    .hint(ctx.t("skills.noRepository"))
    .into()
}
