//! What the hub's list shows: the team's resources of the kinds the filter names, grouped by what this computer needs
//! (*Needs you here*, *Ready here*, *Off here*), and the one the panel shows.

use std::collections::BTreeMap;

use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  resource::{ResourceKind, VersionNumber},
};
use orchester_store::{PublishState, SharedStanding};
use orchester_workspace::{
  agents::{
    team_hub::{Form, TeamHub},
    team_text::Added,
  },
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

  /// The kind the filter holds; `None` for All, which holds them all.
  pub(super) const fn of_filter(filter: HubFilter) -> Option<Self> {
    match filter {
      HubFilter::All => None,
      HubFilter::Profiles => Some(Self::Profile),
      HubFilter::McpServers => Some(Self::McpServer),
      HubFilter::Instructions => Some(Self::Instruction),
      HubFilter::Skills => Some(Self::Skill),
      HubFilter::Tools => Some(Self::Tool),
    }
  }

  pub(super) const fn filter(self) -> HubFilter {
    match self {
      Self::Profile => HubFilter::Profiles,
      Self::McpServer => HubFilter::McpServers,
      Self::Tool => HubFilter::Tools,
      Self::Instruction => HubFilter::Instructions,
      Self::Skill => HubFilter::Skills,
    }
  }

  pub(super) const fn core(self) -> ResourceKind {
    match self {
      Self::Profile => ResourceKind::AgentProfile,
      Self::McpServer => ResourceKind::McpServer,
      Self::Tool => ResourceKind::Tool,
      Self::Instruction => ResourceKind::Instruction,
      Self::Skill => ResourceKind::Skill,
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

/// Who gets a resource of the team, which a ready row says after its name (`Line` in `Item · Project rules` `ezTxs`,
/// `Item · cargo-check` in `u9Hcc`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Reach {
  /// An instruction added to every run.
  EveryRun,
  /// The names of the profiles that use it; none when no profile does.
  Profiles(Vec<String>),
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
  /// Who gets an instruction, a skill or a tool; `None` for the other kinds and for one with no sheet.
  pub reach: Option<Reach>,
  pub group: Group,
  /// How it stands here; `None` for an instruction or a skill, which need nothing.
  pub standing: Option<ReadinessStanding>,
  /// Created here and not yet taken by the team server, which could not be reached: it waits in the outbox to be sent
  /// and has no version number of the team's yet.
  pub waiting: bool,
  /// An admin's draft of the next version is kept on this computer (an MCP server's, so far).
  pub draft: bool,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Listing {
  entries: Vec<Entry>,
}

impl Listing {
  /// Every resource the hub lists, in the order of the list: by group, then kind, then name.
  pub(super) fn of(hub: &TeamHub, standings: &BTreeMap<ResourceId, SharedStanding>) -> Self {
    let waiting = |id| waits_to_be_sent(standings, id);
    let readiness = hub.readiness().resources().iter().filter_map(|found| {
      Some(Entry {
        id: found.resource(),
        kind: Kind::of(found.kind())?,
        name: found.name().to_owned(),
        number: shown_number(standings, found.resource(), Some(found.number())),
        form: hub.form(found.resource()),
        cli: hub.profile(found.resource()).map(|sheet| sheet.cli),
        reach: reach(hub, found.resource(), found.kind()),
        group: found.group(),
        standing: Some(found.standing().clone()),
        waiting: waiting(found.resource()),
        draft: hub
          .mcp_server(found.resource())
          .is_some_and(|sheet| sheet.edit.as_ref().is_some_and(|edit| edit.draft.is_some())),
      })
    });
    let plain = hub.plain().iter().filter_map(|found| {
      Some(Entry {
        id: found.id,
        kind: Kind::of(found.kind)?,
        name: found.name.clone(),
        number: shown_number(standings, found.id, found.number),
        form: None,
        cli: None,
        reach: reach(hub, found.id, found.kind),
        group: Group::Ready,
        standing: None,
        waiting: waiting(found.id),
        draft: false,
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

  /// Whether `filter` holds the resource `id`.
  pub(super) fn holds(&self, filter: HubFilter, id: ResourceId) -> bool {
    self.shown(filter).any(|entry| entry.id == id)
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

/// Whether the resource was created here and no version of it was ever numbered by the team server: its first version
/// is still in the outbox, waiting to be sent. One the server refused (or met a version from the same base) keeps its
/// number and its readiness standing: it waits for the admin's decision, not for the server.
fn waits_to_be_sent(standings: &BTreeMap<ResourceId, SharedStanding>, id: ResourceId) -> bool {
  standings.get(&id).is_some_and(|standing| {
    standing.confirmed.is_none()
      && standing
        .waiting
        .first()
        .is_some_and(|first| first.state == PublishState::Waiting)
  })
}

/// The version number a row and a panel say: the version the team server numbered. A version published here that waits
/// for the server is not yet the team's, so an edit's is the number before it, and a new resource's first has none.
fn shown_number(
  standings: &BTreeMap<ResourceId, SharedStanding>,
  id: ResourceId,
  local: Option<VersionNumber>,
) -> Option<VersionNumber> {
  let Some(standing) = standings.get(&id) else {
    return local;
  };
  let sending = standing
    .waiting
    .iter()
    .any(|waiting| waiting.state == PublishState::Waiting);
  match (&standing.confirmed, sending) {
    (Some(confirmed), true) => Some(confirmed.number),
    (None, true) => None,
    _ => local,
  }
}

fn reach(hub: &TeamHub, id: ResourceId, kind: ResourceKind) -> Option<Reach> {
  match kind {
    ResourceKind::Instruction => hub.instruction(id).map(|sheet| match sheet.added {
      Added::Project => Reach::EveryRun,
      Added::Role(_) | Added::Pipeline(_) | Added::Step { .. } => Reach::Profiles(sheet.used_by.clone()),
    }),
    ResourceKind::Skill => hub.skill(id).map(|sheet| Reach::Profiles(sheet.used_by.clone())),
    ResourceKind::Tool => hub.tool(id).map(|sheet| Reach::Profiles(sheet.used_by.clone())),
    _ => None,
  }
}

const fn group_rank(group: Group) -> u8 {
  match group {
    Group::NeedsYouHere => 0,
    Group::Ready => 1,
    Group::OffHere => 2,
  }
}
