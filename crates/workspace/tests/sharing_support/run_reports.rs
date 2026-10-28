//! A team server that takes the run progress report (`PUT …/run`, ORC-284) in front of the test server, which does
//! not have it (the platform's team server implements it): every other request goes through to the test server. Until
//! the report is turned on, it goes through as well, and the test server answers it as a server without the route
//! does. Once on, each report is recorded, and a merged or cancelled one releases the claim on the test server, which
//! has no completed claims, so the claim ends there either way.

use std::{
  net::SocketAddr,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use axum::{
  Router,
  body::Body,
  extract::{Path, Request, State},
  http::{Method, StatusCode, header::AUTHORIZATION},
  response::{IntoResponse, Response},
  routing::put,
};
use hyper_util::rt::TokioIo;
use orchester_api::RunReport;
use orchester_core::run::TaskPhase;
use parking_lot::Mutex;
use tokio::{
  net::{TcpListener, TcpStream},
  runtime::Runtime,
};

pub struct ReportingServer {
  port: u16,
  reports: Arc<Reports>,
  _runtime: Runtime,
}

struct Reports {
  target: SocketAddr,
  supported: AtomicBool,
  /// By task id, in the order they came.
  taken: Mutex<Vec<(String, TaskPhase)>>,
}

impl ReportingServer {
  /// In front of the test server at `target`, without the report until [`Self::take_reports`].
  pub fn start(target: SocketAddr) -> Self {
    let runtime = Runtime::new().expect("the reporting server's runtime");
    let reports = Arc::new(Reports {
      target,
      supported: AtomicBool::new(false),
      taken: Mutex::default(),
    });
    let app = Router::new()
      .route("/api/v1/projects/{project}/tasks/{task}/run", put(report))
      .fallback(pass)
      .with_state(reports.clone());
    let listener = runtime.block_on(TcpListener::bind("127.0.0.1:0")).expect("listen");
    let port = listener.local_addr().expect("address").port();
    runtime.spawn(async move { axum::serve(listener, app).await });
    Self {
      port,
      reports,
      _runtime: runtime,
    }
  }

  pub fn url(&self) -> String {
    format!("http://127.0.0.1:{}", self.port)
  }

  /// From now on the server takes the report.
  pub fn take_reports(&self) {
    self.reports.supported.store(true, Ordering::SeqCst);
  }

  /// The reports taken, by task id, in the order they came.
  pub fn taken(&self) -> Vec<(String, TaskPhase)> {
    self.reports.taken.lock().clone()
  }
}

async fn report(
  State(reports): State<Arc<Reports>>,
  Path((project, task)): Path<(String, String)>,
  request: Request,
) -> Response {
  if !reports.supported.load(Ordering::SeqCst) {
    return forward(reports.target, request).await;
  }
  let token = request.headers().get(AUTHORIZATION).cloned();
  let body = axum::body::to_bytes(request.into_body(), usize::MAX)
    .await
    .expect("the report");
  let Ok(report) = serde_json::from_slice::<RunReport>(&body) else {
    return StatusCode::UNPROCESSABLE_ENTITY.into_response();
  };
  reports.taken.lock().push((task.clone(), report.phase));
  if !matches!(report.phase, TaskPhase::Merged | TaskPhase::Cancelled) {
    // The tests' desktop reports only the phases that end a claim; this server stores no other.
    return StatusCode::NOT_IMPLEMENTED.into_response();
  }
  let mut release = Request::builder()
    .method(Method::DELETE)
    .uri(format!("/api/v1/projects/{project}/tasks/{task}/claim"));
  if let Some(token) = token {
    release = release.header(AUTHORIZATION, token);
  }
  forward(reports.target, release.body(Body::empty()).expect("a request")).await
}

async fn pass(State(reports): State<Arc<Reports>>, request: Request) -> Response {
  forward(reports.target, request).await
}

/// `request`, sent to the test server at `target`, and its answer.
async fn forward(target: SocketAddr, request: Request) -> Response {
  let stream = TcpStream::connect(target).await.expect("the test server");
  let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
    .await
    .expect("HTTP to the test server");
  tokio::spawn(connection);
  sender
    .send_request(request)
    .await
    .expect("the test server answers")
    .map(Body::new)
}
