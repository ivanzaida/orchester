use std::{io, sync::Arc};

use parking_lot::Mutex;
use tracing_subscriber::util::SubscriberInitExt;

/// Everything the run logged under the service's own filter (`orchester_cloud::LOG_FILTER`), so the run can assert
/// that no secret reaches the service's log.
#[derive(Clone, Default)]
pub struct Capture(Arc<Mutex<Vec<u8>>>);

impl io::Write for Capture {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.0.lock().extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

impl Capture {
  pub fn install() -> Self {
    let capture = Self::default();
    let writer = capture.clone();
    tracing_subscriber::fmt()
      .with_ansi(false)
      .with_env_filter(orchester_cloud::LOG_FILTER)
      .with_writer(move || writer.clone())
      .finish()
      .try_init()
      .expect("install the run's log capture");
    capture
  }

  pub fn text(&self) -> String {
    String::from_utf8_lossy(&self.0.lock()).into_owned()
  }
}
