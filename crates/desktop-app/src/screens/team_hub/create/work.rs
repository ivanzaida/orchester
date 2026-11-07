//! What Create does, off the UI thread: the resource is made as its first version, v1 with no draft, and sent to the
//! team server at once ([`Servers::send_created`](orchester_workspace::sharing::Servers::send_created)). It counts as
//! created only once the server takes it: a server that refuses it takes it back whole, and the refusal comes back for
//! the form to say in place. A server that cannot be reached leaves it waiting in the outbox, which the form says too.
//! The screens that read the resources are told only after that, so none shows one the server did not take.

use std::path::{Path, PathBuf};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  resource::{AgentProfile, Instruction, Skill, Tool},
  time::Timestamp,
};
use orchester_workspace::{
  agents::{
    editor::create_decided,
    team_new_mcp::{self, NewTeamMcpServer},
  },
  background,
  connections::mcp::Allowed,
  instructions::editor as instructions,
  messages::{Message, store_error},
  sharing::Created,
  skills::editor as skills,
  tasks::outside::Outside,
  tools::editor::{self as tools, Edit},
};

use crate::services::Services;

/// What the form's fields make, once they are valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Make {
  Profile(Box<AgentProfile>),
  McpServer(Box<NewTeamMcpServer>),
  Instruction(Name, Box<Instruction>),
  Skill(String, Box<Skill>),
  Tool(Name, Box<Tool>),
}

impl Make {
  /// The screens that show what it makes.
  const fn outside(&self) -> Outside {
    match self {
      Self::Profile(_) => Outside::Profiles,
      Self::McpServer(_) => Outside::Connections,
      Self::Instruction(..) => Outside::Instructions,
      Self::Skill(..) => Outside::Skills,
      Self::Tool(..) => Outside::Tools,
    }
  }

  /// Whether it adds to what agents may do (a profile's prompt, a server to run, a tool), which is a person's to
  /// decide: it is made only by their press, or when they let automation change agent permissions.
  pub(super) const fn needs_a_person(&self) -> bool {
    matches!(self, Self::Profile(_) | Self::McpServer(_) | Self::Tool(..))
  }
}

/// How a resource that was made stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Made {
  pub id: ResourceId,
  /// The team server could not be reached: the resource waits in the outbox and is sent when it can be.
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
  /// The workspace or the team server refused it.
  Said(Message),
}
impl DevtoolsInspectable for Failed {}

pub(super) type Work = FutureAction<Press, Made, Failed>;

pub(super) fn work(ctx: &mut Ctx, (root, project): (PathBuf, ProjectId)) -> Work {
  let services = Services::of(ctx);
  ctx.future_action(move |request: Press| {
    let (services, root) = (services.clone(), root.clone());
    async move {
      let outside = request.make.outside();
      let local = root.clone();
      let id = background::run(move || make_in(&local, request))
        .await
        .map_err(Failed::Said)??;
      let settled = services.servers.send_created(&root, id).await;
      match settled {
        Ok(Created::Accepted) => {
          // Told only now, so no screen shows a resource the team server did not take.
          services.changes.record(project, outside);
          Ok(Made { id, waiting: false })
        }
        // It waits in the outbox, sent when the server is back: nothing shows it as created before.
        Ok(Created::Waiting) => Ok(Made { id, waiting: true }),
        Err(refused) => Err(Failed::Said(refused.message())),
      }
    }
  })
}

/// Makes the resource as its first version, waiting for the team server.
fn make_in(root: &Path, request: Press) -> Result<ResourceId, Failed> {
  if request.make.needs_a_person() && !request.decided {
    return Err(Failed::PersonOnly);
  }
  let said = Failed::Said;
  match request.make {
    Make::Profile(profile) => create_decided(root, *profile).map_err(said),
    Make::McpServer(input) => {
      let taken = team_new_mcp::team_mcp_names(root).map_err(|error| said(store_error(&error)))?;
      let server = team_new_mcp::team_mcp_server(&input, &taken).map_err(|invalid| said(invalid.message()))?;
      team_new_mcp::create_team_mcp_server(root, &server).map_err(|error| said(store_error(&error)))
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
  }
}
