//! The three-way merge of a team MCP server's form (`orchester_workspace::agents::team_mcp_merge`, rule 18 of
//! `docs/design/team-configuration.md`): the version a draft was edited from, the team's newer version and the draft,
//! settled field by field. What only one side changed goes to that side, what both changed to the same value is that
//! value, and what both changed differently has no answer until the person picks one: nothing is dropped unseen.
//!
//! `cargo test -p orchester-workspace --test team_mcp_merge team_mcp_merge_`

use orchester_workspace::{
  agents::team_mcp_merge::{McpField, Pick, Unresolved, Verdict, compare, merge, settle},
  app_mcp::setup::{Header, NewTeamMcpServer, Runs},
};

fn program(arguments: &[&str], exempt: &[&str], folder: &str) -> NewTeamMcpServer {
  NewTeamMcpServer {
    name: "figma".to_owned(),
    runs: Runs::Program {
      program: "npx".to_owned(),
      arguments: arguments.iter().map(ToString::to_string).collect(),
      folder: folder.to_owned(),
      variables: vec!["FIGMA_TOKEN".to_owned()],
    },
    exempt_tools: exempt.iter().map(ToString::to_string).collect(),
  }
}

fn arguments_of(server: &NewTeamMcpServer) -> Vec<String> {
  match &server.runs {
    Runs::Program { arguments, .. } => arguments.clone(),
    Runs::Remote { .. } => Vec::new(),
  }
}

#[test]
fn team_mcp_merge_settles_one_value_by_who_changed_it() {
  assert_eq!(settle(Some(&1), &1, &1), Verdict::Unchanged);
  assert_eq!(settle(Some(&1), &1, &2), Verdict::KeepsMine);
  assert_eq!(settle(Some(&1), &2, &1), Verdict::TakesTheirs);
  assert_eq!(
    settle(Some(&1), &2, &2),
    Verdict::Same,
    "both changed it to the same value"
  );
  assert_eq!(settle(Some(&1), &2, &3), Verdict::BothChanged);
  assert_eq!(
    settle(None, &2, &3),
    Verdict::BothChanged,
    "with no base nothing says who changed it, so a difference is both's"
  );
  assert_eq!(settle(None, &2, &2), Verdict::Same);
}

#[test]
fn team_mcp_merge_keeps_what_only_i_changed_and_takes_what_only_they_changed() {
  let base = program(&["--stdio"], &["get_figma_data"], "");
  let mine = program(&["--stdio", "--port", "3845"], &["get_figma_data"], "");
  let theirs = program(&["--stdio"], &["get_figma_data", "get_images"], "");

  let merged = merge(Some(&base), &theirs, &mine, &[]).expect("no field was changed by both");

  assert_eq!(
    arguments_of(&merged),
    ["--stdio", "--port", "3845"],
    "mine, which they left alone"
  );
  assert_eq!(
    merged.exempt_tools,
    ["get_figma_data", "get_images"],
    "theirs, which I left alone"
  );
}

#[test]
fn team_mcp_merge_asks_for_a_field_both_changed_and_changes_nothing_until_it_is_picked() {
  let base = program(&["--stdio"], &[], "");
  let mine = program(&["--stdio", "--port", "3845"], &[], "");
  let theirs = program(&["--stdio", "--port", "3846"], &[], "");

  assert_eq!(
    merge(Some(&base), &theirs, &mine, &[]),
    Err(Unresolved(vec![McpField::Arguments])),
    "neither side is dropped unseen"
  );
  let mine_picked = merge(Some(&base), &theirs, &mine, &[(McpField::Arguments, Pick::Mine)]).expect("picked");
  assert_eq!(arguments_of(&mine_picked), ["--stdio", "--port", "3845"]);
  let theirs_picked = merge(Some(&base), &theirs, &mine, &[(McpField::Arguments, Pick::Theirs)]).expect("picked");
  assert_eq!(arguments_of(&theirs_picked), ["--stdio", "--port", "3846"]);
}

#[test]
fn team_mcp_merge_takes_a_change_both_made_the_same_way_without_asking() {
  let base = program(&["--stdio"], &[], "");
  let both = program(&["--stdio"], &[], "tools/figma");

  let merged = merge(Some(&base), &both, &both, &[]).expect("the same change is no conflict");

  assert_eq!(merged, both);
  let comparison = compare(Some(&base), &both, &both);
  assert_eq!(comparison.rows.len(), 1);
  assert_eq!(comparison.rows[0].field, McpField::Folder);
  assert_eq!(comparison.rows[0].verdict, Verdict::Same);
}

#[test]
fn team_mcp_merge_without_the_base_asks_for_every_difference() {
  let mine = program(&["--stdio", "--port", "3845"], &["a"], "");
  let theirs = program(&["--stdio"], &["a"], "");

  assert_eq!(
    merge(None, &theirs, &mine, &[]),
    Err(Unresolved(vec![McpField::Arguments])),
    "only what differs is asked; what is the same is not"
  );
}

#[test]
fn team_mcp_merge_comparison_lists_only_what_differs_and_what_must_be_settled_first() {
  let base = program(&["--stdio"], &["get_figma_data"], "");
  let mine = program(&["--stdio", "--port", "3845"], &["get_figma_data"], "tools");
  let theirs = program(
    &["--stdio", "--port", "3846"],
    &["get_figma_data", "get_images"],
    "tools",
  );

  let comparison = compare(Some(&base), &theirs, &mine);

  let seen: Vec<_> = comparison.rows.iter().map(|row| (row.field, row.verdict)).collect();
  assert_eq!(
    seen,
    [
      (McpField::Arguments, Verdict::BothChanged),
      (McpField::ExemptTools, Verdict::TakesTheirs),
      (McpField::Folder, Verdict::Same),
    ],
    "what both changed first; the name, program and variables nobody changed are left out"
  );
  assert_eq!(comparison.both_changed(), [McpField::Arguments]);
  assert_eq!(comparison.count(Verdict::TakesTheirs), 1);
}

#[test]
fn team_mcp_merge_a_server_reached_at_an_address_is_settled_by_its_own_fields() {
  let header = |secret: &str| {
    Some(Header {
      name: "Authorization".to_owned(),
      secret: secret.to_owned(),
      prefix: "Bearer ".to_owned(),
    })
  };
  let remote = |url: &str, secret: &str| NewTeamMcpServer {
    name: "github".to_owned(),
    runs: Runs::Remote {
      url: url.to_owned(),
      header: header(secret),
    },
    exempt_tools: Vec::new(),
  };
  let (base, mine, theirs) = (
    remote("https://a.example/mcp", "GITHUB_TOKEN"),
    remote("https://b.example/mcp", "GITHUB_TOKEN"),
    remote("https://a.example/mcp", "GITHUB_PAT"),
  );

  let merged = merge(Some(&base), &theirs, &mine, &[]).expect("each side changed its own field");

  assert_eq!(merged, remote("https://b.example/mcp", "GITHUB_PAT"));
  let fields: Vec<_> = compare(Some(&base), &theirs, &mine)
    .rows
    .iter()
    .map(|row| row.field)
    .collect();
  assert!(!fields.contains(&McpField::Program) && !fields.contains(&McpField::Arguments));
}
