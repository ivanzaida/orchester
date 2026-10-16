//! N1 security re-review 2: the per-account caps hold past the account. A person who deletes their account and signs
//! up again with the same email still owns at most five linked servers, the ones they linked before counted; and the
//! servers they linked before send invites from one daily 100, not 100 each.

use orchester_cloud::clock::Clock;
use orchester_core::id::InviteId;
use serde_json::json;

use crate::{
  registry::{TeamServer, deliver, link, signed_in},
  support::{PASSWORD, TestCloud},
};

pub async fn a_person_keeps_their_caps_past_deleting_the_account() {
  let cloud = TestCloud::start_with(vec!["127.0.0.1".parse().expect("proxy")]).await;
  let email = "ivan@example.test";
  let first = signed_in(&cloud, email).await;
  let servers: Vec<TeamServer> = (0..6).map(|_| TeamServer::generate()).collect();
  for server in &servers[..5] {
    link(&cloud, &first, server).await;
  }
  let deleted = cloud
    .request("DELETE", "/me", Some(&first), Some(json!({"password": PASSWORD})))
    .await;
  assert_eq!(deleted.status, 204);

  let again = signed_in(&cloud, email).await;
  let sixth = json!({"link": servers[5].statement(cloud.clock.now_ms())});
  let refused = cloud.request("POST", "/servers", Some(&again), Some(sixth)).await;
  assert_eq!(
    (refused.status, refused.error()),
    (409, "too_many_servers".into()),
    "the five servers linked before still count"
  );

  let now = cloud.clock.now_ms();
  for index in 0..100 {
    let invite = servers[index % 5].invite(InviteId::generate(), &format!("person{index}@example.test"), now);
    let from = format!("198.51.100.{}", index / 50);
    assert_eq!(deliver(&cloud, &invite, &from).await.status, 202, "invite {index}");
  }
  let over = servers[0].invite(InviteId::generate(), "one-more@example.test", now);
  let limited = deliver(&cloud, &over, "198.51.100.9").await;
  assert_eq!(
    (limited.status, limited.error()),
    (429, "rate_limited".into()),
    "one daily 100 for the person's servers"
  );
  cloud.finish().await;
}
