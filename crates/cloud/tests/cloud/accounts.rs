use std::time::Duration;

use orchester_api::cloud::{AccountResponse, CloudPlatform, DevicesResponse};
use orchester_cloud::mail::EmailPurpose;
use serde_json::json;

use crate::support::{BASE_URL, DAY, HOUR, PASSWORD, TestCloud, postgres, remember};

pub async fn sign_up_confirm_sign_in() {
  let cloud = TestCloud::start().await;
  let email = "ada@example.test";
  assert_eq!(cloud.sign_up(" Ada@Example.TEST ", PASSWORD).await.status, 202);
  let sent = cloud.mail.nth(email, 1).await;
  assert_eq!(sent.purpose, EmailPurpose::Confirm);
  assert_eq!(sent.subject, "Confirm your Orchester account");
  assert!(sent.body.contains(&format!("{BASE_URL}/confirm/")));
  let token = sent.link_token();
  assert_eq!(cloud.confirm(&token, PASSWORD).await.status, 204);
  let again = cloud.confirm(&token, PASSWORD).await;
  assert_eq!((again.status, again.error()), (422, "invalid_link".into()));
  let signed = cloud.sign_in(email, PASSWORD, "Ada's laptop").await;
  assert!(signed.token.expose().starts_with("orcc_") && signed.token.expose().len() == 69);
  assert_eq!(signed.account.email, email);
  assert_eq!(signed.account.display_name, "Ada Lovelace");
  assert!(signed.device.current && signed.device.platform == CloudPlatform::Windows);
  let token = signed.token.expose();
  let me = cloud.request("GET", "/me", Some(token), None).await;
  assert_eq!(me.status, 200);
  assert_eq!(me.json::<AccountResponse>().account.id, signed.account.id);
  let devices = cloud
    .request("GET", "/devices", Some(token), None)
    .await
    .json::<DevicesResponse>();
  assert_eq!(devices.devices.len(), 1);
  assert_eq!(devices.devices[0].name, "Ada's laptop");
  cloud.finish().await;
}

pub async fn unconfirmed_sign_in_is_refused() {
  let cloud = TestCloud::start().await;
  let email = "grace@example.test";
  assert_eq!(cloud.sign_up(email, PASSWORD).await.status, 202);
  let token = cloud.mail.nth(email, 1).await.link_token();
  let right = cloud.try_sign_in(email, PASSWORD, "Laptop").await;
  assert_eq!((right.status, right.error()), (403, "email_unconfirmed".into()));
  let wrong = cloud.try_sign_in(email, "not the password at all", "Laptop").await;
  assert_eq!((wrong.status, wrong.error()), (401, "invalid_credentials".into()));
  // A day later the link has expired and the sweep deletes the account that never confirmed.
  cloud.clock.advance(DAY + 1000);
  cloud.cloud.sweep().await.expect("sweep");
  let late = cloud.confirm(&token, PASSWORD).await;
  assert_eq!((late.status, late.error()), (422, "invalid_link".into()));
  let gone = cloud.try_sign_in(email, PASSWORD, "Laptop").await;
  assert_eq!((gone.status, gone.error()), (401, "invalid_credentials".into()));
  cloud.finish().await;
}

pub async fn wrong_password_and_unknown_email_answer_alike() {
  let cloud = TestCloud::start().await;
  let email = "alan@example.test";
  cloud.register(email).await;
  let wrong = cloud.try_sign_in(email, "a wrong but long password", "Laptop").await;
  let unknown = cloud
    .try_sign_in("nobody@example.test", "a wrong but long password", "Laptop")
    .await;
  assert_eq!(wrong.status, 401);
  assert_eq!((wrong.status, &wrong.body), (unknown.status, &unknown.body));
  let malformed = cloud
    .try_sign_in("not an email", "a wrong but long password", "Laptop")
    .await;
  assert_eq!((malformed.status, &malformed.body), (unknown.status, &unknown.body));
  repeated_sign_ups_change_nothing(&cloud, email).await;
  resets_answer_alike(&cloud, email).await;
  cloud.finish().await;
}

/// Signing up with a registered email answers as a new one does and changes nothing but sending an email.
async fn repeated_sign_ups_change_nothing(cloud: &TestCloud, email: &str) {
  let again = cloud.sign_up(email, "another password of mine").await;
  let fresh = cloud.sign_up("new@example.test", "another password of mine").await;
  assert_eq!((again.status, &again.body), (fresh.status, &fresh.body));
  assert_eq!(cloud.mail.nth(email, 2).await.purpose, EmailPurpose::AlreadyRegistered);
  let changed = cloud.try_sign_in(email, "another password of mine", "Laptop").await;
  assert_eq!(changed.status, 401);
  assert_eq!(cloud.try_sign_in(email, PASSWORD, "Laptop").await.status, 201);
}

/// Review 2, N1: confirming asks for the password of the sign-up the link was sent for. Someone who signed up with
/// another person's email cannot have their link confirmed with the owner's password, and their sign-up does not
/// end the owner's own link; a wrong password on the owner's link fails too.
pub async fn an_earlier_sign_up_cannot_take_the_account() {
  let cloud = TestCloud::start().await;
  let email = "victim@example.test";
  let (owner, attacker) = ("the owner's own password", "the attacker's password");
  assert_eq!(cloud.sign_up(email, owner).await.status, 202);
  let owner_link = cloud.mail.nth(email, 1).await.link_token();
  assert_eq!(cloud.sign_up(email, attacker).await.status, 202);
  let attacker_link = cloud.mail.nth(email, 2).await.link_token();
  let crossed = cloud.confirm(&attacker_link, owner).await;
  assert_eq!((crossed.status, crossed.error()), (422, "invalid_link".into()));
  let wrong = cloud.confirm(&owner_link, "not the password of this sign-up").await;
  assert_eq!((wrong.status, wrong.error()), (422, "invalid_link".into()));
  assert_eq!(
    cloud.confirm(&owner_link, owner).await.status,
    204,
    "the owner's link outlives the later sign-up"
  );
  let ended = cloud.confirm(&attacker_link, attacker).await;
  assert_eq!(
    (ended.status, ended.error()),
    (422, "invalid_link".into()),
    "confirming ends the other links"
  );
  let refused = cloud.try_sign_in(email, attacker, "Laptop").await;
  assert_eq!((refused.status, refused.error()), (401, "invalid_credentials".into()));
  assert_eq!(cloud.sign_in(email, owner, "Laptop").await.account.email, email);
  cloud.finish().await;
}

/// Review 2, N3: a confirmation, a repeated sign-up and a reset of one account at the same time never fail with a
/// server error (they lock the account row before its links).
pub async fn concurrent_confirm_sign_up_and_reset_never_fail() {
  let cloud = TestCloud::start().await;
  for round in 0..12 {
    // Each round sends three emails from one address; an hour apart, the email limits never stop a round.
    cloud.clock.advance(HOUR);
    let email = format!("race{round}@example.test");
    assert_eq!(cloud.sign_up(&email, PASSWORD).await.status, 202);
    let confirm = cloud.mail.nth(&email, 1).await.link_token();
    assert_eq!(
      cloud
        .request("POST", "/auth/reset", None, Some(json!({"email": email})))
        .await
        .status,
      202
    );
    let reset = cloud.mail.nth(&email, 2).await.link_token();
    let finish = json!({"link_token": reset, "new_password": "a password from the reset"});
    let (confirmed, signed_up, finished) = tokio::join!(
      cloud.confirm(&confirm, PASSWORD),
      cloud.sign_up(&email, "another sign-up password"),
      cloud.request("POST", "/auth/reset/finish", None, Some(finish)),
    );
    for (what, status) in [
      ("confirm", confirmed.status),
      ("sign-up", signed_up.status),
      ("reset", finished.status),
    ] {
      assert!(
        [202, 204, 422].contains(&status),
        "round {round}: {what} answered {status}"
      );
    }
  }
  cloud.finish().await;
}

/// A reset request answers alike for any email and mails only an account.
async fn resets_answer_alike(cloud: &TestCloud, email: &str) {
  remember("nobody@example.test");
  let known = cloud
    .request("POST", "/auth/reset", None, Some(json!({"email": email})))
    .await;
  let stranger = cloud
    .request(
      "POST",
      "/auth/reset",
      None,
      Some(json!({"email": "nobody@example.test"})),
    )
    .await;
  assert_eq!((known.status, &known.body), (stranger.status, &stranger.body));
  assert_eq!(cloud.mail.nth(email, 3).await.purpose, EmailPurpose::Reset);
  assert_eq!(cloud.mail.count("nobody@example.test"), 0);
}

/// Review 1, M5: a reset request answers before it touches the database, so how long it takes says nothing about
/// whether the email has an account. With the tables locked, both answers still come at once.
pub async fn a_reset_request_answers_before_any_database_work() {
  let cloud = TestCloud::start().await;
  let email = "timing@example.test";
  cloud.register(email).await;
  remember("nobody-timing@example.test");
  let mut admin = postgres().database(&cloud.database).await;
  let lock = admin.transaction().await.expect("a locking transaction");
  lock
    .batch_execute("LOCK TABLE accounts, email_links IN ACCESS EXCLUSIVE MODE")
    .await
    .expect("lock the tables");
  for address in [email, "nobody-timing@example.test"] {
    let request = cloud.request("POST", "/auth/reset", None, Some(json!({"email": address})));
    let answer = tokio::time::timeout(Duration::from_secs(2), request)
      .await
      .expect("the answer does not wait for the database");
    assert_eq!(answer.status, 202);
  }
  lock.commit().await.expect("release the tables");
  assert_eq!(cloud.mail.nth(email, 2).await.purpose, EmailPurpose::Reset);
  drop(admin);
  cloud.finish().await;
}

/// Review 2, N3: the order of row locks, made deterministic. Another transaction holds the account row (as a sign-up
/// or reset does) and then locks the account's links; a confirmation in between must wait for the account row, not
/// hold a link row, or the two deadlock.
pub async fn confirm_locks_the_account_before_its_links() {
  let cloud = TestCloud::start().await;
  let email = "order@example.test";
  assert_eq!(cloud.sign_up(email, PASSWORD).await.status, 202);
  let token = cloud.mail.nth(email, 1).await.link_token();
  let mut admin = postgres().database(&cloud.database).await;
  let other = admin.transaction().await.expect("the other transaction");
  other
    .execute("SELECT 1 FROM accounts WHERE email = $1 FOR UPDATE", &[&email])
    .await
    .expect("the account row");
  let then_links = async move {
    tokio::time::sleep(Duration::from_secs(1)).await;
    let links = other
      .execute(
        "SELECT 1 FROM email_links l JOIN accounts a ON a.id = l.account WHERE a.email = $1 FOR UPDATE OF l",
        &[&email],
      )
      .await;
    let committed = other.commit().await;
    links.and(committed)
  };
  let (confirmed, locked) = tokio::join!(cloud.confirm(&token, PASSWORD), then_links);
  assert!(locked.is_ok(), "the other transaction met a deadlock");
  assert_eq!(confirmed.status, 204);
  drop(admin);
  cloud.finish().await;
}
