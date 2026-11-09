//! What Create does, off the UI thread: the resource is made as its first version, v1 with no draft, and sent to the
//! team server at once ([`Servers::send_created`](orchester_workspace::sharing::Servers::send_created)); a local
//! project has nothing to send, so there it is made and that is all. A server that refuses it takes it back whole, and
//! the refusal comes back for the form to say in place. A server that cannot be reached leaves it kept in the outbox,
//! which the form says too and the list shows as waiting to send; it is sent when the server answers. The screens that
//! read the resources are told once the answer is known, so none shows one the server refused. A local MCP server has
//! no versions: it is made with the values of its secrets ([`LocalMcp`]) and is not sent anywhere.

use std::path::{Path, PathBuf};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId},
  name::Name,
  resource::{AgentProfile, Instruction, Skill, Tool},
  secret::SecretVault,
  time::Timestamp,
};
use orchester_workspace::{
  agents::editor::create_decided,
  app_mcp::setup::{NewTeamMcpServer, create_team_mcp_server, team_mcp_names, team_mcp_server},
  background,
  connections::mcp::{Allowed, McpServerDraft, create_with_values},
  instructions::editor as instructions,
  local_archive::Archivable,
  messages::{Message, store_error},
  sharing::{Created, NotCreated, team_edit::Refusal},
  skills::editor as skills,
  tasks::outside::Outside,
  tools::editor::{self as tools, Edit},
};

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

/// How a resource that was made stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Made {
  pub what: Archivable,
  /// The team server could not be reached: the resource is kept in the outbox and sent when it can be.
  pub waiting: bool,
}
impl DevtoolsInspectable for Made {}

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
  /// The team server refused it, with the name it was made under.
  Refused(Refusal, String),
  /// The workspace refused it, or it could not be sent for a reason other than being offline.
  Said(Message),
}
impl DevtoolsInspectable for Failed {}

pub(super) type Work = FutureAction<Press, Made, Failed>;

pub(super) fn work(ctx: &mut Ctx, (root, project): (PathBuf, ProjectId)) -> Work {
  let services = Services::of(ctx);
  let vault = services.vault.clone();
  ctx.future_action(move |request: Press| {
    let (services, root, vault) = (services.clone(), root.clone(), vault.clone());
    async move {
      let outside = request.make.outside();
      let name = request.make.name();
      let local = root.clone();
      let kept = background::run(move || make_in(&local, vault.get(), request))
        .await
        .map_err(Failed::Said)??;
      let id = match kept {
        Kept::Resource(id) => id,
        Kept::Server(server) => {
          services.changes.record(project, outside);
          return Ok(Made {
            what: Archivable::McpServer(server),
            waiting: false,
          });
        }
      };
      let settled = services.servers.send_created(&root, id).await;
      match settled {
        // Told once the answer is known, so no screen shows a resource the team server refused. One kept in the
        // outbox is read as waiting to send, not as published.
        Ok(created) => {
          services.changes.record(project, outside);
          Ok(Made {
            what: Archivable::Resource(id),
            waiting: created == Created::Waiting,
          })
        }
        Err(NotCreated::Refused { code }) => Err(Failed::Refused(Refusal::of_code(&code), name)),
        Err(refused) => Err(Failed::Said(refused.message())),
      }
    }
  })
}

/// What was made.
enum Kept {
  /// A versioned resource: in a team's project it goes to the team server next.
  Resource(ResourceId),
  /// A local MCP server, which has no versions and goes nowhere.
  Server(ConnectionId),
}

/// Makes the resource as its first version, waiting for the team server in a team's project.
fn make_in(root: &Path, vault: &dyn SecretVault, request: Press) -> Result<Kept, Failed> {
  if request.make.needs_a_person() && !request.decided {
    return Err(Failed::PersonOnly);
  }
  let said = Failed::Said;
  let resource = match request.make {
    // The person's press is what the new server's widening asks for.
    Make::LocalMcpServer(local) => {
      let values = local.values.values_for(&local.draft.secrets());
      return create_with_values(root, vault, (local.draft, &Allowed::Anything), local.enabled, &values)
        .map(Kept::Server)
        .map_err(|error| said(error.message()));
    }
    Make::Profile(profile) => create_decided(root, *profile, request.decided).map_err(said),
    Make::McpServer(input) => {
      let taken = team_mcp_names(root).map_err(|error| said(store_error(&error)))?;
      let server = team_mcp_server(&input, &taken).map_err(|invalid| said(invalid.message()))?;
      create_team_mcp_server(root, &server).map_err(|error| said(store_error(&error)))
    }
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
  resource.map(Kept::Resource)
}
