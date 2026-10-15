use std::time::Duration;

use orchester_api::cloud::LinkInspected;
use orchester_cloud::mail::EmailPurpose;
use serde_json::{Value, json};

use crate::support::{DAY, PASSWORD, TestCloud, postgres, remember};

async fn resend(cloud: &TestCloud, email: &str) -> crate::support::Response {
  remember(email);
  cloud
    .request("POST", "/auth/confirm/resend", None, Some(json!({"email": email})))
    .await
}

async fn inspect(cloud: &TestCloud, token: &str, purpose: &str) -> crate::support::Response {
  let body = json!({"link_token": token, "purpose": purpose});
  cloud.request("POST", "/auth/links/inspect", None, Some(body)).await
}

/// A resent link carries the sign-up's password (decision 65): it confirms with that password only, and confirming it
/// ends the first link too.
pub async fn a_resent_link_confirms_with_the_sign_up_password() {
  let cloud = TestCloud::start().await;
  let email = "resend@example.test";
  assert_eq!(cloud.sign_up(email, PASSWORD).await.status, 202);
  let first = cloud.mail.nth(email, 1).await.link_token();
  assert_eq!(resend(&cloud, email).await.status, 202);
  let sent = cloud.mail.nth(email, 2).await;
  assert_eq!(sent.purpose, EmailPurpose::Confirm);
  let second = sent.link_token();
  assert_ne!(first, second);
  let wrong = cloud.confirm(&second, "not the sign-up's password").await;
  assert_eq!((wrong.status, wrong.error()), (422, "invalid_link".into()));
  assert_eq!(cloud.confirm(&second, PASSWORD).await.status, 204);
  let ended = cloud.confirm(&first, PASSWORD).await;
  assert_eq!((ended.status, ended.error()), (422, "invalid_link".into()));
  assert_eq!(
    cloud.sign_in(email, PASSWORD, "Laptop").await.account.display_name,
    "Ada Lovelace"
  );
  cloud.finish().await;
}

/// The same answer for an unknown, an unconfirmed and a confirmed email; only the accounts get an email, the
/// confirmed one the *already have an account* note. An account whose links all expired gets nothing.
pub async fn resend_answers_alike_for_any_email() {
  let cloud = TestCloud::start().await;
  let (confirmed, unconfirmed, unknown) = ("done@example.test", "waiting@example.test", "nobody@example.test");
  cloud.register(confirmed).await;
  assert_eq!(cloud.sign_up(unconfirmed, PASSWORD).await.status, 202);
  cloud.mail.nth(unconfirmed, 1).await.link_token();
  let answers = [
    resend(&cloud, confirmed).await,
    resend(&cloud, unconfirmed).await,
    resend(&cloud, unknown).await,
  ];
  for answer in &answers {
    assert_eq!((answer.status, &answer.body), (answers[2].status, &answers[2].body));
  }
  assert_eq!(
    cloud.mail.nth(confirmed, 2).await.purpose,
    EmailPurpose::AlreadyRegistered
  );
  assert_eq!(cloud.mail.nth(unconfirmed, 2).await.purpose, EmailPurpose::Confirm);
  cloud.mail.nth(unconfirmed, 2).await.link_token();
  assert_eq!(cloud.mail.count(unknown), 0);
  let malformed = resend(&cloud, "not an email").await;
  assert_eq!((malformed.status, malformed.error()), (422, "invalid".into()));
  // Once every link has expired, no password is left to carry: nothing is sent.
  let late = "late@example.test";
  assert_eq!(cloud.sign_up(late, PASSWORD).await.status, 202);
  cloud.mail.nth(late, 1).await.link_token();
  cloud.clock.advance(DAY + 1000);
  assert_eq!(resend(&cloud, late).await.status, 202);
  tokio::time::sleep(Duration::from_millis(500)).await;
  assert_eq!(cloud.mail.count(late), 1);
  cloud.finish().await;
}

/// Resend answers before it touches the database (review 1, M5), and counts against the email limits: a sign-up and
/// four resends are the email's five an hour.
pub async fn resend_answers_before_the_database_and_is_limited() {
  let cloud = TestCloud::start().await;
  let email = "limited-resend@example.test";
  assert_eq!(cloud.sign_up(email, PASSWORD).await.status, 202);
  cloud.mail.nth(email, 1).await.link_token();
  let mut admin = postgres().database(&cloud.database).await;
  let lock = admin.transaction().await.expect("a locking transaction");
  lock
    .batch_execute("LOCK TABLE accounts, email_links IN ACCESS EXCLUSIVE MODE")
    .await
    .expect("lock the tables");
  let answer = tokio::time::timeout(Duration::from_secs(2), resend(&cloud, email))
    .await
    .expect("the answer does not wait for the database");
  assert_eq!(answer.status, 202);
  lock.commit().await.expect("release the tables");
  drop(admin);
  for _ in 0..3 {
    assert_eq!(resend(&cloud, email).await.status, 202);
  }
  let limited = resend(&cloud, email).await;
  assert_eq!((limited.status, limited.error()), (429, "rate_limited".into()));
  assert!(limited.headers.contains_key("retry-after"));
  for index in 2..=5 {
    cloud.mail.nth(email, index).await.link_token();
  }
  cloud.finish().await;
}

/// A live link names its email without being used; anything else is one `invalid_link`.
pub async fn inspect_names_the_email_without_using_the_link() {
  let cloud = TestCloud::start().await;
  let email = "inspect@example.test";
  assert_eq!(cloud.sign_up(email, PASSWORD).await.status, 202);
  let confirm = cloud.mail.nth(email, 1).await.link_token();
  let shown = inspect(&cloud, &confirm, "confirm").await;
  assert_eq!(shown.status, 200);
  assert_eq!(shown.json::<LinkInspected>().email, email);
  let other_purpose = inspect(&cloud, &confirm, "reset").await;
  assert_eq!(
    (other_purpose.status, other_purpose.error()),
    (422, "invalid_link".into())
  );
  assert_eq!(
    cloud.confirm(&confirm, PASSWORD).await.status,
    204,
    "inspecting did not use the link"
  );
  let used = inspect(&cloud, &confirm, "confirm").await;
  let unknown = inspect(&cloud, &"0".repeat(64), "confirm").await;
  let malformed = inspect(&cloud, "not-a-token", "confirm").await;
  for refused in [&used, &unknown, &malformed] {
    assert_eq!((refused.status, &refused.body), (422, &unknown.body));
  }
  assert_eq!(unknown.json::<Value>()["error"], "invalid_link");
  let reset = cloud
    .request("POST", "/auth/reset", None, Some(json!({"email": email})))
    .await;
  assert_eq!(reset.status, 202);
  let token = cloud.mail.nth(email, 2).await.link_token();
  assert_eq!(
    inspect(&cloud, &token, "reset").await.json::<LinkInspected>().email,
    email
  );
  let bad_purpose = inspect(&cloud, &token, "delete").await;
  assert_eq!((bad_purpose.status, bad_purpose.error()), (400, "bad_request".into()));
  cloud.finish().await;
}

/// Guessing links through inspect counts against the address like failed finishes: 20 in 15 minutes block it.
pub async fn inspect_failures_are_limited_per_address() {
  let cloud = TestCloud::start().await;
  for guess in 0..20 {
    let answer = inspect(&cloud, &format!("{guess:064x}"), "confirm").await;
    assert_eq!(answer.status, 422);
  }
  let blocked = inspect(&cloud, &"f".repeat(64), "confirm").await;
  assert_eq!((blocked.status, blocked.error()), (429, "rate_limited".into()));
  let finish = json!({"link_token": "e".repeat(64), "new_password": "a long enough new password"});
  remember("a long enough new password");
  let also = cloud.request("POST", "/auth/reset/finish", None, Some(finish)).await;
  assert_eq!(also.status, 429, "the address is blocked for finishes too");
  cloud.finish().await;
}
