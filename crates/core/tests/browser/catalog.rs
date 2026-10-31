//! The browser tool catalog: fourteen tools and `browser_eval`, each with a strict schema whose properties are the
//! fields its call takes.

use std::collections::HashSet;

use orchester_core::orchester_tools::browser::{BrowserCall, BrowserCallError, BrowserReach, BrowserTool};
use serde_json::{Value, json};

#[test]
fn browser_catalog_offers_fourteen_tools_and_eval() {
  let names: Vec<&str> = BrowserTool::ALL.iter().map(|tool| tool.as_str()).collect();
  assert_eq!(
    names,
    [
      "browser_tabs",
      "browser_open",
      "browser_navigate",
      "browser_close",
      "browser_read_page",
      "browser_page_text",
      "browser_click",
      "browser_type",
      "browser_key",
      "browser_scroll",
      "browser_screenshot",
      "browser_console",
      "browser_network",
      "browser_wait",
      "browser_eval",
    ]
  );
  for tool in BrowserTool::ALL {
    assert_eq!(BrowserTool::from_name(tool.as_str()), Some(tool));
    assert!(!tool.description().is_empty(), "{tool:?}");
  }
  assert_eq!(BrowserTool::from_name("browser_cookies"), None);
  let script: Vec<BrowserTool> = BrowserTool::ALL
    .into_iter()
    .filter(|tool| tool.reach() == BrowserReach::Script)
    .collect();
  assert_eq!(script, [BrowserTool::Eval], "only browser_eval runs JavaScript");
  assert!(
    BrowserTool::Eval
      .description()
      .contains("A person is asked before every call")
  );
}

#[test]
fn browser_catalog_sorts_tools_by_what_they_reach() {
  let reads = [
    BrowserTool::Tabs,
    BrowserTool::ReadPage,
    BrowserTool::PageText,
    BrowserTool::Screenshot,
    BrowserTool::Console,
    BrowserTool::Network,
    BrowserTool::Wait,
  ];
  for tool in reads {
    assert_eq!(tool.reach(), BrowserReach::Reads, "{tool:?}");
    assert!(tool.read_only(), "{tool:?}");
  }
  assert_eq!(
    BrowserTool::Close.reach(),
    BrowserReach::Reads,
    "closing the run's own tab changes no site"
  );
  for tool in [
    BrowserTool::Open,
    BrowserTool::Navigate,
    BrowserTool::Click,
    BrowserTool::Type,
    BrowserTool::Key,
    BrowserTool::Scroll,
  ] {
    assert_eq!(tool.reach(), BrowserReach::Site, "{tool:?}");
    assert!(!tool.read_only(), "{tool:?}");
  }
}

/// Valid arguments of `tool` that give every field its schema lists.
fn every_field(tool: BrowserTool) -> Value {
  match tool {
    BrowserTool::Tabs => json!({}),
    BrowserTool::Open => json!({"url": "https://docs.rs", "new_tab": true}),
    BrowserTool::Navigate => json!({"tab": "t1", "to": "back"}),
    BrowserTool::Close => json!({"tab": "t1"}),
    BrowserTool::ReadPage => json!({"tab": "t1", "query": "Save", "interactive_only": true, "max_chars": 100}),
    BrowserTool::PageText => json!({"tab": "t1", "max_chars": 100}),
    BrowserTool::Click => json!({"tab": "t1", "ref": "e1", "x": null, "y": null, "button": "right", "count": 2}),
    BrowserTool::Type => json!({"tab": "t1", "ref": "e2", "text": "8", "submit": true}),
    BrowserTool::Key => json!({"tab": "t1", "keys": ["Ctrl+A"]}),
    BrowserTool::Scroll => json!({"tab": "t1", "ref": null, "direction": "down", "pixels": 400}),
    BrowserTool::Screenshot => json!({"tab": "t1", "ref": "e3", "full_page": false}),
    BrowserTool::Console => json!({"tab": "t1", "errors_only": true, "pattern": "TypeError", "limit": 10}),
    BrowserTool::Network => json!({"tab": "t1", "url_pattern": "/api/", "limit": 10}),
    BrowserTool::Wait => json!({"tab": "t1", "text": "Saved", "ref": null, "timeout": 5}),
    BrowserTool::Eval => json!({"tab": "t1", "expression": "document.title"}),
  }
}

#[test]
fn browser_catalog_schemas_are_strict_and_list_what_calls_take() {
  for tool in BrowserTool::ALL {
    let schema = tool.input_schema();
    assert_eq!(schema["type"], "object", "{tool:?}");
    assert_eq!(schema["additionalProperties"], false, "{tool:?}");
    let properties: HashSet<&str> = schema["properties"]
      .as_object()
      .expect("properties")
      .keys()
      .map(String::as_str)
      .collect();
    for required in schema["required"].as_array().expect("required") {
      assert!(
        properties.contains(required.as_str().expect("a name")),
        "{tool:?}: {required}"
      );
    }
    let arguments = every_field(tool);
    let given: HashSet<&str> = arguments
      .as_object()
      .expect("an object")
      .keys()
      .map(String::as_str)
      .collect();
    assert_eq!(given, properties, "{tool:?}: the example gives every listed field");
    let call = BrowserCall::parse(tool.as_str(), &arguments).unwrap_or_else(|error| panic!("{tool:?}: {error}"));
    assert_eq!(call.tool(), tool);
    let mut extra = arguments.clone();
    extra["cookie"] = json!("x");
    assert!(
      matches!(
        BrowserCall::parse(tool.as_str(), &extra),
        Err(BrowserCallError::Invalid { .. })
      ),
      "{tool:?} refuses a field it does not list"
    );
  }
}
