//! The stand-in team server of the resource tests, started as a signed-in member or admin of one shared project, and
//! the calls its resource routes answer.

use std::{
  io::{Read, Write},
  net::TcpStream,
};

use orchester_api::test_support::team_server::{CODE, Standing, TeamServer};
use serde_json::{Value, json};

pub struct Team {
  pub server: TeamServer,
  token: String,
  project: String,
}

impl Team {
  pub fn start(standing: Standing) -> Self {
    let server = TeamServer::start();
    server.share(&[("Kontur", "kontur", None)]);
    server.stand_as(standing);
    let project = server.project("kontur").expect("the shared project").to_string();
    let mut team = Self {
      server,
      token: String::new(),
      project,
    };
    let (status, joined) = team.call("POST", "/api/v1/auth/code", &json!({"code": CODE}));
    assert_eq!(status, 201, "{joined}");
    joined["token"].as_str().expect("a token").clone_into(&mut team.token);
    team
  }

  fn call(&self, method: &str, path: &str, body: &Value) -> (u16, Value) {
    let address = self.server.url().trim_start_matches("http://").to_owned();
    let mut stream = TcpStream::connect(address).expect("the stand-in listens");
    let body = body.to_string();
    let request = format!(
      "{method} {path} HTTP/1.1\r\nHost: stand-in\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\n\
       Content-Length: {}\r\n\r\n{body}",
      self.token,
      body.len()
    );
    stream.write_all(request.as_bytes()).expect("send");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).expect("an answer");
    let (head, body) = answer.split_once("\r\n\r\n").expect("a head and a body");
    let status = head
      .split_whitespace()
      .nth(1)
      .and_then(|code| code.parse().ok())
      .expect("a status");
    (status, serde_json::from_str(body).unwrap_or(Value::Null))
  }

  pub fn resources(&self, method: &str, rest: &str, body: &Value) -> (u16, Value) {
    self.call(
      method,
      &format!("/api/v1/projects/{}/resources{rest}", self.project),
      body,
    )
  }
}
