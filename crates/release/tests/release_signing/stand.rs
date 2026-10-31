//! A stand-in cloud on loopback: answers each request with the next canned answer and keeps what it was asked.

use std::{
  io::{BufRead, BufReader, Read, Write},
  net::{TcpListener, TcpStream},
  sync::{Arc, Mutex},
  thread,
};

use orchester_api::client::{CloudAddress, CloudClient, ServerAddress};

/// One request as the stand-in read it: `METHOD /path`, the `authorization` header and the body.
#[derive(Clone, Debug)]
pub struct Asked {
  pub line: String,
  pub authorization: Option<String>,
  pub body: Vec<u8>,
}

pub struct Stand {
  pub client: CloudClient,
  asked: Arc<Mutex<Vec<Asked>>>,
}

impl Stand {
  /// Answers with `answers`, each a status line such as `200 OK` and a JSON body.
  pub fn start(answers: Vec<(&'static str, serde_json::Value)>) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let api = format!("http://{}", listener.local_addr().expect("address"));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let kept = asked.clone();
    thread::spawn(move || {
      for (stream, (status, body)) in listener.incoming().flatten().zip(answers) {
        let request = read(&stream);
        kept.lock().expect("asked").push(request);
        write(stream, status, &serde_json::to_vec(&body).expect("JSON"));
      }
    });
    let api = ServerAddress::parse(&api).expect("API address");
    Self {
      client: CloudClient::new(CloudAddress::new(api.clone(), Some(api))).expect("client"),
      asked,
    }
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
  let (mut length, mut authorization) = (0, None);
  loop {
    let mut header = String::new();
    reader.read_line(&mut header).expect("header");
    let Some((name, value)) = header.trim_end().split_once(':') else {
      break;
    };
    match name.to_ascii_lowercase().as_str() {
      "content-length" => length = value.trim().parse().expect("a length"),
      "authorization" => authorization = Some(value.trim().to_owned()),
      _ => {}
    }
  }
  let mut body = vec![0; length];
  reader.read_exact(&mut body).expect("body");
  Asked {
    line,
    authorization,
    body,
  }
}

fn write(mut stream: TcpStream, status: &str, body: &[u8]) {
  let head = format!(
    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    body.len()
  );
  // The test reads what was asked; a client that hung up early fails its own assertions.
  let _ = stream.write_all(head.as_bytes());
  let _ = stream.write_all(body);
}
