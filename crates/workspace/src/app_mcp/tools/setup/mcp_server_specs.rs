//! The tool specs of the MCP servers a project's profiles can give their agents.

use rmcp::handler::server::common::schema_for_type;

use super::mcp_servers;
use crate::app_mcp::tools::Spec;

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "add_mcp_server",
    description: "Adds an MCP server to a project, as Connections › Add MCP server does: a unique `name` (small \
                  letters, digits, - and _, starting with a letter) and either a `program` the agent's CLI starts \
                  with each agent (a name on the search path or an absolute path, stored as the fixed file it is, \
                  links followed, outside the project's repository and on this computer, as a command tool's \
                  program; with `args`, `working_directory` inside the worktree or absolute, and `environment`, where \
                  ${secret:NAME@environment} takes the value of an active project secret when an agent starts) or a \
                  remote `url` it reaches over HTTP (with an optional `authorization`: scheme bearer, basic with a \
                  username and the password's secret, basic_credential with a pasted Basic credential's secret, or \
                  header with a header name and the value's secret; each secret an active project secret \
                  NAME@environment, never a value), and `exempt_tools`, the server's tools that run without asking \
                  for profiles that ask before MCP actions. A server runs only for profiles that choose it \
                  (save_profile_draft). Refused as person_only unless a person turned on Let automation change \
                  agent permissions. Returns the server as list_connections lists it. In a team project it creates \
                  the team's MCP server as v1 on the team server (admins only, a member is refused not_admin; no \
                  draft; offline it waits and the answer says waiting_for_server, and a server the team server \
                  refuses is not created): the team holds names only, so `program` is a name looked up on the search \
                  path (npx), never a path, `working_directory` is relative, the variables the program needs are \
                  named in `variable_names` and the headers a remote server is sent a secret in in `secret_headers` \
                  (`Authorization` for a bearer or Basic credential), and a value or a secret in `environment` or \
                  `authorization` is refused (team_value_refused, a path team_path_refused). A team server's `url` \
                  has no query or fragment (team_value_refused): a key in it would reach every member, so a secret \
                  goes in `secret_headers`. Each computer sets the program's path and the values itself.",
    schema: schema_for_type::<mcp_servers::AddMcpServer>,
    read_only: false,
  },
  Spec {
    name: "update_mcp_server",
    description: "Changes an MCP server's fields, as Edit MCP server does; a field left out keeps its value, and \
                  `program` or `url` switches how it is reached; a field the transport does not have is refused \
                  (field_not_applicable). Dropping the authorization (scheme none) and removing tools that run \
                  without asking are always taken. A new name (a run's answers approve a server's tools by its \
                  name), another program, arguments, folder or endpoint, another authorization that sends a secret \
                  (another secret, scheme, username or header), any variable but RUST_LOG, RUST_BACKTRACE, CI, \
                  NO_COLOR, FORCE_COLOR, TZ, LANG and the LC_ ones (removals included), a variable that takes a \
                  secret under any name, and a tool added to exempt_tools are refused as person_only unless a person \
                  turned on Let automation change agent permissions, judged by the fields as given before anything \
                  reads the disk. Whether it is on stays. A team's MCP server (in a team project, by id or name) is the \
                  team's: an admin's change that differs from the version this computer uses is saved as the draft \
                  and published as the next version on the team server at once (offline it waits and the answer \
                  says waiting_for_server; a change that widens what agents may do, a different program or \
                  arguments, address or tools that run without asking, is person_only as above), a member is \
                  refused not_admin, a different `name` is refused as name_not_editable, a change that changes \
                  nothing publishes nothing (`unchanged`), and the fields are the team's (`variable_names`, \
                  `secret_headers`; environment and authorization are refused as above). `program` is the team \
                  body's program name (npx), team data like the rest: another one is person_only unless a person \
                  turned on Let automation change agent permissions, and a path is team_path_refused. The program's \
                  path on this computer is set by a person in the window, never by a tool. `name` is the body's \
                  name and is not editable here (name_not_editable): rename the resource in the window. A draft of \
                  the server that differs from the version this computer uses (someone's unpublished edit) is not \
                  replaced: the call is refused (draft_changed). The `url` of a team's server has no query or \
                  fragment (a key in it would reach every member: team_value_refused); a secret goes in \
                  `secret_headers`.",
    schema: schema_for_type::<mcp_servers::UpdateMcpServer>,
    read_only: false,
  },
  Spec {
    name: "remove_mcp_server",
    description: "Removes an MCP server from a project. Refused as in_use while an agent profile's draft or latest \
                  published version chooses it (the reasons name their roles): switch them first, or turn the \
                  server off with set_mcp_server_enabled, which keeps its settings. A team's MCP server (in a team \
                  project, by id or name) is archived on the team server instead, admins only (a member is refused \
                  not_admin), and never refused for being used: the answer first reads what uses it, `used_by` \
                  { resources, runs } as the team server lists them (the runs that have not ended and pin it), and \
                  `runs_stopping_here`, the runs on this computer whose profile version chooses it. They are the \
                  runs that will stop, not runs that have stopped: this computer cancels them when the team \
                  server's archive reaches it, right after the answer, and the other computers do the same. Nothing \
                  is archived while the team server cannot be reached or what uses it cannot be read, nor a server \
                  the team server has not received yet (team_mcp_server_not_sent).",
    schema: schema_for_type::<mcp_servers::InServer>,
    read_only: false,
  },
  Spec {
    name: "set_mcp_server_enabled",
    description: "Turns an MCP server on or off for every profile that chooses it, as its inspector's Turn on and \
                  Turn off do: agents that start from now on get it, or work without it; agents at work keep what \
                  they started with. Turning off is always taken; turning on is refused as person_only unless a \
                  person turned on Let automation change agent permissions. In a team project `server` can also be \
                  one of the team's MCP servers (by id or name): turning it off here is taken, but turning it on is \
                  refused as person_only always, also while Let automation change agent permissions is on, with \
                  nothing changed: only a person turns a team MCP server on here, in the window.",
    schema: schema_for_type::<mcp_servers::SetMcpServerEnabled>,
    read_only: false,
  },
];
