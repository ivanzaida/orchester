//! Stateful status snapshots: full discovery starts relays, targeted reads reuse the thread.
use serde_json::{Value, json};

use super::{Session, emit, fake_codex_tools};

pub fn status(session: &mut Session, message: &Value) {
  let target = message["params"]["serverName"].as_str();
  let mut servers = if let Some(name) = target {
    assert_eq!(message["params"]["threadId"], session.thread);
    assert!(session.project_servers().contains(name));
    vec![json!({"name":name,"runtimeStatus":"connected","toolsError":null})]
  } else {
    assert_eq!(session.status_reads, 0, "full discovery must never be retried");
    session.servers()
  };
  let scenario = &session.model;
  if scenario.contains("mcp-ready") {
    session.status_reads += 1;
    let state = if session.status_reads == 1 || scenario.contains("forever") || scenario.contains("silent") {
      "starting"
    } else if scenario.contains("failed") {
      "failed"
    } else {
      "connected"
    };
    servers.retain(|server| !(scenario.contains("missing") && server["name"] == "readiness"));
    for server in &mut servers {
      if server["name"]
        == if scenario.contains("orchester") {
          "Orchester"
        } else {
          "readiness"
        }
      {
        server["runtimeStatus"] = json!(state);
        if state == "failed" {
          let token = std::env::var("ORCHESTER_APPROVAL_TOKEN").expect("relay token");
          server["toolsError"] = json!(format!("startup refused token {token}\nsecond line"));
        }
      }
    }
    eprintln!("mcp-readiness: {state}");
    if target.is_some() && scenario.contains("silent") {
      return;
    }
  }
  emit(&json!({"id":message["id"],"result":{"data":servers,"nextCursor":null}}));
}

/// Full inventory's independent discovery lists tools and closes its connection.
pub fn orchester_status(args: &[String]) -> Value {
  match fake_codex_tools::start(args).map(|started| started.tools) {
    Some(Ok(tools)) => {
      let tools: serde_json::Map<String, Value> = tools
        .into_iter()
        .map(|tool| (tool.clone(), json!({"name": tool})))
        .collect();
      json!({"name": "Orchester", "runtimeStatus": "connected", "toolsError": null, "tools": tools})
    }
    Some(Err(error)) => json!({"name": "Orchester", "runtimeStatus": "connected", "toolsError": error}),
    None => json!({"name": "Orchester", "runtimeStatus": "failed", "toolsError": "no command"}),
  }
}
