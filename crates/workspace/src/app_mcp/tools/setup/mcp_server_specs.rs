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
                  agent permissions. Returns the server as list_connections lists it.",
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
                  team's: its definition is changed on the team server, and giving it a `program` on this computer is \
                  refused as person_only always, also while Let automation change agent permissions is on, with \
                  nothing changed: only a person chooses the program in the window.",
    schema: schema_for_type::<mcp_servers::UpdateMcpServer>,
    read_only: false,
  },
  Spec {
    name: "remove_mcp_server",
    description: "Removes an MCP server from a project. Refused as in_use while an agent profile's draft or latest \
                  published version chooses it (the reasons name their roles): switch them first, or turn the \
                  server off with set_mcp_server_enabled, which keeps its settings.",
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
