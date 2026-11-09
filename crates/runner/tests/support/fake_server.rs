//! `listen <port>`: a stand-in web server for the service tests. It accepts connections on `127.0.0.1:<port>` until
//! it is ended and answers every request by its path: `/moved` with `302 Found`, `/missing` with `404 Not Found`,
//! `/api/…` as the seed tests' small JSON API ([`api`]), `/app` with the browser driver's fixture page ([`APP`]) and
//! `/worker.js` with its workers' script ([`WORKER`]), any other with `200 OK`. It prints `listening on <port>` once
//! it accepts connections. `connect <port>` fails unless something accepts connections on `127.0.0.1:<port>`: a
//! service that needs another checks it is ready.

use std::{
  collections::BTreeMap,
  io::{BufRead, BufReader, Read, Write},
  net::{Ipv4Addr, TcpListener, TcpStream},
};

/// The browser driver's fixture page: a field, a list and Save, which says what was saved, logs a console error and
/// asks for `/missing` (404) and for a page of another origin; an image and a link of another origin, a link that
/// opens another origin in a new tab, and a worker and a service worker that ask for another origin.
const APP: &str = r#"<!doctype html>
<html><head><title>Fixture app</title></head>
<body>
<h1>Fixture app</h1>
<input id="title" aria-label="Title">
<select id="priority" aria-label="Priority"><option>Low</option><option>High</option></select>
<button id="save" onclick="save()">Save</button>
<p id="status">Status: idle</p>
<a id="away" href="https://example.com/away">Elsewhere</a>
<a id="popup" href="https://example.com/popup" target="_blank">New tab</a>
<img alt="" src="https://example.com/pixel.png">
<script>
new Worker("/worker.js");
navigator.serviceWorker.register("/worker.js").catch((error) => console.error("no service worker: " + error));
function save() {
  const status = document.getElementById("status");
  status.textContent = "Saved: " + document.getElementById("title").value + " ("
    + document.getElementById("priority").value + ")";
  status.className = "saved";
  console.error("save logged an error on purpose");
  fetch("/missing");
  fetch("https://example.com/track").catch(() => {});
}
</script>
</body></html>"#;

/// The fixture page's workers' script: each asks for another origin, the service worker as `from-service-worker`.
const WORKER: &str = r#"const who = self.registration ? "service-worker" : "worker";
fetch("https://example.com/from-" + who).catch(() => {});"#;

pub fn listen(port: Option<&str>) -> Result<(), String> {
  let port: u16 = port.and_then(|port| port.parse().ok()).ok_or("listen needs a port")?;
  let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|error| error.to_string())?;
  super::say(Some(&format!("listening on {port}")))?;
  for stream in listener.incoming() {
    match stream {
      // The connection closes once answered, as the stream drops.
      Ok(stream) => answer(&stream),
      Err(error) => eprintln!("cannot accept a connection: {error}"),
    }
  }
  Ok(())
}

pub fn connect(port: Option<&str>) -> Result<(), String> {
  let port: u16 = port.and_then(|port| port.parse().ok()).ok_or("connect needs a port")?;
  TcpStream::connect((Ipv4Addr::LOCALHOST, port))
    .map(drop)
    .map_err(|error| format!("nothing accepts connections on {port}: {error}"))
}

fn answer(stream: &TcpStream) {
  let mut reader = BufReader::new(stream);
  let mut request = String::new();
  if reader.read_line(&mut request).is_err() {
    return;
  }
  // The rest of the head, up to its empty line, its headers by their lowercase names.
  let mut headers = BTreeMap::new();
  let mut line = String::new();
  while reader.read_line(&mut line).is_ok_and(|read| read > 0) && line.trim_end() != "" {
    if let Some((name, value)) = line.trim_end().split_once(':') {
      headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
    }
    line.clear();
  }
  let length = headers
    .get("content-length")
    .and_then(|length| length.parse::<usize>().ok())
    .unwrap_or(0);
  let mut body = vec![0; length];
  if reader.read_exact(&mut body).is_err() {
    return;
  }
  let mut words = request.split_whitespace();
  let (method, path) = (words.next().unwrap_or("GET"), words.next().unwrap_or("/"));
  let (status, kind, body) = match path {
    "/moved" => ("302 Found\r\nLocation: /", "text/plain", "ok".to_owned()),
    "/missing" => ("404 Not Found", "text/plain", "ok".to_owned()),
    "/app" => ("200 OK", "text/html; charset=utf-8", APP.to_owned()),
    "/worker.js" => ("200 OK", "text/javascript", WORKER.to_owned()),
    _ if path.starts_with("/api/") => {
      let (status, body) = api(method, path, &headers, &String::from_utf8_lossy(&body));
      (status, "application/json", body)
    }
    _ => ("200 OK", "text/plain", "ok".to_owned()),
  };
  let mut stream = stream;
  if let Err(error) = write!(
    stream,
    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
    body.len()
  ) {
    eprintln!("cannot answer: {error}");
  }
}

/// The seed tests' API. It prints `api: <method> <path>` and `header <name>: <value>` for each header it got, then
/// answers:
///
/// - `POST /api/teams` with a body that names the team: `201` with `{"team":{"id":"team-7"}}`;
/// - `POST /api/teams/team-7/users`: `201` with `{"users":[{"id":"user-3","admin":true}]}`, any other team `404`;
/// - `GET /api/session` with the header `X-User: user-3`: `200` with `{"token":"tok-9","expires":3600}`, else `403`;
/// - `GET /api/key`: `200` with `{"key":"<the value of its X-Admin-Key header>"}`;
/// - `/api/echo`: `500` with every header it got;
/// - `/api/fail`: `500` with `{"error":"database is locked"}` and 2000 `x`;
/// - any other: `404`.
fn api(method: &str, path: &str, headers: &BTreeMap<String, String>, body: &str) -> (&'static str, String) {
  println!("api: {method} {path}");
  for (name, value) in headers {
    println!("header {name}: {value}");
  }
  let header = |name: &str| headers.get(name).map_or("", String::as_str);
  match (method, path) {
    ("POST", "/api/teams") if body.contains("\"name\"") => ("201 Created", r#"{"team":{"id":"team-7"}}"#.to_owned()),
    ("POST", "/api/teams/team-7/users") => ("201 Created", r#"{"users":[{"id":"user-3","admin":true}]}"#.to_owned()),
    ("GET", "/api/session") if header("x-user") == "user-3" => {
      ("200 OK", r#"{"token":"tok-9","expires":3600}"#.to_owned())
    }
    ("GET", "/api/session") => ("403 Forbidden", r#"{"error":"who are you"}"#.to_owned()),
    ("GET", "/api/key") => ("200 OK", format!(r#"{{"key":"{}"}}"#, header("x-admin-key"))),
    (_, "/api/echo") => {
      let echoed: Vec<String> = headers.iter().map(|(name, value)| format!("{name}={value}")).collect();
      ("500 Internal Server Error", echoed.join("; "))
    }
    (_, "/api/fail") => (
      "500 Internal Server Error",
      format!(r#"{{"error":"database is locked"}}{}"#, "x".repeat(2000)),
    ),
    _ => ("404 Not Found", r#"{"error":"no such thing"}"#.to_owned()),
  }
}
