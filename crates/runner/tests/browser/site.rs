//! The pages the browser tests open, served on loopback until the test ends: `/form`, a guest book whose Save button
//! writes a status, logs to the console, fetches `/api/save` and shows an alert; `/later`, a page to go back from;
//! `/api/save`, a small JSON answer; the *Browser* window tests' `/window-form`, `/animated` and `/dialog`
//! (`crate::window`); and any other `/<name>`, a page titled `Page <name>`.

use tokio::{
  io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::{TcpListener, TcpStream},
};

const FORM: &str = r#"<!doctype html><html><head><title>Form</title></head><body><main>
<h1>Guest book</h1>
<label for="name">Name</label> <input id="name" type="text">
<button id="save" onclick="save()">Save</button>
<p id="status">Not saved</p>
<div style="height: 3000px">The end of the book is far below.</div>
</main><script>
function save() {
  const name = document.getElementById('name').value;
  document.getElementById('status').textContent = 'Saved ' + name;
  console.log('saved ' + name);
  console.error('oops ' + name);
  fetch('/api/save?name=' + encodeURIComponent(name));
  alert('Saved ' + name);
}
</script></body></html>"#;

/// Serves the pages; the address to put their names after, `http://127.0.0.1:<port>/`.
pub async fn serve() -> String {
  let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
  let address = listener.local_addr().expect("its address");
  tokio::spawn(async move {
    while let Ok((stream, _)) = listener.accept().await {
      tokio::spawn(answer(stream));
    }
  });
  format!("http://{address}/")
}

async fn answer(stream: TcpStream) {
  let mut reader = BufReader::new(stream);
  let mut request = String::new();
  if reader.read_line(&mut request).await.is_err() {
    return;
  }
  let mut header = String::new();
  while reader.read_line(&mut header).await.is_ok_and(|read| read > 2) {
    header.clear();
  }
  let path = request.split_whitespace().nth(1).unwrap_or("/");
  let name = path.trim_start_matches('/');
  let (kind, body) = match name.split('?').next().unwrap_or_default() {
    "form" => ("text/html", FORM.to_owned()),
    "later" => (
      "text/html",
      "<!doctype html><html><head><title>Later</title></head><body><p>A later page.</p></body></html>".to_owned(),
    ),
    "api/save" => ("application/json", r#"{"saved":true}"#.to_owned()),
    "window-form" => ("text/html", crate::window::FORM.to_owned()),
    "animated" => ("text/html", crate::window::ANIMATED.to_owned()),
    "dialog" => ("text/html", crate::window::DIALOG.to_owned()),
    _ => (
      "text/html",
      format!("<!doctype html><html><head><title>Page {name}</title></head><body>{name}</body></html>"),
    ),
  };
  let response = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
    body.len()
  );
  // A browser that stopped reading leaves nothing to answer.
  let _ = reader.get_mut().write_all(response.as_bytes()).await;
}
