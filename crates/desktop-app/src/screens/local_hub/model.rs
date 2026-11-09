//! What the hub's list shows in a local project: its profiles, MCP servers, tools, instructions and skills grouped by
//! what the person has to do about them (*Needs you*, *Ready*, *Off*), what was archived apart, and the one the panel
//! shows. A local MCP server is a connection with its own id, so an entry is named by an [`Archivable`], not a resource
//! id.

use orchester_core::{
  connection::AgentProvider,
  resource::{ResourceKind, VersionNumber},
  time::Timestamp,
};
use orchester_workspace::{
  agents::{
    local_hub::{LocalHub, LocalProfile, mcp_form},
    team_hub::Form,
  },
  local_archive::Archivable,
  readiness::{Group, ReadinessStanding},
};

use crate::{
  routes::{HubFilter, HubView},
  ui::icon::Icon,
};

/// One row of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Entry {
  pub what: Archivable,
  pub kind: ResourceKind,
  pub name: String,
  /// The version this computer uses; a local MCP server has none.
  pub number: Option<VersionNumber>,
  /// What an MCP server or a tool is.
  pub form: Option<Form>,
  /// The CLI a profile runs with.
  pub cli: Option<AgentProvider>,
  pub group: Group,
  pub standing: Option<ReadinessStanding>,
  /// A profile's connection, its MCP servers and its pipelines.
  pub profile: Option<LocalProfile>,
  /// The profiles that choose an MCP server.
  pub used_by: Vec<String>,
  /// When it was archived; `None` while it is not.
  pub archived: Option<Timestamp>,
}

impl Entry {
  /// The glyph at a row's start.
  pub(super) const fn glyph(&self) -> Icon {
    match (self.kind, self.form) {
      (ResourceKind::AgentProfile, _) => Icon::Bot,
      (ResourceKind::McpServer, Some(Form::McpRemote)) => Icon::Globe,
      (ResourceKind::McpServer, _) => Icon::Plug,
      (ResourceKind::Tool, Some(Form::ToolCommand)) => Icon::Terminal,
      (ResourceKind::Tool, Some(Form::ToolHttp)) => Icon::Globe,
      (ResourceKind::Tool, _) => Icon::Wrench,
      (ResourceKind::Instruction, _) => Icon::FileText,
      _ => Icon::Sparkles,
    }
  }

  /// A row's height: 56 when it says what the person has to do, 44 for a quiet one.
  pub(super) fn height(&self) -> f32 {
    if self.group == Group::NeedsYouHere || self.kind == ResourceKind::McpServer || self.archived.is_some() {
      ROW_TALL
    } else {
      ROW_SHORT
    }
  }

  /// Whether the filter holds it: the kind it names, or All; an archived one only under Archived.
  fn held_by(&self, filter: HubFilter) -> bool {
    match filter {
      HubFilter::Archived => self.archived.is_some(),
      _ if self.archived.is_some() => false,
      HubFilter::All => true,
      HubFilter::Profiles => self.kind == ResourceKind::AgentProfile,
      HubFilter::McpServers => self.kind == ResourceKind::McpServer,
      HubFilter::Instructions => self.kind == ResourceKind::Instruction,
      HubFilter::Skills => self.kind == ResourceKind::Skill,
      HubFilter::Tools => self.kind == ResourceKind::Tool,
    }
  }
}

/// A row that says what the person has to do (`Item · pencil-design`).
const ROW_TALL: f32 = 56.0;
/// A quiet row (`Item · Project rules`).
const ROW_SHORT: f32 = 44.0;

/// The groups' order in the list.
pub(super) const GROUPS: [Group; 3] = [Group::NeedsYouHere, Group::Ready, Group::OffHere];

/// Every entry of the hub, and how many each filter holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Listing {
  entries: Vec<Entry>,
}

impl Listing {
  /// Every entry, in the order of the list: by group, then kind, then name; the archived ones by when.
  pub(super) fn of(hub: &LocalHub) -> Self {
    let mut entries = Vec::new();
    for found in hub.readiness().resources() {
      let id = found.resource();
      entries.push(Entry {
        what: Archivable::Resource(id),
        kind: found.kind(),
        name: found.name().to_owned(),
        number: Some(found.number()),
        form: hub.tool_form(id),
        cli: hub.profile(id).and_then(|profile| profile.cli),
        group: found.group(),
        standing: Some(found.standing().clone()),
        profile: hub.profile(id).cloned(),
        used_by: Vec::new(),
        archived: None,
      });
    }
    for found in hub.plain() {
      entries.push(Entry {
        what: Archivable::Resource(found.id),
        kind: found.kind,
        name: found.name.clone(),
        number: found.number,
        form: None,
        cli: None,
        group: Group::Ready,
        standing: None,
        profile: None,
        used_by: Vec::new(),
        archived: None,
      });
    }
    for found in hub.mcp_servers() {
      entries.push(Entry {
        what: Archivable::McpServer(found.id()),
        kind: ResourceKind::McpServer,
        name: found.server().name().to_string(),
        number: None,
        form: Some(mcp_form(found.server().transport())),
        cli: None,
        group: found.group(),
        standing: Some(found.standing().clone()),
        profile: None,
        used_by: hub
          .profiles_choosing(found.id())
          .into_iter()
          .map(str::to_owned)
          .collect(),
        archived: None,
      });
    }
    entries.sort_by_cached_key(|entry| {
      (
        group_rank(entry.group),
        kind_rank(entry.kind),
        entry.name.to_lowercase(),
      )
    });
    entries.extend(hub.archived().iter().map(|found| Entry {
      what: found.what,
      kind: found.kind,
      name: found.name.clone(),
      number: None,
      form: None,
      cli: None,
      group: Group::Ready,
      standing: None,
      profile: None,
      used_by: Vec::new(),
      archived: Some(found.at),
    }));
    Self { entries }
  }

  /// The name of the entry `what`.
  pub(super) fn name_of(&self, what: Archivable) -> Option<&str> {
    self
      .entries
      .iter()
      .find(|entry| entry.what == what)
      .map(|entry| entry.name.as_str())
  }

  /// How many entries `filter` holds.
  pub(super) fn count(&self, filter: HubFilter) -> usize {
    self.shown(filter).count()
  }

  pub(super) fn shown(&self, filter: HubFilter) -> impl Iterator<Item = &Entry> {
    self.entries.iter().filter(move |entry| entry.held_by(filter))
  }

  /// The entries of `group` that `filter` holds.
  pub(super) fn group(&self, filter: HubFilter, group: Group) -> impl Iterator<Item = &Entry> {
    self.shown(filter).filter(move |entry| entry.group == group)
  }

  /// The entry the panel shows: the one the route names while the filter holds it, else the first of the list; under
  /// Archived nothing is chosen until the person picks one (`Agents · Archived` L13).
  pub(super) fn selected(&self, view: &HubView) -> Option<&Entry> {
    let chosen = chosen(view);
    chosen
      .and_then(|what| self.shown(view.filter).find(|entry| entry.what == what))
      .or_else(|| {
        (view.filter != HubFilter::Archived)
          .then(|| self.shown(view.filter).next())
          .flatten()
      })
  }

  /// Whether `filter` holds the entry `what`.
  pub(super) fn holds(&self, filter: HubFilter, what: Archivable) -> bool {
    self.shown(filter).any(|entry| entry.what == what)
  }
}

/// What a route names.
pub(super) fn chosen(view: &HubView) -> Option<Archivable> {
  match (view.item, view.server) {
    (Some(id), _) => Some(Archivable::Resource(id)),
    (None, Some(server)) => Some(Archivable::McpServer(server)),
    (None, None) => None,
  }
}

/// The hub on `what` under `filter`.
pub(super) fn view_of(filter: HubFilter, what: Option<Archivable>) -> HubView {
  match what {
    Some(Archivable::Resource(id)) => HubView::new(filter, Some(id)),
    Some(Archivable::McpServer(server)) => HubView::on_server(filter, server),
    None => HubView::new(filter, None),
  }
}

const fn group_rank(group: Group) -> u8 {
  match group {
    Group::NeedsYouHere => 0,
    Group::Ready => 1,
    Group::OffHere => 2,
  }
}

const fn kind_rank(kind: ResourceKind) -> u8 {
  match kind {
    ResourceKind::AgentProfile => 0,
    ResourceKind::McpServer => 1,
    ResourceKind::Tool => 2,
    ResourceKind::Instruction => 3,
    _ => 4,
  }
}
