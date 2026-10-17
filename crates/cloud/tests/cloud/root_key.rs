//! The cloud's own root key: made at the first start and sealed in the database, reused at every later start, made
//! once by replicas that start together, never replaced when the key-encryption key does not open it, and the root
//! whose key sets `GET /keys` serves and linked team servers pin.

use std::sync::Arc;

use orchester_api::{
  cloud::KeySetResponse,
  identity::{PublicKey, VerifiedKeySet},
};
use orchester_cloud::{Cloud, clock::Clock, error::CloudError};

use crate::{
  registry::{TeamServer, link, signed_in},
  support::{BASE_URL, RecordingMailer, TestClock, TestCloud, config, key_encryption_key, postgres, stored_root},
};

const START: i64 = 1_791_000_000_000;

async fn open(database: &str, key: &str) -> Result<Cloud, CloudError> {
  Cloud::open(
    config(database, key, Vec::new()),
    Arc::new(TestClock::at(START)),
    Arc::new(RecordingMailer::default()),
  )
  .await
}

/// How many roots, signing keys and key sets the database `admin` is connected to holds.
async fn counts(admin: &tokio_postgres::Client) -> (i64, i64, i64) {
  let row = admin
    .query_one(
      "SELECT (SELECT count(*) FROM root_key), (SELECT count(*) FROM signing_keys), (SELECT count(*) FROM key_sets)",
      &[],
    )
    .await
    .expect("counts");
  (row.get(0), row.get(1), row.get(2))
}

pub async fn the_first_start_makes_the_root() {
  let database = postgres().create_database().await;
  open(&database, &key_encryption_key()).await.expect("the first start");
  let admin = postgres().database(&database).await;
  let root = stored_root(&admin).await.expect("the first start made a root");
  let sealed: Vec<u8> = admin
    .query_one("SELECT sealed_private_key FROM root_key", &[])
    .await
    .expect("the sealed root")
    .get(0);
  // ring's PKCS#8 document carries the public key: a sealed one does not show it.
  assert!(
    !sealed.windows(32).any(|window| window == root.as_bytes()),
    "the root's private half is sealed"
  );
  let audited: i64 = admin
    .query_one(
      "SELECT count(*) FROM audit_events WHERE action = 'root_key_created' AND target = $1",
      &[&root.fingerprint()],
    )
    .await
    .expect("audit")
    .get(0);
  assert_eq!(audited, 1);
  assert_eq!(counts(&admin).await, (1, 1, 1));
  drop(admin);
  postgres().drop_database(&database).await;
}

pub async fn a_restart_reuses_the_root() {
  let database = postgres().create_database().await;
  let key = key_encryption_key();
  drop(open(&database, &key).await.expect("the first start"));
  let admin = postgres().database(&database).await;
  let first = stored_root(&admin).await.expect("a root");
  drop(open(&database, &key).await.expect("a restart"));
  assert_eq!(stored_root(&admin).await, Some(first));
  assert_eq!(
    counts(&admin).await,
    (1, 1, 1),
    "a restart within the week neither rotates nor signs again"
  );
  drop(admin);
  postgres().drop_database(&database).await;
}

pub async fn replicas_starting_together_make_one_root() {
  let database = postgres().create_database().await;
  let key = key_encryption_key();
  let (first, second, third) = tokio::join!(open(&database, &key), open(&database, &key), open(&database, &key));
  assert!(first.is_ok() && second.is_ok() && third.is_ok(), "every replica starts");
  drop((first, second, third));
  let admin = postgres().database(&database).await;
  assert_eq!(
    counts(&admin).await,
    (1, 1, 1),
    "one root, one signing key and one key set, made by whichever replica came first"
  );
  drop(admin);
  postgres().drop_database(&database).await;
}

pub async fn a_changed_key_encryption_key_stops_the_start() {
  let database = postgres().create_database().await;
  drop(open(&database, &key_encryption_key()).await.expect("the first start"));
  let admin = postgres().database(&database).await;
  let root = stored_root(&admin).await.expect("a root");
  let refused = open(&database, &key_encryption_key()).await;
  assert!(
    matches!(refused, Err(CloudError::WrongKeyEncryptionKey)),
    "another key-encryption key is refused, got {:?}",
    refused.map(|_| ())
  );
  assert_eq!(
    stored_root(&admin).await,
    Some(root),
    "the cloud never makes a new root in its place"
  );
  drop(admin);
  postgres().drop_database(&database).await;
}

/// Signing keys or key sets with no root (the root row deleted alone) stop the start, whatever the key-encryption key:
/// the cloud never makes a root beside keys another root vouched for.
pub async fn keys_without_a_root_stop_the_start() {
  let database = postgres().create_database().await;
  let key = key_encryption_key();
  drop(open(&database, &key).await.expect("the first start"));
  let admin = postgres().database(&database).await;
  admin
    .batch_execute("DELETE FROM root_key")
    .await
    .expect("the root row deleted alone");
  for key in [key, key_encryption_key()] {
    let refused = open(&database, &key).await;
    assert!(
      matches!(refused, Err(CloudError::KeysWithoutRoot)),
      "keys without a root are refused, got {:?}",
      refused.map(|_| ())
    );
  }
  assert_eq!(counts(&admin).await, (0, 1, 1), "no root is made beside the keys");
  drop(admin);
  postgres().drop_database(&database).await;
}

/// The procedure for a key-encryption key lost for good (docs/cloud-hosting.md): emptying the root, the signing keys
/// and the key sets together lets a new key start the cloud with a new root.
pub async fn losing_the_key_encryption_key_starts_over_with_a_new_root() {
  let database = postgres().create_database().await;
  drop(open(&database, &key_encryption_key()).await.expect("the first start"));
  let admin = postgres().database(&database).await;
  let lost = stored_root(&admin).await.expect("a root");
  admin
    .batch_execute("DELETE FROM key_sets; DELETE FROM signing_keys; DELETE FROM root_key;")
    .await
    .expect("the documented procedure");
  drop(
    open(&database, &key_encryption_key())
      .await
      .expect("a start with a new key"),
  );
  let root = stored_root(&admin).await.expect("a new root");
  assert_ne!(root, lost);
  assert_eq!(counts(&admin).await, (1, 1, 1));
  let compact: String = admin
    .query_one("SELECT compact FROM key_sets", &[])
    .await
    .expect("a key set")
    .get(0);
  VerifiedKeySet::verify(&compact, &root, START / 1000).expect("the key set is the new root's");
  drop(admin);
  postgres().drop_database(&database).await;
}

/// A root row that is not a sealed key is stored data gone bad, not a wrong key-encryption key.
pub async fn a_corrupt_root_is_stored_data() {
  let database = postgres().create_database().await;
  let key = key_encryption_key();
  drop(open(&database, &key).await.expect("the first start"));
  let admin = postgres().database(&database).await;
  admin
    .batch_execute("UPDATE root_key SET sealed_private_key = '\\x00'")
    .await
    .expect("a corrupt root");
  let refused = open(&database, &key).await;
  assert!(
    matches!(refused, Err(CloudError::StoredData)),
    "a corrupt root is stored data, got {:?}",
    refused.map(|_| ())
  );
  drop(admin);
  postgres().drop_database(&database).await;
}

/// A key set dated in the future (a clock set back, a row written by hand) is neither served nor taken as the newest,
/// so the cloud does not sign again every minute.
pub async fn a_key_set_dated_in_the_future_is_ignored() {
  let cloud = TestCloud::start().await;
  let admin = postgres().database(&cloud.database).await;
  let later = cloud.clock.now_ms() / 1000 + 365 * 24 * 60 * 60;
  admin
    .execute(
      "INSERT INTO key_sets (issued_at, expires_at, compact, published_at)
       VALUES (to_timestamp($1::bigint), to_timestamp($1::bigint + 86400), 'dated.in.the-future', now())",
      &[&later],
    )
    .await
    .expect("a key set dated a year ahead");
  cloud.cloud.keep_keys().await.expect("keep the keys");
  cloud.cloud.keep_keys().await.expect("keep the keys");
  assert_eq!(counts(&admin).await, (1, 1, 2), "nothing is signed again");
  let response = cloud.request("GET", "/keys", None, None).await;
  assert_eq!(response.status, 200);
  let served = response.json::<KeySetResponse>().key_set;
  VerifiedKeySet::verify(&served, &cloud.root().await, cloud.clock.now_ms() / 1000).expect("the served set is current");
  drop(admin);
  cloud.finish().await;
}

pub async fn keys_verify_with_the_root_a_linked_server_pins() {
  let cloud = TestCloud::start().await;
  let now = cloud.clock.now_ms() / 1000;
  let response = cloud.request("GET", "/keys", None, None).await;
  assert_eq!(response.status, 200, "a new cloud serves a key set at once");
  let served = response.json::<KeySetResponse>().key_set;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let linked = link(&cloud, &owner, &TeamServer::generate()).await;
  let pinned = PublicKey::parse(&linked.root_key).expect("the root a server pins");
  assert_eq!(pinned, cloud.root().await);
  let set = VerifiedKeySet::verify(&served, &pinned, now).expect("the served key set verifies with the pinned root");
  assert_eq!(set.statement().iss, BASE_URL);
  assert_eq!(linked.key_set, served);
  cloud.finish().await;
}
