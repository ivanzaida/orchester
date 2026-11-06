use orchester_api::{
  Event, ResourceCreate, ResourceCreated, Resources, SharedResourceKind, SharedVersion, VersionContent, VersionPublish,
  Versions,
};
use orchester_core::{id::ResourceId, resource::TeamMcpServer};
use serde_json::{Value, json};

use crate::fixtures::{key, resource, round_trip, user, version};

fn bodies() -> [Value; 2] {
  [
    json!({
      "name":"project-knowledge",
      "launch":{"kind":"program","program":"npx","args":["-y","knowledge-mcp"],
                "working_directory":"tools","variables":["KNOWLEDGE_TOKEN"]},
      "exempt_tools":["search"]
    }),
    json!({
      "name":"issues",
      "launch":{"kind":"remote","url":"https://mcp.example.org/issues","secret_headers":["X-API-Key"]}
    }),
  ]
}

#[test]
fn mcp_server_kind_round_trips_on_the_wire() {
  assert_eq!(
    serde_json::to_value(SharedResourceKind::McpServer).expect("kind"),
    json!("mcp_server")
  );
  let id = ResourceId::generate();
  for body in bodies() {
    round_trip::<TeamMcpServer>(&body);
    let content = json!({"body":body,"references":[]});
    round_trip::<VersionContent>(&content);
    let create: ResourceCreate = round_trip(&json!({
      "idempotency_key":key(),"id":id,"kind":"mcp_server","slug":"issues","name":"Issues","version":content
    }));
    assert_eq!(create.kind, SharedResourceKind::McpServer);
    round_trip::<VersionPublish>(&json!({"idempotency_key":key(),"based_on":1,"body":body,"references":[]}));
    let mut published = version(id, 2);
    published["body"] = body;
    published["references"] = json!([]);
    round_trip::<SharedVersion>(&published);
    round_trip::<ResourceCreated>(&json!({"resource":resource(id,"mcp_server"),"version":published}));
    round_trip::<Versions>(&json!({"versions":[published]}));
  }
  round_trip::<Resources>(&json!({"resources":[resource(id,"mcp_server")],"cursor":9}));
}

#[test]
fn mcp_server_uses_existing_resource_deltas() {
  let id = ResourceId::generate();
  for (kind, change) in [
    ("resource.published", json!({"resource":resource(id,"mcp_server")})),
    ("resource.renamed", json!({"resource":id,"name":"Renamed server"})),
    ("resource.archived", json!({"resource":id,"archived":true})),
  ] {
    let mut data = json!({"id":42,"by":user(),"actor":"person","at":3});
    data
      .as_object_mut()
      .expect("delta")
      .extend(change.as_object().expect("change").clone());
    round_trip::<Event>(&json!({"kind":kind,"data":data}));
  }
}
