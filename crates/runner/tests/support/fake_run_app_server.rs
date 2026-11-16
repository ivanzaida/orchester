//! A started app stand-in serves MCP over loopback with the app's discovery contract.

use std::{
  fs,
  io::{BufRead, BufReader, Read, Write},
  net::{TcpListener, TcpStream},
  path::Path,
  time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

const TOKEN: &str = "stand-in-app-private-token";

pub fn serve(folder: &str) -> Result<(), String> {
  let folder = Path::new(folder);
  let listener = publish(folder, "orchester-app")?;
  println!("ready");
  std::io::stdout().flush().map_err(|_| "flush")?;
  accept(&listener, folder)
}

fn publish(folder: &Path, app: &str) -> Result<TcpListener, String> {
  fs::create_dir_all(folder).map_err(|_| "discovery folder")?;
  let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| "bind")?;
  let port = listener.local_addr().map_err(|_| "address")?.port();
  let pid = std::process::id();
  let discovery = json!({"pid":pid,"app":app,"port":port,
    "url":format!("http://127.0.0.1:{port}/mcp"),"token":TOKEN});
  fs::write(folder.join(format!("{pid}.json")), discovery.to_string()).map_err(|_| "discovery")?;
  Ok(listener)
}

fn accept(listener: &TcpListener, folder: &Path) -> Result<(), String> {
  for stream in listener.incoming() {
    let stream = stream.map_err(|_| "accept")?;
    let folder = folder.to_path_buf();
    std::thread::spawn(move || respond(stream, &folder).expect("stand-in request"));
  }
  Ok(())
}

fn respond(mut stream: TcpStream, folder: &Path) -> std::io::Result<()> {
  stream.set_read_timeout(Some(Duration::from_secs(5)))?;
  let mut reader = BufReader::new(stream.try_clone()?);
  let mut line = String::new();
  reader.read_line(&mut line)?;
  let mut length = 0;
  let mut authorized = false;
  let mut session = false;
  loop {
    line.clear();
    reader.read_line(&mut line)?;
    if line == "\r\n" || line.is_empty() {
      break;
    }
    if let Some((name, value)) = line.split_once(':') {
      match name.to_ascii_lowercase().as_str() {
        "content-length" => length = value.trim().parse().expect("length"),
        "authorization" => authorized = value.trim() == format!("Bearer {TOKEN}"),
        "mcp-session-id" => session = value.trim() == "stand-in-session",
        _ => {}
      }
    }
  }
  assert!(authorized, "credential reached only the stand-in server");
  let mut body = vec![0; length];
  reader.read_exact(&mut body)?;
  let request: Value = serde_json::from_slice(&body).expect("request");
  let result = match request["method"].as_str().expect("method") {
    "initialize" => {
      json!({"protocolVersion":"2025-03-26","capabilities":{},"serverInfo":{"name":"stand-in","version":"1"}})
    }
    "notifications/initialized" => {
      assert!(session);
      stream.write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
      return Ok(());
    }
    "tools/call" => {
      assert!(session);
      record(folder, &request["params"])?;
      if request["params"]["name"] == "redirect" {
        let port = stream.local_addr()?.port();
        write!(
          stream,
          "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:{port}/mcp\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{TOKEN}",
          TOKEN.len()
        )?;
        return Ok(());
      }
      tool_result(&request["params"], folder)
    }
    _ => panic!("unexpected MCP method"),
  };
  let reply = json!({"jsonrpc":"2.0","id":request["id"],"result":result}).to_string();
  let sse = request["params"]["name"] == "lurq_screenshot";
  let body = if sse {
    format!("event: message\ndata: {reply}\n\n")
  } else {
    reply
  };
  if sse {
    write!(
      stream,
      "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: keep-alive\r\n\r\n{body}"
    )?;
    stream.flush()?;
    std::thread::sleep(Duration::from_secs(20));
    return Ok(());
  }
  write!(
    stream,
    "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nMcp-Session-Id: stand-in-session\r\nConnection: close\r\n\r\n{body}",
    if sse { "text/event-stream" } else { "application/json" },
    body.len()
  )
}

/// Notes the tool a call names in `calls`, and a lurq tool's call with its arguments, which show what each `ui_*` call
/// asked of the app, in `lurq-calls`.
fn record(folder: &Path, params: &Value) -> std::io::Result<()> {
  let append = |file: &str| fs::OpenOptions::new().create(true).append(true).open(folder.join(file));
  let name = params["name"].as_str().expect("tool");
  writeln!(append("calls")?, "{name}")?;
  if name.starts_with("lurq_") {
    writeln!(append("lurq-calls")?, "{name} {}", params["arguments"])?;
  }
  Ok(())
}

fn tool_result(params: &Value, folder: &Path) -> Value {
  match params["name"].as_str().expect("tool") {
    "navigate" => json!({"content":[{"type":"text","text":format!("navigated {TOKEN}")}],
      "structuredContent":{"route":params["arguments"]["route"],"credential":TOKEN,"nested":{TOKEN:[TOKEN]}},
      "isError":false}),
    "app_state" => {
      json!({"content":[{"type":"text","text":"app ready"}],"structuredContent":{"ready":true},"isError":false})
    }
    "tool_error" => json!({"content":[{"type":"text","text":format!("error {TOKEN}")}],"isError":true}),
    "lurq_screenshot" => {
      let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([7, 13, 29, 255]));
      let mut png = std::io::Cursor::new(Vec::new());
      image.write_to(&mut png, image::ImageFormat::Png).expect("PNG");
      json!({"content":[{"type":"image","mimeType":"image/png","data":STANDARD.encode(png.into_inner())}],"isError":false})
    }
    "bad_image" => json!({"content":[{"type":"image","mimeType":"image/png","data":"not base64"}]}),
    name if name.starts_with("lurq_") => lurq_result(name, &params["arguments"], folder),
    _ => panic!("unknown stand-in tool"),
  }
}

/// The lurq tools the `mcp` UI driver calls, answered as lurq answers them: JSON as pretty text and structured
/// content, the log as text. Its window holds a Save button, a Title field and a Priority select; an element named
/// `Saved` is shown from the fourth `lurq_inspect` on, so a wait for it looks more than once.
fn lurq_result(name: &str, arguments: &Value, folder: &Path) -> Value {
  let json = |value: Value| {
    json!({"content":[{"type":"text","text":serde_json::to_string_pretty(&value).expect("JSON")}],
      "structuredContent":value,"isError":false})
  };
  let node =
    |reference: &str, role: &str, name: &str| json!({"ref":reference,"role":role,"name":name,"actions":["invoke"]});
  match name {
    "lurq_windows" => json(json!({"windows":[{"id":"main","name":"main","title":"Orchester QA","focused":false}]})),
    "lurq_inspect" => {
      let inspected = fs::read_to_string(folder.join("calls"))
        .unwrap_or_default()
        .lines()
        .filter(|line| *line == "lurq_inspect")
        .count();
      let elements = [
        node("ref_2", "button", "Save"),
        node("ref_3", "textbox", "Title"),
        node("ref_4", "combobox", "Priority"),
      ];
      match arguments["query"].as_str() {
        Some("Saved") if inspected >= 4 => {
          json(json!({"window":"main","matches":[node("ref_9", "status", "Saved")],"truncated":false}))
        }
        Some(query) => {
          let matches: Vec<&Value> = elements
            .iter()
            .filter(|element| element["name"].as_str().is_some_and(|name| name.contains(query)))
            .collect();
          json(json!({"window":"main","matches":matches,"truncated":false}))
        }
        None => json(json!({"window":"main","truncated":false,"tree":
          {"ref":"ref_1","role":"window","name":"Orchester QA","state":{},"children":elements}})),
      }
    }
    "lurq_act" | "lurq_interact" | "lurq_set_value" => {
      let mut answered = arguments.clone();
      answered["ok"] = json!(true);
      json(answered)
    }
    "lurq_wait" => json(json!({"ok":true,"idle":true})),
    "lurq_logs" => json!({"content":[{"type":"text","text":"INFO app ready\nINFO saved the task"}],"isError":false}),
    _ => panic!("unknown stand-in lurq tool"),
  }
}
