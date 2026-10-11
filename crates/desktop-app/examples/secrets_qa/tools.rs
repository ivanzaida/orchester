//! Example-only fault controls; no argument, response or log can carry a credential.
use std::sync::Arc;

use lurq::{
  app::Tree,
  mcp::{McpTool, Scope},
};
use orchester_app::{mcp::InputGate, services::Services};
use orchester_workspace::{tasks::outside::Outside, workspace::OpenProject};
use serde_json::json;

use super::{fixtures, vault::QaVault};

pub fn tool(vault: Arc<QaVault>, open: OpenProject, services: Services, gate: InputGate) -> McpTool {
  McpTool::new("qa_secrets")
    .description("Native Secrets QA example only: fill synthetic masked input; hold/release/fail vault writes; damage/repair its fixture metadata. Never supplies or returns a credential. Person-only input remains protected.")
    .scope(Scope::Interact)
    .input_schema(json!({
      "type":"object",
      "properties":{"action":{"type":"string","enum":["fill","hold","allow","deny","backend","read-error","repair-read"]}},
      "required":["action"],"additionalProperties":false
    }))
    .handler(move |ctx, args| {
      let action = args.get("action").and_then(serde_json::Value::as_str).ok_or("action is required")?;
      match action {
        "fill" => fill(ctx.tree, &gate)?,
        "hold" => vault.configure(true, 0),
        "allow" => vault.configure(false, 0),
        "deny" => vault.configure(false, 1),
        "backend" => vault.configure(false, 2),
        "read-error" | "repair-read" => {
          fixtures::read_failure(&open.repo_root, action == "read-error")
            .map_err(|error| error.to_string())?;
          if action == "read-error" {
            services.changes.record(open.project.id(), Outside::Connections);
          }
        }
        _ => return Err("unknown QA action".into()),
      }
      ctx.tree.request_redraw();
      Ok(json!({"applied":action,"vault_entered":vault.entered()}))
    })
}

/// Fills only the mounted masked input, before any person-only question. No arbitrary input or clicks.
pub fn fill(tree: &mut Tree, gate: &InputGate) -> Result<(), String> {
  if gate.protecting() {
    return Err("person_only".into());
  }
  let mut input = tree
    .get_element_by_id_mut("secret-value")
    .and_then(|element| element.as_text_input())
    .ok_or("no editable credential field")?;
  if !input.is_masked() {
    return Err("credential field is not masked".into());
  }
  input.set_value("native-qa-only-new-credential");
  Ok(())
}
