use std::{net::SocketAddr, path::PathBuf};

use orchester_api::client::{AddressError, Client, ClientError, PlainHttp, ServerAddress};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
};

use crate::support::TestServer;

fn address(text: &str) -> ServerAddress {
  ServerAddress::parse(text).expect("an address")
}

#[test]
fn client_addresses_are_normalized_and_know_this_computer() {
  assert_eq!(
    address("https://Orchester.Kontur.lan/").as_str(),
    "https://orchester.kontur.lan"
  );
  assert_eq!(address("orchester.kontur.lan").as_str(), "https://orchester.kontur.lan");
  assert_eq!(
    address("https://orchester.kontur.lan:443").as_str(),
    "https://orchester.kontur.lan"
  );
  assert_eq!(
    address("https://tools.example.lan/orchester/").as_str(),
    "https://tools.example.lan/orchester"
  );
  assert_eq!(address("http://10.0.4.20:8740").authority(), "10.0.4.20:8740");
  for local in [
    "http://localhost:8740",
    "http://127.0.0.1:8740",
    "http://127.4.5.6",
    "http://[::1]:8740",
  ] {
    let local = address(local);
    assert!(local.is_loopback(), "{local}");
    assert!(!local.needs_opt_in(), "{local}");
  }
  assert!(address("http://10.0.4.20:8740").needs_opt_in());
  assert!(address("http://localhost.kontur.lan").needs_opt_in());
  assert!(!address("https://10.0.4.20").needs_opt_in());
  assert_eq!(ServerAddress::parse("  "), Err(AddressError::Empty));
  assert_eq!(ServerAddress::parse("ftp://kontur.lan"), Err(AddressError::Scheme));
  assert_eq!(
    ServerAddress::parse("https://alice:pw@kontur.lan"),
    Err(AddressError::Extra)
  );
  assert_eq!(
    ServerAddress::parse("https://kontur.lan/?a=1"),
    Err(AddressError::Extra)
  );
}

#[test]
fn client_refuses_plain_http_to_another_computer_without_the_opt_in() {
  let lab = address("http://10.0.4.20:8740");
  assert!(matches!(
    Client::new(lab.clone(), PlainHttp::NotAllowed),
    Err(ClientError::PlainHttpRefused)
  ));
  assert!(Client::new(lab, PlainHttp::Allowed).is_ok());
  assert!(Client::new(address("http://localhost:8740"), PlainHttp::NotAllowed).is_ok());
}

#[tokio::test]
async fn client_reports_a_redirect_with_its_target_and_does_not_follow_it() {
  let redirect = canned(
    "HTTP/1.1 302 Found\r\nLocation: https://sso.kontur.lan/login\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
  )
  .await;
  let client = Client::new(
    address(&format!("http://127.0.0.1:{}", redirect.port())),
    PlainHttp::NotAllowed,
  )
  .expect("client");
  let error = client.server_info().await.expect_err("a redirect");
  assert_eq!(
    error,
    ClientError::Redirect {
      location: Some("https://sso.kontur.lan/login".into())
    }
  );
}

#[tokio::test]
async fn client_tells_what_is_not_an_orchester_server_and_what_cannot_be_reached() {
  let page = canned(
    "HTTP/1.1 404 Not Found\r\nContent-Type: text/html\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot here!",
  )
  .await;
  let client = Client::new(
    address(&format!("http://127.0.0.1:{}", page.port())),
    PlainHttp::NotAllowed,
  )
  .expect("client");
  assert_eq!(
    client.server_info().await.expect_err("not Orchester"),
    ClientError::NotOrchester { status: 404 }
  );

  let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("listen");
  let port = closed.local_addr().expect("address").port();
  drop(closed);
  let client = Client::new(address(&format!("http://127.0.0.1:{port}")), PlainHttp::NotAllowed).expect("client");
  let error = client.server_info().await.expect_err("nothing listens");
  assert!(error.is_connection(), "{error:?}");
}

#[tokio::test]
async fn client_refuses_a_certificate_the_system_does_not_trust() {
  let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../server/tests/fixtures");
  let server = TestServer::start(|config| {
    config.tls_cert = Some(fixtures.join("server.pem"));
    config.tls_key = Some(fixtures.join("server-key.pem"));
  })
  .await;
  // The test certificate authority is not in the operating system's store.
  let client = Client::new(
    address(&format!("https://localhost:{}", server.address.port())),
    PlainHttp::NotAllowed,
  )
  .expect("client");
  assert_eq!(
    client.server_info().await.expect_err("untrusted"),
    ClientError::UntrustedCertificate
  );
  server.finish().await;
}

/// Something at a loopback port that answers every request with `response`, then closes.
pub async fn canned(response: &'static str) -> SocketAddr {
  let listener = TcpListener::bind("127.0.0.1:0").await.expect("listen");
  let address = listener.local_addr().expect("address");
  tokio::spawn(async move {
    while let Ok((mut stream, _)) = listener.accept().await {
      let mut request = [0_u8; 4096];
      // What was asked does not matter; the answer is the same.
      let _ = stream.read(&mut request).await;
      let _ = stream.write_all(response.as_bytes()).await;
      let _ = stream.shutdown().await;
    }
  });
  address
}
