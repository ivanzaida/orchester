//! The program path this computer set for each team MCP server of a run is kept with the run's inputs and read back; a
//! value is not: the bindings hold paths and ids only.

use orchester_core::run::inputs::{TeamMcpBinding, team_mcp_id};

use super::*;

#[test]
fn team_run_inputs_team_mcp_bindings_round_trip_with_the_program_path_and_no_value() {
  let mut team = team_project();
  let task = team.task();
  let id = team.db.record_team_task(task, revision(3)).expect("identity");
  let server = ResourceId::generate();
  for programs in [
    Vec::new(),
    vec![(server, Some(std::path::PathBuf::from("/opt/docs-mcp")))],
  ] {
    let operation = OperationId::generate();
    let original = team.inputs(accepted(task, operation), Vec::new());
    let mut bindings = original.bindings().clone();
    bindings.team_mcp = programs
      .iter()
      .map(|(server, program)| TeamMcpBinding {
        server: *server,
        program: program.clone(),
      })
      .collect();
    bindings.profiles[0].mcp_servers = bindings
      .team_mcp
      .iter()
      .map(|bound| team_mcp_id(bound.server))
      .collect();
    let inputs = RunInputs::new(original.bundle().clone(), bindings, original.consent().clone()).expect("inputs");
    team.db.accept_team_inputs(id, &inputs, at(6)).expect("accepted");
    let read = team.db.accepted_team_inputs(operation).expect("read").expect("inputs");
    assert_eq!(read.inputs, inputs);
    let raw: String = connection(team.repo.path())
      .query_row(
        "SELECT bindings FROM accepted_run_inputs WHERE operation_id = ?1",
        [operation.to_string()],
        |row| row.get(0),
      )
      .expect("stored JSON");
    assert_eq!(
      raw.contains("team_mcp"),
      !programs.is_empty(),
      "older rows omit the field"
    );
  }
}
