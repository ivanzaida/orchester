use serde_json::json;

use crate::support::{MINUTE, PASSWORD, TestCloud, remember};

pub async fn sign_in_is_rate_limited_per_email_and_address() {
  let cloud = TestCloud::start_with(vec!["127.0.0.1".parse().expect("proxy")]).await;
  let email = "limit@example.test";
  cloud.register(email).await;
  for _ in 0..5 {
    assert_eq!(
      cloud
        .try_sign_in(email, "a wrong but long password", "Laptop")
        .await
        .status,
      401
    );
  }
  let blocked = cloud.try_sign_in(email, PASSWORD, "Laptop").await;
  assert_eq!((blocked.status, blocked.error()), (429, "rate_limited".into()));
  let retry: u64 = blocked.headers["retry-after"].parse().expect("Retry-After seconds");
  assert!((1..=15 * 60).contains(&retry));
  cloud.clock.advance(15 * MINUTE + 1000);
  assert_eq!(cloud.try_sign_in(email, PASSWORD, "Laptop").await.status, 201);

  // 20 failures from one address block it, whichever emails they named; another address is not affected.
  let from = |address: &'static str| [("X-Forwarded-For", address)];
  for attempt in 0..20 {
    let other = format!("guess{}@example.test", attempt % 4);
    remember(&other);
    let body = json!({"email": other, "password": "a wrong but long password",
      "device": {"name": "Laptop", "platform": "linux"}});
    let response = cloud
      .request_headers("POST", "/auth/sessions", None, Some(body), &from("203.0.113.7"))
      .await;
    assert_eq!(response.status, 401, "attempt {attempt}");
  }
  let body = json!({"email": email, "password": PASSWORD, "device": {"name": "Laptop", "platform": "linux"}});
  let blocked = cloud
    .request_headers("POST", "/auth/sessions", None, Some(body.clone()), &from("203.0.113.7"))
    .await;
  assert_eq!(blocked.status, 429);
  let other = cloud
    .request_headers("POST", "/auth/sessions", None, Some(body), &from("198.51.100.9"))
    .await;
  assert_eq!(other.status, 201);
  cloud.finish().await;
}

pub async fn emails_and_link_finishes_are_rate_limited() {
  let cloud = TestCloud::start().await;
  let email = "mailbox@example.test";
  cloud.register(email).await;
  // The sign-up sent one email to this address already: four more resets fit in the hour.
  for _ in 0..4 {
    let response = cloud
      .request("POST", "/auth/reset", None, Some(json!({"email": email})))
      .await;
    assert_eq!(response.status, 202);
  }
  let limited = cloud
    .request("POST", "/auth/reset", None, Some(json!({"email": email})))
    .await;
  assert_eq!((limited.status, limited.error()), (429, "rate_limited".into()));
  assert!(limited.headers.contains_key("retry-after"));
  cloud.clock.advance(60 * MINUTE + 1000);
  let later = cloud
    .request("POST", "/auth/reset", None, Some(json!({"email": email})))
    .await;
  assert_eq!(later.status, 202);

  // A different made-up link each time, so only the per-address limit counts them.
  let bogus = |index: u32| json!({"link_token": format!("{index:064x}"), "password": PASSWORD});
  for index in 0..20 {
    let response = cloud.request("POST", "/auth/confirm", None, Some(bogus(index))).await;
    assert_eq!(response.status, 422);
  }
  let blocked = cloud.request("POST", "/auth/confirm", None, Some(bogus(20))).await;
  assert_eq!(blocked.status, 429);
  cloud.finish().await;
}

/// Review 2, N1: wrong passwords on one confirmation link are limited per link, whatever address they come from.
pub async fn confirm_is_rate_limited_per_link() {
  let cloud = TestCloud::start_with(vec!["127.0.0.1".parse().expect("proxy")]).await;
  let email = "guarded@example.test";
  assert_eq!(cloud.sign_up(email, PASSWORD).await.status, 202);
  let token = cloud.mail.nth(email, 1).await.link_token();
  for attempt in 0..5 {
    let address = format!("198.51.100.{attempt}");
    let body = json!({"link_token": token, "password": "a guessed password"});
    let response = cloud
      .request_headers(
        "POST",
        "/auth/confirm",
        None,
        Some(body),
        &[("X-Forwarded-For", address.as_str())],
      )
      .await;
    assert_eq!(response.status, 422);
  }
  let blocked = cloud.confirm(&token, PASSWORD).await;
  assert_eq!((blocked.status, blocked.error()), (429, "rate_limited".into()));
  cloud.finish().await;
}

/// Review 3: guesses sent in parallel cannot exceed a link's 5; each reserves its place before the password check.
pub async fn parallel_confirm_guesses_stay_within_the_link_limit() {
  let cloud = TestCloud::start_with(vec!["127.0.0.1".parse().expect("proxy")]).await;
  let email = "parallel@example.test";
  assert_eq!(cloud.sign_up(email, PASSWORD).await.status, 202);
  let token = cloud.mail.nth(email, 1).await.link_token();
  let body = serde_json::to_vec(&json!({"link_token": token, "password": "a guessed password"})).expect("JSON");
  let mut running = tokio::task::JoinSet::new();
  for attempt in 0..10 {
    let (address, body) = (cloud.address(), body.clone());
    running.spawn(async move {
      let forwarded = format!("198.51.100.{attempt}");
      let headers = [
        ("Content-Type", "application/json"),
        ("X-Forwarded-For", forwarded.as_str()),
      ];
      crate::support::http::exchange(address, "POST", "/api/v1/auth/confirm", &headers, &body).await
    });
  }
  let mut checked = 0;
  while let Some(answer) = running.join_next().await {
    let status = answer.expect("the request task").status;
    assert!([422, 429].contains(&status), "answered {status}");
    checked += usize::from(status == 422);
  }
  assert!(checked <= 5, "{checked} guesses were checked; the link allows 5");
  cloud.finish().await;
}
