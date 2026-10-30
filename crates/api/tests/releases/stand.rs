//! A stand-in cloud on loopback: answers each request with the next canned answer and keeps what it was asked.

use std::{
  io::{BufRead, BufReader, Read, Write},
  net::{TcpListener, TcpStream},
  sync::{Arc, Mutex},
  thread,
};

use orchester_api::client::{CloudAddress, CloudClient, ServerAddress};

/// One request as the stand-in read it.
#[derive(Clone, Debug)]
pub struct Asked {
  /// The method and the target, e.g. `GET /v1/updates/check?version=0.4.0&…`.
  pub line: String,
  /// Header names in lower case, with their values.
  pub headers: Vec<(String, String)>,
  pub body: Vec<u8>,
}

impl Asked {
  pub fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(key, _)| key == name)
      .map(|(_, value)| value.as_str())
  }
}

/// A canned answer: the status line's code and reason, extra headers and the body.
pub struct Answer {
  pub status: &'static str,
  pub headers: Vec<String>,
  pub body: Vec<u8>,
}

impl Answer {
  pub fn json(status: &'static str, body: &serde_json::Value) -> Self {
    Self {
      status,
      headers: vec!["Content-Type: application/json".to_owned()],
      body: serde_json::to_vec(body).expect("JSON"),
    }
  }

  pub fn empty(status: &'static str) -> Self {
    Self {
      status,
      headers: Vec::new(),
      body: Vec::new(),
    }
  }
}

pub struct Stand {
  pub client: CloudClient,
  asked: Arc<Mutex<Vec<Asked>>>,
}

impl Stand {
  pub fn start(answers: Vec<Answer>) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    // The bound address, not `localhost`: each `localhost` connection costs ~300 ms on Windows (IPv6 first).
    let api = format!("http://{}", listener.local_addr().expect("address"));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let kept = asked.clone();
    thread::spawn(move || {
      for (stream, answer) in listener.incoming().flatten().zip(answers) {
        let request = read(&stream);
        kept.lock().expect("asked").push(request);
        write(stream, &answer);
      }
    });
    let web = ServerAddress::parse("https://orchester.cloud").expect("web address");
    let address = CloudAddress::new(web, Some(ServerAddress::parse(&api).expect("API address")));
    Self {
      client: CloudClient::new(address).expect("client"),
      asked,
    }
  }

  pub fn asked(&self) -> Vec<Asked> {
    self.asked.lock().expect("asked").clone()
  }

  pub fn lines(&self) -> Vec<String> {
    self.asked().into_iter().map(|asked| asked.line).collect()
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
  let length = headers
    .iter()
    .find(|(name, _)| name == "content-length")
    .map_or(0, |(_, value)| value.parse().expect("a length"));
  let mut body = vec![0; length];
  reader.read_exact(&mut body).expect("body");
  Asked { line, headers, body }
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
  // The test reads what was asked; a client that hung up early fails its own assertions.
  let _ = stream.write_all(head.as_bytes());
  let _ = stream.write_all(&answer.body);
}
