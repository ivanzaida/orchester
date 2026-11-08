//! The hub's new team MCP server (`Agents · New MCP server` A8 in `acFkz`): its fields are checked by the rules
//! `add_mcp_server` applies, so a name the team has, a path and a query in a URL are each refused with the field they
//! are about, and a program's name, its variables and a remote server's one secret header make the team's body.
//! `cargo test -p orchester-workspace --test team_new_mcp team_new_mcp`.

use orchester_core::{connection::mcp::McpServerName, resource::TeamMcpLaunch};
use orchester_workspace::app_mcp::setup::{Header, Invalid, NewTeamMcpServer, Runs, team_mcp_server};

fn program(name: &str, program: &str) -> NewTeamMcpServer {
  NewTeamMcpServer {
    name: name.to_owned(),
    runs: Runs::Program {
      program: program.to_owned(),
      arguments: vec!["--stdio".to_owned()],
      folder: String::new(),
      variables: vec!["FIGMA_TOKEN".to_owned()],
    },
    exempt_tools: Vec::new(),
  }
}

fn remote(url: &str, header: Option<Header>) -> NewTeamMcpServer {
  NewTeamMcpServer {
    name: "github".to_owned(),
    runs: Runs::Remote {
      url: url.to_owned(),
      header,
    },
    exempt_tools: Vec::new(),
  }
}

#[test]
fn team_new_mcp_a_program_is_named_and_its_variables_are_names_only() {
  let server = team_mcp_server(&program("figma", "npx"), &[]).expect("a team server");
  let TeamMcpLaunch::Program(launch) = server.launch() else {
    panic!("a program");
  };
  assert_eq!(launch.program().as_str(), "npx");
  assert_eq!(
    launch.variables().iter().map(|name| name.as_str()).collect::<Vec<_>>(),
    ["FIGMA_TOKEN"]
  );
}

#[test]
fn team_new_mcp_a_name_the_team_has_a_path_and_a_url_with_a_key_are_refused_at_their_field() {
  let taken = [McpServerName::new("figma").expect("a name")];
  assert!(matches!(
    team_mcp_server(&program("figma", "npx"), &taken).map_err(|invalid| *invalid),
    Err(Invalid::Name(_))
  ));
  assert!(matches!(
    team_mcp_server(&program("figma", "/usr/local/bin/npx"), &[]).map_err(|invalid| *invalid),
    Err(Invalid::Program(_))
  ));
  let mut folder = program("figma", "npx");
  if let Runs::Program { folder: path, .. } = &mut folder.runs {
    *path = "/srv/tools".to_owned();
  }
  assert!(matches!(
    team_mcp_server(&folder, &[]).map_err(|invalid| *invalid),
    Err(Invalid::Folder(_))
  ));
  assert!(matches!(
    team_mcp_server(&remote("https://mcp.example.org/x?key=abc", None), &[]).map_err(|invalid| *invalid),
    Err(Invalid::Url(_))
  ));
}

#[test]
fn team_new_mcp_a_remote_server_names_the_secret_its_header_takes() {
  let header = Header {
    name: "Authorization".to_owned(),
    secret: "GITHUB_TOKEN".to_owned(),
    prefix: "Bearer ".to_owned(),
  };
  let server = team_mcp_server(&remote("https://mcp.example.org/x", Some(header)), &[]).expect("a team server");
  let TeamMcpLaunch::Remote(launch) = server.launch() else {
    panic!("remote");
  };
  assert_eq!(launch.secret_headers().len(), 1);
  assert_eq!(launch.secret_headers()[0].secret().as_str(), "GITHUB_TOKEN");
  // A value put where a name goes is refused, never kept.
  let value = Header {
    name: "Authorization".to_owned(),
    secret: "not a name".to_owned(),
    prefix: String::new(),
  };
  assert!(team_mcp_server(&remote("https://mcp.example.org/x", Some(value)), &[]).is_err());
}
