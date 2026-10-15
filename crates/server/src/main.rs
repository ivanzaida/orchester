use std::{
  io::{self, Write},
  path::PathBuf,
  process::ExitCode,
  sync::Arc,
};

use orchester_server::{
  Server,
  clock::{Clock, SystemClock},
  config::Config,
  error::ServerError,
  host::{self, CloudCommand},
};
use tokio::net::TcpListener;

mod shutdown;

enum Command {
  Serve,
  Backup { path: PathBuf },
  TlsShow,
  TlsRegenerate,
  SetupCode,
  AccountCode { username: String },
  Cloud(CloudCommand),
}

#[tokio::main]
async fn main() -> ExitCode {
  // Only our deliberate structured request records are logged. Library debug logs may contain HTTP data.
  tracing_subscriber::fmt()
    .with_env_filter("orchester_server=info")
    .with_ansi(false)
    .with_writer(io::stderr)
    .init();
  match run().await {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      error.log();
      ExitCode::FAILURE
    }
  }
}

async fn run() -> Result<(), ServerError> {
  let (command, path) = arguments()?;
  let config = Config::load(path.as_deref())?;
  let now = SystemClock.now_ms();
  match command {
    Command::Serve => serve(config).await,
    Command::Backup { path } => orchester_server::backup_existing(config.data_dir, path).await,
    Command::TlsShow => match host::tls_pin(&config, now)? {
      Some(pin) => console(&format!(
        "Certificate check code: {}
SHA-256 fingerprint: {}",
        pin.check_code(),
        pin.fingerprint()
      )),
      None => console("This server serves plain HTTP (tls = \"off\"); its proxy holds the certificate."),
    },
    Command::TlsRegenerate => {
      let code = host::tls_regenerate(&config, now)?;
      console(&format!(
        "New certificate check code: {code}\nRestart the server to present it; every desktop must confirm it again."
      ))
    }
    Command::SetupCode => {
      let server = Server::open(config, Arc::new(SystemClock)).await?;
      match server.host_setup_code().await? {
        Some(code) => console(&format!("Setup code: {}", code.expose())),
        None => console("This server has an admin already; it has no setup code."),
      }
    }
    Command::AccountCode { username } => {
      let server = Server::open(config, Arc::new(SystemClock)).await?;
      let code = server.host_account_code(username).await?;
      console(&format!("Account code: {}", code.expose()))
    }
    Command::Cloud(command) => {
      let server = Server::open(config, Arc::new(SystemClock)).await?;
      let link = server.host_cloud(command).await?;
      let state = match (&link.root_fingerprint, link.accepting) {
        (None, _) => "not linked".to_owned(),
        (Some(root), true) => format!("linked, accepting, root key {root}"),
        (Some(root), false) => format!("linked, off, root key {root}"),
      };
      console(&format!("Server {}: Orchester Cloud {state}", link.server))
    }
  }
}

/// Serves until a shutdown signal, after printing the certificate's check code and, while the server has no admin, a
/// setup code: to the console only, never to the log.
async fn serve(config: Config) -> Result<(), ServerError> {
  let listen = config.listen;
  let server = Server::open(config, Arc::new(SystemClock)).await?;
  if let Some(code) = server.check_code() {
    console(&format!("Certificate check code: {code}"))?;
  }
  if let Some(code) = server.host_setup_code().await? {
    console(&format!(
      "Setup code: {} (valid 24 hours; open the server admin and enter it to make the first admin)",
      code.expose()
    ))?;
  }
  let shutdown = shutdown::Shutdown::install()?;
  let listener = TcpListener::bind(listen).await?;
  tracing::info!(address = %listener.local_addr()?, "server listening");
  let Some(admin_api) = server.admin_api_listen() else {
    return orchester_server::serve(server, listener, shutdown.wait()).await;
  };
  let internal = TcpListener::bind(admin_api).await?;
  tracing::info!(address = %internal.local_addr()?, "admin API listening");
  let stopping = shutdown.wait();
  let (stop, stopped) = tokio::sync::watch::channel(false);
  let mut internal_stopped = stopped.clone();
  let internal = tokio::spawn(orchester_server::serve_admin_api(
    server.clone(),
    internal,
    async move {
      // Ends with the public listener: a dropped sender means it is gone too.
      if internal_stopped.wait_for(|stop| *stop).await.is_err() {
        tracing::info!("admin API stopping with the server");
      }
    },
  ));
  let public = orchester_server::serve(server, listener, async move {
    stopping.await;
    stop.send_replace(true);
  })
  .await;
  drop(stopped);
  internal.await.map_err(|_| ServerError::Worker)??;
  public
}

/// One line for the person at the console. Codes go here and nowhere else.
fn console(line: &str) -> Result<(), ServerError> {
  writeln!(io::stdout().lock(), "{line}")?;
  Ok(())
}

fn arguments() -> Result<(Command, Option<PathBuf>), ServerError> {
  let mut args: Vec<_> = std::env::args_os().skip(1).collect();
  let path = if let Some(index) = args.iter().position(|arg| arg == "--config") {
    if index + 1 >= args.len() {
      return Err(usage());
    }
    let path = PathBuf::from(args.remove(index + 1));
    args.remove(index);
    Some(path)
  } else {
    None
  };
  let args = args
    .into_iter()
    .map(|arg| arg.into_string().map_err(|_| usage()))
    .collect::<Result<Vec<_>, _>>()?;
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let command = match args.as_slice() {
    [] | ["serve"] => Command::Serve,
    ["backup", path] => Command::Backup { path: path.into() },
    ["tls", "show"] => Command::TlsShow,
    ["tls", "regenerate"] => Command::TlsRegenerate,
    ["setup-code"] => Command::SetupCode,
    ["account-code", username] => Command::AccountCode {
      username: (*username).to_owned(),
    },
    ["cloud", "show"] => Command::Cloud(CloudCommand::Show),
    ["cloud", "on"] => Command::Cloud(CloudCommand::On),
    ["cloud", "off"] => Command::Cloud(CloudCommand::Off),
    ["cloud", "pin", root] => Command::Cloud(CloudCommand::Pin((*root).to_owned())),
    ["cloud", "unlink"] => Command::Cloud(CloudCommand::Unlink),
    _ => return Err(usage()),
  };
  Ok((command, path))
}

fn usage() -> ServerError {
  ServerError::Configuration(
    "usage: orchester-server [--config server.toml] [serve | backup <path> | tls show | tls regenerate | setup-code | account-code <username> | cloud show|on|off|unlink | cloud pin <root public key>]",
  )
}
