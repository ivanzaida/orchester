//! A team server that gives itself another server's id (`docs/architecture/server.md`, "Codes"): `GET /server` names
//! that id, and `POST /server/key-proof` answers with a key it holds, whose id is not that one, or with the other
//! server's public key, which it cannot sign with. It records the paths it was asked, so a test can tell that no code
//! reached it.

use std::{
  io::{Read, Write},
  net::{TcpListener, TcpStream},
  sync::Arc,
  thread,
};

use orchester_api::{
  identity::{PrivateKey, SigningKey},
  statements::{KEY_PROOF_TYPE, KeyProof},
};
use parking_lot::Mutex;
use serde_json::{Value, json};

/// Whose public key the impostor answers with.
pub enum Claimed {
  /// Its own, whose id is not the one it gives itself.
  OwnKey,
  /// The other server's (base64url), which it cannot sign with.
  Borrowed(String),
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
    let key = SigningKey::new(&PrivateKey::generate().expect("a key")).expect("a signing key");
    let public_key = match claimed {
      Claimed::OwnKey => key.public_key().encoded(),
      Claimed::Borrowed(public_key) => public_key,
    };
    let (id, seen) = (id.to_owned(), asked.clone());
    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        answer(stream, (&id, &public_key, &key), &seen);
      }
    });
    Self { port, asked }
  }

  pub fn url(&self) -> String {
    format!("http://localhost:{}", self.port)
  }

  /// The requests it was asked, as `METHOD /path`.
  pub fn asked(&self) -> Vec<String> {
    self.asked.lock().clone()
  }
}

fn answer(mut stream: TcpStream, (id, public_key, key): (&str, &str, &SigningKey), asked: &Mutex<Vec<String>>) {
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
      let proof = KeyProof {
        server: id.parse().expect("a server id"),
        nonce: request["nonce"].as_str().unwrap_or_default().to_owned(),
        issued_at: 0,
      };
      let signed = key.sign_statement(KEY_PROOF_TYPE, &proof).expect("signed");
      json!({"public_key": public_key, "proof": signed})
    }
    _ => json!({"error": "not_found", "message": "Not found"}),
  };
  let text = reply.to_string();
  let response = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
    text.len()
  );
  // The test reads the answer or fails on its own; a broken connection needs nothing here.
  let _ = stream.write_all(response.as_bytes());
}

fn read_request(stream: &mut TcpStream) -> Option<(String, String)> {
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
