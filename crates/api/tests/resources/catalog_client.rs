//! The catalog's client calls go to their endpoints: the report as JSON, answered `204`, and the two reads.

use orchester_api::CatalogReport;
use orchester_core::{id::ProjectId, resource::ModelId};
use serde_json::{Value, json};

use crate::client::{Server, json_answer, token};

#[tokio::test]
async fn catalog_the_report_and_the_reads_go_to_their_endpoints() {
  let project = ProjectId::generate();
  let server = Server::start(vec![
    (204, Vec::new()),
    json_answer(200, &json!({"cli_kinds":["claude_code_cli","codex_cli"]})),
    json_answer(200, &json!({"models":{"codex_cli":["gpt-5.5"]}})),
  ]);
  let client = server.client();
  let report = CatalogReport {
    cli_kinds: vec!["codex_cli".to_owned()],
    models: [("codex_cli".to_owned(), vec![ModelId::new("gpt-5.5").expect("model")])].into(),
  };
  client.report_catalog(&token(), project, &report).await.expect("report");
  let cli_kinds = client.catalog_cli_kinds(&token(), project).await.expect("cli kinds");
  assert_eq!(cli_kinds, ["claude_code_cli", "codex_cli"]);
  let models = client.catalog_models(&token(), project).await.expect("models");
  assert_eq!(models["codex_cli"][0].as_str(), "gpt-5.5");
  let seen = server.seen();
  let base = format!("/api/v1/projects/{project}");
  let lines: Vec<_> = seen.iter().map(|seen| seen.line.clone()).collect();
  assert_eq!(
    lines,
    [
      format!("PUT {base}/catalog"),
      format!("GET {base}/cli-kinds"),
      format!("GET {base}/models"),
    ]
  );
  assert_eq!(seen[0].content_type.as_deref(), Some("application/json"));
  let sent: Value = serde_json::from_slice(&seen[0].body).expect("a JSON body");
  assert_eq!(
    sent,
    json!({"cli_kinds":["codex_cli"],"models":{"codex_cli":["gpt-5.5"]}})
  );
}
