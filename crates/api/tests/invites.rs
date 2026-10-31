//! The answers of making and revoking an invite (`docs/architecture/server.md`, "Invites"): how the team server's own
//! delivery to Orchester Cloud went, read and written back unchanged, and older answers without it still read.
//!
//! `cargo test -p orchester-api --test invites invites_`

use orchester_api::{CloudCause, CloudDelivery, InviteCreated, InviteRevoked};
use serde_json::{Value, json};

const INVITE: &str = "01a183cf-b73c-71db-8a62-bd509770cd4d";
const USER: &str = "01a183cf-b73c-71db-8a62-bd509770cd4e";

fn invite() -> Value {
  json!({
    "id": INVITE, "kind": "server", "email": "yuri@example.com", "project": null, "role": null, "user": null,
    "admin": false, "by": { "id": USER, "display_name": "Lurq" }, "delivery": "cloud", "expires_at": 20,
    "created_at": 10, "state": "open", "ended_at": null, "ended_by": null
  })
}

fn deliveries() -> Vec<Value> {
  vec![
    json!({ "kind": "delivered" }),
    json!({ "kind": "declined" }),
    json!({ "kind": "no_cloud" }),
    json!({ "kind": "unreachable", "cause": "unreachable" }),
    json!({ "kind": "unreachable", "cause": "tls" }),
    json!({ "kind": "unreachable", "cause": "refused" }),
    json!({ "kind": "unreachable", "cause": "timeout" }),
  ]
}

#[test]
fn invites_cloud_delivery_reads_and_writes_back_unchanged() {
  for delivery in deliveries() {
    let read: CloudDelivery =
      serde_json::from_value(delivery.clone()).unwrap_or_else(|error| panic!("{delivery}: {error}"));
    assert_eq!(serde_json::to_value(read).expect("serialize"), delivery);
  }
  assert_eq!(
    serde_json::from_value::<CloudDelivery>(json!({ "kind": "unreachable", "cause": "tls" })).expect("a delivery"),
    CloudDelivery::Unreachable { cause: CloudCause::Tls }
  );
}

#[test]
fn invites_created_and_revoked_carry_the_delivery() {
  let created = json!({ "invite": invite(), "code": null, "signed": "signed-invite",
    "cloud_delivery": { "kind": "unreachable", "cause": "timeout" } });
  let read: InviteCreated = serde_json::from_value(created).expect("an invite created");
  assert_eq!(
    read.cloud_delivery,
    Some(CloudDelivery::Unreachable {
      cause: CloudCause::Timeout
    })
  );
  assert_eq!(read.signed.as_deref(), Some("signed-invite"));

  let revoked = json!({ "revocation": "signed-revocation", "cloud_delivery": { "kind": "delivered" } });
  let read: InviteRevoked = serde_json::from_value(revoked.clone()).expect("an invite revoked");
  assert_eq!(read.cloud_delivery, Some(CloudDelivery::Delivered));
  assert_eq!(serde_json::to_value(&read).expect("serialize"), revoked);
}

#[test]
fn invites_answers_without_a_delivery_still_read() {
  let created = json!({ "invite": invite(), "code": null, "signed": "signed-invite" });
  let read: InviteCreated = serde_json::from_value(created).expect("an older invite created");
  assert_eq!(read.cloud_delivery, None);

  let revoked = json!({ "revocation": null });
  let read: InviteRevoked = serde_json::from_value(revoked).expect("an older invite revoked");
  assert_eq!(read.cloud_delivery, None);
}

#[test]
fn invites_unknown_delivery_kinds_and_causes_are_refused() {
  for wrong in [
    json!({ "kind": "sent" }),
    json!({ "kind": "unreachable" }),
    json!({ "kind": "unreachable", "cause": "dns" }),
  ] {
    assert!(
      serde_json::from_value::<CloudDelivery>(wrong.clone()).is_err(),
      "{wrong}"
    );
  }
}
