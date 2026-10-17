//! A team server that gives itself another server's id (`docs/architecture/server.md`, "Codes"), over TLS with a
//! certificate of its own: `GET /server` names that id, and `POST /server/key-proof` answers with a key it holds, whose
//! id is not that one; with the other server's public key, which it cannot sign with; or with the other server's own
//! proof for the same nonce, passed on from it, which names the other server's certificate. It records the paths it
//! was asked, so a test can tell that no code reached it.

use std::{
  io::{Read, Write},
  net::{TcpListener, TcpStream},
  sync::Arc,
  thread,
};

use orchester_api::{
  client::Client,
  identity::{PrivateKey, SigningKey},
  statements::{KEY_PROOF_TYPE, KeyProof},
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio_rustls::rustls::{
  ServerConfig, ServerConnection, StreamOwned,
  crypto::ring,
  pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};

const CERTIFICATE: &[u8] = include_bytes!("../../../server/tests/fixtures/server.pem");
const KEY: &[u8] = include_bytes!("../../../server/tests/fixtures/server-key.pem");

/// What the impostor answers a key proof with.
pub enum Claimed {
  /// A key of its own, whose id is not the one it gives itself.
  OwnKey,
  /// The other server's public key (base64url), which it cannot sign with.
  Borrowed(String),
  /// The other server's own answer for the same nonce, asked through `Client`.
  Relayed(Client),
}

pub struct Impostor {
  port: u16,
  asked: Arc<Mutex<Vec<String>>>,
}

impl Impostor {
  /// An impostor of the server `id`, answering key proofs as `claimed` says.
  pub fn start(id: &str, claimed: Claimed) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind the impostor");
    let port = listener.local_addr().expect("address").port();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let tls = Arc::new(tls_config());
    let (id, seen) = (id.to_owned(), asked.clone());
    let answers = Answers::new(claimed);
    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        let connection = ServerConnection::new(tls.clone()).expect("a TLS connection");
        answer(StreamOwned::new(connection, stream), (&id, &answers), &seen);
      }
    });
    Self { port, asked }
  }

  pub fn url(&self) -> String {
    format!("https://localhost:{}", self.port)
  }

  /// The requests it was asked, as `METHOD /path`.
  pub fn asked(&self) -> Vec<String> {
    self.asked.lock().clone()
  }
}

/// The impostor's own key, and what it claims.
struct Answers {
  key: SigningKey,
  claimed: Claimed,
}

impl Answers {
  fn new(claimed: Claimed) -> Self {
    let key = SigningKey::new(&PrivateKey::generate().expect("a key")).expect("a signing key");
    Self { key, claimed }
  }

  fn key_proof(&self, id: &str, nonce: &str) -> Value {
    if let Claimed::Relayed(client) = &self.claimed {
      let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
      let theirs = runtime
        .block_on(client.key_proof(nonce))
        .expect("the other server's proof");
      return json!({"public_key": theirs.public_key, "proof": theirs.proof});
    }
    let public_key = match &self.claimed {
      Claimed::Borrowed(public_key) => public_key.clone(),
      _ => self.key.public_key().encoded(),
    };
    let proof = KeyProof {
      server: id.parse().expect("a server id"),
      nonce: nonce.to_owned(),
      certificate: Some(orchester_api::certificate::CertificatePin::of(&certificate()).to_hex()),
      previous_certificate: None,
      issued_at: 0,
    };
    let signed = self.key.sign_statement(KEY_PROOF_TYPE, &proof).expect("signed");
    json!({"public_key": public_key, "proof": signed})
  }
}

fn certificate() -> CertificateDer<'static> {
  CertificateDer::from_pem_slice(CERTIFICATE).expect("the fixture certificate")
}

fn tls_config() -> ServerConfig {
  let key = PrivateKeyDer::from_pem_slice(KEY).expect("the fixture key");
  ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
    .with_safe_default_protocol_versions()
    .expect("protocol versions")
    .with_no_client_auth()
    .with_single_cert(vec![certificate()], key)
    .expect("the impostor's certificate")
}

fn answer(
  mut stream: StreamOwned<ServerConnection, TcpStream>,
  (id, answers): (&str, &Answers),
  asked: &Mutex<Vec<String>>,
) {
  let Some((line, body)) = read_request(&mut stream) else {
    return;
  };
  let path = line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
  asked.lock().push(path.clone());
  let reply = match path.as_str() {
    "GET /api/v1/server" => json!({"name": "Kontur team", "api_version": 3, "needs_setup": false, "version": "0.1.0",
      "cloud": {"server": id, "root_fingerprint": "root", "accepting": true, "codes_need_two_factor": true}}),
    "POST /api/v1/server/key-proof" => {
      let request: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
      answers.key_proof(id, request["nonce"].as_str().unwrap_or_default())
    }
    _ => json!({"error": "not_found", "message": "Not found"}),
  };
  let text = reply.to_string();
  let response = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
    text.len()
  );
  // The test reads the answer or fails on its own; a broken connection needs nothing here, nor a close it missed.
  let _ = stream.write_all(response.as_bytes());
  stream.conn.send_close_notify();
  let _ = stream.flush();
}

fn read_request(stream: &mut impl Read) -> Option<(String, String)> {
  let mut buffer = Vec::new();
  let mut chunk = [0_u8; 4096];
  loop {
    let read = stream.read(&mut chunk).ok()?;
    if read == 0 {
      return None;
    }
    buffer.extend_from_slice(&chunk[..read]);
    let text = String::from_utf8_lossy(&buffer).into_owned();
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
      continue;
    };
    let length = head
      .to_ascii_lowercase()
      .lines()
      .find_map(|line| line.strip_prefix("content-length:").map(str::to_owned))
      .and_then(|value| value.trim().parse::<usize>().ok())
      .unwrap_or(0);
    if body.len() >= length {
      return Some((head.lines().next().unwrap_or_default().to_owned(), body.to_owned()));
    }
  }
}
