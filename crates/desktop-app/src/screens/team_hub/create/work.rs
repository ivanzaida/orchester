//! What Create does, off the UI thread. In a team's project the resource is created on the team server at once with
//! its first version ([`Servers::create_team`](orchester_workspace::sharing::Servers::create_team),
//! `docs/design/team-configuration-live.md`): nothing of it is kept on this computer, a refusal comes back for the
//! form to say in place, and while the server cannot be reached nothing is created and the form says why. In a local
//! project it is made as v1 on this computer, and that is all. The screens that read the resources are told once the
//! answer is known. A local MCP server has no versions: it is made with the values of its secrets ([`LocalMcp`]).

use std::path::{Path, PathBuf};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  resource::{AgentProfile, Instruction, ResourceKind, Skill, Tool},
  secret::SecretVault,
  time::Timestamp,
};
use orchester_workspace::{
  agents::editor::create_decided,
  app_mcp::setup::{NewTeamMcpServer, team_mcp_names, team_mcp_server},
  background,
  connections::mcp::{Allowed, McpServerDraft, create_with_values},
  instructions::editor as instructions,
  local_archive::Archivable,
  messages::{Message, name_error},
  sharing::team_edit::{EditRefused, NewTeamResource, Publication, Refusal},
  skills::editor as skills,
  tasks::outside::Outside,
  tools::editor::{self as tools, Edit},
  workspace::Workspace,
};

use super::mode::Mode;
use crate::{screens::local_hub::mcp::Pending, services::Services};

/// A local MCP server as the form describes it: the server, the values typed for the secrets its variables take, which
/// go to the secret store with it, and whether runs may use it once it is made. The values stay in the form's own
/// store ([`Pending`]) and are copied out only by the create that stores them, and only those of the secrets the
/// draft takes.
#[derive(Clone, Debug)]
pub(in crate::screens) struct LocalMcp {
  pub draft: McpServerDraft,
  pub values: Pending,
  pub enabled: bool,
}

impl PartialEq for LocalMcp {
  fn eq(&self, other: &Self) -> bool {
    self.draft == other.draft && self.enabled == other.enabled && self.values.same_as(&other.values)
  }
}
impl Eq for LocalMcp {}

/// What the form's fields make, once they are valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Make {
  Profile(Box<AgentProfile>),
  McpServer(Box<NewTeamMcpServer>),
  /// The form of a local project's MCP server, which has no versions.
  LocalMcpServer(Box<LocalMcp>),
  Instruction(Name, Box<Instruction>),
  Skill(String, Box<Skill>),
  Tool(Name, Box<Tool>),
}

impl Make {
  /// The name it makes the resource under.
  fn name(&self) -> String {
    match self {
      Self::Profile(profile) => profile.role().as_str().to_owned(),
      Self::McpServer(input) => input.name.clone(),
      Self::LocalMcpServer(local) => local.draft.name.as_str().to_owned(),
      Self::Instruction(name, _) | Self::Tool(name, _) => name.as_str().to_owned(),
      Self::Skill(name, _) => name.clone(),
    }
  }

  /// The screens that show what it makes.
  const fn outside(&self) -> Outside {
    match self {
      Self::Profile(_) => Outside::Profiles,
      Self::McpServer(_) | Self::LocalMcpServer(_) => Outside::Connections,
      Self::Instruction(..) => Outside::Instructions,
      Self::Skill(..) => Outside::Skills,
      Self::Tool(..) => Outside::Tools,
    }
  }

  /// Whether it adds to what agents may do (a profile's prompt, a server to run, a tool), which is a person's to
  /// decide: it is made only by their press, or when they let automation change agent permissions.
  pub(super) const fn needs_a_person(&self) -> bool {
    matches!(
      self,
      Self::Profile(_) | Self::McpServer(_) | Self::LocalMcpServer(_) | Self::Tool(..)
    )
  }
}

/// A press of Create: what to make, and whether a person (or automation they let decide) pressed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Press {
  pub make: Make,
  pub decided: bool,
}

/// Why nothing was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Failed {
  /// It adds to what agents may do, which only a person decides, and a person did not press Create.
  PersonOnly,
  /// The team server, or this computer, which knows the team cannot take it, refused it, with the name it was made
  /// under.
  Refused(Refusal, String),
  /// The workspace refused it, the team server could not be reached, or it could not be sent; the words say why.
  Said(Message),
}
impl DevtoolsInspectable for Failed {}

pub(super) type Work = FutureAction<Press, Archivable, Failed>;

pub(super) fn work(ctx: &mut Ctx, (root, project, mode): (PathBuf, ProjectId, Mode)) -> Work {
  let services = Services::of(ctx);
  ctx.future_action(move |request: Press| {
    let (services, root) = (services.clone(), root.clone());
    async move {
      let outside = request.make.outside();
      let name = request.make.name();
      let made = match mode {
        Mode::Team => {
          let (workspace, local) = (services.workspace.clone(), root.clone());
          let new = background::run(move || team_resource(&workspace, &local, request))
            .await
            .map_err(Failed::Said)??;
          let id = services
            .servers
            .create_team(&root, new)
            .await
            .map_err(|refused| match refused {
              EditRefused::Refused(refusal) => Failed::Refused(refusal, name),
              other => Failed::Said(other.message()),
            })?;
          Archivable::Resource(id)
        }
        Mode::Local => {
          let (vault, local) = (services.vault.clone(), root.clone());
          background::run(move || make_in(&local, vault.get(), request))
            .await
            .map_err(Failed::Said)??
        }
      };
      services.changes.record(project, outside);
      Ok(made)
    }
  })
}

/// The new team resource `request` makes, in the team's form: nothing of it is kept here. Refused while the team's
/// configuration was not read this session, which a team MCP server's name is checked against.
fn team_resource(workspace: &Workspace, root: &Path, request: Press) -> Result<NewTeamResource, Failed> {
  if request.make.needs_a_person() && !request.decided {
    return Err(Failed::PersonOnly);
  }
  let name = request.make.name();
  let refused = |error: EditRefused| match error {
    EditRefused::Refused(refusal) => Failed::Refused(refusal, name.clone()),
    other => Failed::Said(other.message()),
  };
  let config = workspace
    .team_read(root)
    .config()
    .cloned()
    .ok_or_else(|| Failed::Said(Message::new("teamConfig.reading")))?;
  let id = ResourceId::generate();
  let (kind, name, publication) = match request.make {
    Make::Profile(profile) => (
      ResourceKind::AgentProfile,
      profile.role().clone(),
      Publication::of_profile(root, id, &profile, &config).map_err(refused)?,
    ),
    Make::McpServer(input) => {
      let server =
        team_mcp_server(&input, &team_mcp_names(&config)).map_err(|invalid| Failed::Said(invalid.message()))?;
      let name = Name::new(server.name().as_str()).map_err(|error| Failed::Said(name_error(&error)))?;
      (
        ResourceKind::McpServer,
        name,
        Publication::of_body(root, id, &server).map_err(refused)?,
      )
    }
    Make::Instruction(name, body) => (
      ResourceKind::Instruction,
      name,
      Publication::of_body(root, id, &*body).map_err(refused)?,
    ),
    Make::Skill(name, body) => (
      ResourceKind::Skill,
      Name::new(&name).map_err(|error| Failed::Said(name_error(&error)))?,
      Publication::of_body(root, id, &*body).map_err(refused)?,
    ),
    Make::Tool(name, tool) => (
      ResourceKind::Tool,
      name,
      Publication::of_body(root, id, &*tool).map_err(refused)?,
    ),
    Make::LocalMcpServer(_) => return Err(Failed::Said(Message::new("team_new.localOnly"))),
  };
  Ok(NewTeamResource {
    id,
    kind,
    name,
    publication,
  })
}

/// Makes the resource as its first version in a local project.
fn make_in(root: &Path, vault: &dyn SecretVault, request: Press) -> Result<Archivable, Failed> {
  if request.make.needs_a_person() && !request.decided {
    return Err(Failed::PersonOnly);
  }
  let said = Failed::Said;
  let resource = match request.make {
    // The person's press is what the new server's widening asks for.
    Make::LocalMcpServer(local) => {
      let values = local.values.values_for(&local.draft.secrets());
      return create_with_values(root, vault, (local.draft, &Allowed::Anything), local.enabled, &values)
        .map(Archivable::McpServer)
        .map_err(|error| said(error.message()));
    }
    Make::Profile(profile) => create_decided(root, *profile, request.decided).map_err(said),
    // A team MCP server is the team's alone: a local project's form makes a local one.
    Make::McpServer(_) => return Err(said(Message::new("team_new.teamOnly"))),
    Make::Instruction(name, body) => instructions::create_in(root, name, &body)
      .map(|entry| entry.id)
      .map_err(|error| said(error.message())),
    Make::Skill(name, body) => skills::save_checked_in(root, (None, &name, &body), Timestamp::now())
      .map(|entry| entry.id)
      .map_err(|error| said(error.message())),
    Make::Tool(name, tool) => {
      let edit = Edit::Save {
        name,
        tool,
        before: None,
      };
      // A person's press is what the new tool's widening asks for.
      let allowed = Allowed::These(edit.reasons(None));
      tools::apply_in(root, edit, &allowed).map_err(said)
    }
  };
  resource.map(Archivable::Resource)
}
