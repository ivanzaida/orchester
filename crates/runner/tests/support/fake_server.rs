//! `listen <port>`: a stand-in web server for the service tests. It accepts connections on `127.0.0.1:<port>` until
//! it is ended and answers every request by its path: `/moved` with `302 Found`, `/missing` with `404 Not Found`,
//! any other with `200 OK`. It prints `listening on <port>` once it accepts connections. `connect <port>` fails
//! unless something accepts connections on `127.0.0.1:<port>`: a service that needs another checks it is ready.

use std::{
  io::{BufRead, BufReader, Write},
  net::{Ipv4Addr, TcpListener, TcpStream},
};

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
  // The rest of the head, up to its empty line.
  let mut line = String::new();
  while reader.read_line(&mut line).is_ok_and(|read| read > 0) && line.trim_end() != "" {
    line.clear();
  }
  let path = request.split_whitespace().nth(1).unwrap_or("/");
  let status = match path {
    "/moved" => "302 Found\r\nLocation: /",
    "/missing" => "404 Not Found",
    _ => "200 OK",
  };
  let mut stream = stream;
  if let Err(error) = write!(
    stream,
    "HTTP/1.1 {status}\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
  ) {
    eprintln!("cannot answer: {error}");
  }
}
