//! What the hub's list shows: the team's resources of the kinds the filter names, grouped by what this computer needs
//! (*Needs you here*, *Ready here*, *Off here*), and the one the panel shows.

use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  resource::{ResourceKind, VersionNumber},
};
use orchester_workspace::{
  agents::team_hub::{Form, TeamHub},
  readiness::{Group, ReadinessStanding},
};

use crate::{routes::HubFilter, ui::icon::Icon};

/// The kinds the hub lists; pipelines keep their own page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
  Profile,
  McpServer,
  Tool,
  Instruction,
  Skill,
}

impl Kind {
  fn of(kind: ResourceKind) -> Option<Self> {
    match kind {
      ResourceKind::AgentProfile => Some(Self::Profile),
      ResourceKind::McpServer => Some(Self::McpServer),
      ResourceKind::Tool => Some(Self::Tool),
      ResourceKind::Instruction => Some(Self::Instruction),
      ResourceKind::Skill => Some(Self::Skill),
      _ => None,
    }
  }

  const fn filter(self) -> HubFilter {
    match self {
      Self::Profile => HubFilter::Profiles,
      Self::McpServer => HubFilter::McpServers,
      Self::Tool => HubFilter::Tools,
      Self::Instruction => HubFilter::Instructions,
      Self::Skill => HubFilter::Skills,
    }
  }

  /// The kind's name in element ids.
  pub(super) const fn name(self) -> &'static str {
    match self {
      Self::Profile => "profile",
      Self::McpServer => "mcp",
      Self::Tool => "tool",
      Self::Instruction => "instruction",
      Self::Skill => "skill",
    }
  }

  pub(super) const fn tag_key(self) -> &'static str {
    match self {
      Self::Profile => "team_hub.kind.tag.profile",
      Self::McpServer => "team_hub.kind.tag.mcp",
      Self::Tool => "team_hub.kind.tag.tool",
      Self::Instruction => "team_hub.kind.tag.instruction",
      Self::Skill => "team_hub.kind.tag.skill",
    }
  }

  pub(super) const fn where_key(self) -> &'static str {
    match self {
      Self::Profile => "team_hub.kind.where.profile",
      Self::McpServer => "team_hub.kind.where.mcp",
      Self::Tool => "team_hub.kind.where.tool",
      Self::Instruction => "team_hub.kind.where.instruction",
      Self::Skill => "team_hub.kind.where.skill",
    }
  }

  /// Whether the kind has something to fill in on a computer, so a row says what it needs (profiles, MCP servers and
  /// tools); instructions and skills are always ready here.
  pub(super) const fn fills_in(self) -> bool {
    matches!(self, Self::Profile | Self::McpServer | Self::Tool)
  }
}

/// What a form's tag says (`Tag` in `Item · pencil-design` `U9RFS`, `Item · sentry-issues`).
pub(super) const fn form_key(form: Form) -> &'static str {
  match form {
    Form::McpProgram => "team_hub.form.program",
    Form::McpRemote => "team_hub.form.remote",
    Form::ToolCommand => "team_hub.form.command",
    Form::ToolHttp => "team_hub.form.http",
    Form::ToolDatabase => "team_hub.form.database",
  }
}

/// One resource of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Entry {
  pub id: ResourceId,
  pub kind: Kind,
  pub name: String,
  pub number: Option<VersionNumber>,
  /// What an MCP server or a tool is; `None` for the other kinds.
  pub form: Option<Form>,
  /// The CLI kind a profile runs with, whose mark stands at its row's start; `None` for the other kinds and for a
  /// profile that is not in the team's form.
  pub cli: Option<AgentProvider>,
  pub group: Group,
  /// How it stands here; `None` for an instruction or a skill, which need nothing.
  pub standing: Option<ReadinessStanding>,
}

impl Entry {
  /// The glyph at a row's start for what is not a profile with a CLI (`Item` in `t4mtQL`): an MCP server's is a plug,
  /// or a globe when it is reached at an address; a tool's is a wrench, a terminal for a command and a globe for a
  /// request.
  pub(super) const fn glyph(&self) -> Icon {
    match (self.kind, self.form) {
      (Kind::Profile, _) => Icon::Bot,
      (Kind::McpServer, Some(Form::McpRemote)) => Icon::Globe,
      (Kind::McpServer, _) => Icon::Plug,
      (Kind::Tool, Some(Form::ToolCommand)) => Icon::Terminal,
      (Kind::Tool, Some(Form::ToolHttp)) => Icon::Globe,
      (Kind::Tool, _) => Icon::Wrench,
      (Kind::Instruction, _) => Icon::FileText,
      (Kind::Skill, _) => Icon::Sparkles,
    }
  }

  /// A row's height: 56 when it says what a computer needs, 44 for a quiet one (`Item` in `t4mtQL`, `kgvaK`, `c3jU1R`).
  pub(super) fn height(&self, filter: HubFilter) -> f32 {
    if self.group == Group::NeedsYouHere || (filter != HubFilter::All && self.kind.fills_in()) {
      ROW_TALL
    } else {
      ROW_SHORT
    }
  }
}

/// A row that says what this computer needs (`Item · Builder` `tF2OL`).
pub(super) const ROW_TALL: f32 = 56.0;
/// A quiet row (`Item · Project rules` `NRyeu`).
pub(super) const ROW_SHORT: f32 = 44.0;

/// The groups' order in the list.
pub(super) const GROUPS: [Group; 3] = [Group::NeedsYouHere, Group::Ready, Group::OffHere];

/// The team's resources, and how many each filter holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Listing {
  entries: Vec<Entry>,
}

impl Listing {
  /// Every resource the hub lists, in the order of the list: by group, then kind, then name.
  pub(super) fn of(hub: &TeamHub) -> Self {
    let readiness = hub.readiness().resources().iter().filter_map(|found| {
      Some(Entry {
        id: found.resource(),
        kind: Kind::of(found.kind())?,
        name: found.name().to_owned(),
        number: Some(found.number()),
        form: hub.form(found.resource()),
        cli: hub.profile(found.resource()).map(|sheet| sheet.cli),
        group: found.group(),
        standing: Some(found.standing().clone()),
      })
    });
    let plain = hub.plain().iter().filter_map(|found| {
      Some(Entry {
        id: found.id,
        kind: Kind::of(found.kind)?,
        name: found.name.clone(),
        number: found.number,
        form: None,
        cli: None,
        group: Group::Ready,
        standing: None,
      })
    });
    let mut entries: Vec<Entry> = readiness.chain(plain).collect();
    entries.sort_by_cached_key(|entry| (group_rank(entry.group), entry.kind as u8, entry.name.to_lowercase()));
    Self { entries }
  }

  /// How many resources `filter` holds.
  pub(super) fn count(&self, filter: HubFilter) -> usize {
    self.shown(filter).count()
  }

  /// The resources `filter` holds, in the order of the list.
  pub(super) fn shown(&self, filter: HubFilter) -> impl Iterator<Item = &Entry> {
    self
      .entries
      .iter()
      .filter(move |entry| filter == HubFilter::All || entry.kind.filter() == filter)
  }

  /// The resources of `group` that `filter` holds.
  pub(super) fn group(&self, filter: HubFilter, group: Group) -> impl Iterator<Item = &Entry> {
    self.shown(filter).filter(move |entry| entry.group == group)
  }

  /// The resource the panel shows: `chosen` while the filter holds it, else the first one of the list.
  pub(super) fn selected(&self, filter: HubFilter, chosen: Option<ResourceId>) -> Option<&Entry> {
    chosen
      .and_then(|id| self.shown(filter).find(|entry| entry.id == id))
      .or_else(|| self.shown(filter).next())
  }

  /// A resource of the team, wherever the filter stands.
  pub(super) fn find(&self, id: ResourceId) -> Option<&Entry> {
    self.entries.iter().find(|entry| entry.id == id)
  }
}

const fn group_rank(group: Group) -> u8 {
  match group {
    Group::NeedsYouHere => 0,
    Group::Ready => 1,
    Group::OffHere => 2,
  }
}
