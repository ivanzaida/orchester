//! The client uses no proxy, whatever `HTTP_PROXY`, `HTTPS_PROXY` and `ALL_PROXY` say: a plain
//! HTTP proxy would see the token. The proxy variables are set only for a child process running
//! this binary's [`client_proxy_signs_in_with_proxies_in_the_environment`], so they never reach
//! other tests; the parent checks that the proxy saw nothing while the server answered.
//!
//! `cargo test -p orchester-api --features client --test client_proxy client_proxy_`

mod support;

use std::{
  io::Read,
  net::TcpListener,
  process::Command,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  thread,
};

use support::TestServer;

/// Set for the child process only: it runs the client with the proxy variables.
const CHILD: &str = "ORCHESTER_CLIENT_PROXY_CHILD";

#[test]
fn client_proxy_variables_are_never_used() {
  let proxy = TcpListener::bind("127.0.0.1:0").expect("listen");
  let address = format!("http://{}", proxy.local_addr().expect("address"));
  let reached = Arc::new(AtomicUsize::new(0));
  let counted = reached.clone();
  thread::spawn(move || {
    for mut stream in proxy.incoming().flatten() {
      counted.fetch_add(1, Ordering::SeqCst);
      let mut request = [0_u8; 4096];
      // What the client sent does not matter: that it came is the failure.
      let _ = stream.read(&mut request);
    }
  });
  let output = Command::new(std::env::current_exe().expect("this test binary"))
    .args([
      "--exact",
      "client_proxy_signs_in_with_proxies_in_the_environment",
      "--nocapture",
    ])
    .env(CHILD, "1")
    .env("HTTP_PROXY", &address)
    .env("http_proxy", &address)
    .env("HTTPS_PROXY", &address)
    .env("https_proxy", &address)
    .env("ALL_PROXY", &address)
    .env("all_proxy", &address)
    .env_remove("NO_PROXY")
    .env_remove("no_proxy")
    .output()
    .expect("run the child");
  assert!(
    output.status.success(),
    "the child signed in directly:\n{}",
    String::from_utf8_lossy(&output.stdout)
  );
  assert!(
    String::from_utf8_lossy(&output.stdout).contains("1 passed"),
    "the child ran its test"
  );
  assert_eq!(reached.load(Ordering::SeqCst), 0, "nothing went to the proxy");
}

/// Runs only as the child of [`client_proxy_variables_are_never_used`].
#[tokio::test]
async fn client_proxy_signs_in_with_proxies_in_the_environment() {
  if std::env::var_os(CHILD).is_none() {
    return;
  }
  let server = TestServer::start(|_| {}).await;
  let admin = server.admin().await;
  let me = server.client().me(&admin).await.expect("the server answered directly");
  assert_eq!(me.username, "alice");
  server.finish().await;
}
