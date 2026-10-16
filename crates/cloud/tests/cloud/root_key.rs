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

/// A database from before the cloud kept its own root holds key sets an offline root signed and no root: the cloud
/// makes its root, drops those key sets and serves its own.
pub async fn a_database_from_an_offline_root_gets_its_own() {
  let database = postgres().create_database().await;
  let key = key_encryption_key();
  drop(open(&database, &key).await.expect("the first start"));
  let admin = postgres().database(&database).await;
  admin
    .batch_execute(
      "DELETE FROM root_key;
       INSERT INTO key_sets (issued_at, expires_at, compact, published_at)
       VALUES (now() + interval '1 year', now() + interval '2 years', 'signed.by.another-root', now());",
    )
    .await
    .expect("a database of the offline ceremony");
  drop(open(&database, &key).await.expect("a start on it"));
  let root = stored_root(&admin).await.expect("a root of its own");
  let compact: String = admin
    .query_one("SELECT compact FROM key_sets ORDER BY issued_at DESC LIMIT 1", &[])
    .await
    .expect("a key set")
    .get(0);
  VerifiedKeySet::verify(&compact, &root, START / 1000).expect("the newest key set is the new root's");
  drop(admin);
  postgres().drop_database(&database).await;
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
