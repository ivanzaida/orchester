//! A stand-in Orchester Cloud on loopback: answers each request with what its handler makes of it, cut short or
//! stalled when the handler says so, and keeps what it was asked.

use std::{
  io::{BufRead, BufReader, Write},
  net::{TcpListener, TcpStream},
  sync::{Arc, Mutex},
  thread,
  time::Duration,
};

/// One request as the stand-in read it.
#[derive(Clone, Debug)]
pub struct Asked {
  /// The method and the target, e.g. `GET /v1/updates/check?version=0.3.0&…`.
  pub line: String,
  /// Header names in lower case, with their values.
  pub headers: Vec<(String, String)>,
}

impl Asked {
  pub fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(key, _)| key == name)
      .map(|(_, value)| value.as_str())
  }

  /// The first byte a `Range: bytes=<start>-` asks for; 0 without one.
  pub fn range_start(&self) -> u64 {
    self
      .header("range")
      .and_then(|range| range.strip_prefix("bytes="))
      .and_then(|range| range.strip_suffix('-'))
      .map_or(0, |start| start.parse().expect("a range start"))
  }
}

/// An answer: the status, extra headers and the body; with `cut`, only that many bytes of the body go out before the
/// connection closes, as when a download breaks off; with `stall`, that many go out and the rest only after
/// [`STALL`], as a slow download.
pub struct Answer {
  pub status: &'static str,
  pub headers: Vec<String>,
  pub body: Vec<u8>,
  pub cut: Option<usize>,
  pub stall: Option<usize>,
}

/// How long a stalled answer holds the rest of its body.
const STALL: Duration = Duration::from_secs(30);

impl Answer {
  pub fn json(body: &serde_json::Value) -> Self {
    Self {
      status: "200 OK",
      headers: vec!["Content-Type: application/json".to_owned()],
      body: serde_json::to_vec(body).expect("JSON"),
      cut: None,
      stall: None,
    }
  }

  /// The file `bytes` from the byte the request asks for: `200` with all of it, or `206` with the rest.
  pub fn file(asked: &Asked, bytes: &[u8]) -> Self {
    let start = asked.range_start();
    let total = bytes.len();
    let start_index = usize::try_from(start).expect("a small file");
    let mut headers = vec!["Content-Type: application/octet-stream".to_owned()];
    let status = if start == 0 {
      "200 OK"
    } else {
      headers.push(format!("Content-Range: bytes {start}-{}/{total}", total - 1));
      "206 Partial Content"
    };
    Self {
      status,
      headers,
      body: bytes[start_index..].to_vec(),
      cut: None,
      stall: None,
    }
  }
}

type Handler = dyn Fn(&Asked) -> Answer + Send + Sync;

pub struct Stand {
  /// The stand-in's API address, `http://127.0.0.1:<port>`.
  pub api: String,
  asked: Arc<Mutex<Vec<Asked>>>,
}

impl Stand {
  pub fn start(handler: impl Fn(&Asked) -> Answer + Send + Sync + 'static) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    // The bound address, not `localhost`: each `localhost` connection costs ~300 ms on Windows (IPv6 first).
    let api = format!("http://{}", listener.local_addr().expect("address"));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let kept = asked.clone();
    let handler: Arc<Handler> = Arc::new(handler);
    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        let request = read(&stream);
        kept.lock().expect("asked").push(request.clone());
        let answer = handler(&request);
        // A stalled answer goes out on its own, so the next request is answered meanwhile.
        thread::spawn(move || write(stream, &answer));
      }
    });
    Self { api, asked }
  }

  pub fn asked(&self) -> Vec<Asked> {
    self.asked.lock().expect("asked").clone()
  }
}

fn read(stream: &TcpStream) -> Asked {
  let mut reader = BufReader::new(stream);
  let mut line = String::new();
  reader.read_line(&mut line).expect("request line");
  let line = line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
  let mut headers = Vec::new();
  loop {
    let mut header = String::new();
    reader.read_line(&mut header).expect("header");
    let Some((name, value)) = header.trim_end().split_once(':') else {
      break;
    };
    headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
  }
  Asked { line, headers }
}

fn write(mut stream: TcpStream, answer: &Answer) {
  let mut head = format!(
    "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n",
    answer.status,
    answer.body.len()
  );
  for header in &answer.headers {
    head.push_str(header);
    head.push_str("\r\n");
  }
  head.push_str("\r\n");
  let body = answer.cut.map_or(&answer.body[..], |cut| &answer.body[..cut]);
  // The test reads what was asked; a client that hung up early fails its own assertions.
  let _ = stream.write_all(head.as_bytes());
  if let Some(first) = answer.stall {
    let _ = stream.write_all(&body[..first]);
    let _ = stream.flush();
    thread::sleep(STALL);
    let _ = stream.write_all(&body[first..]);
    return;
  }
  let _ = stream.write_all(body);
}
