use std::{
  io::{self, Write},
  path::PathBuf,
  process::ExitCode,
  sync::Arc,
};

use orchester_api::CreateUser;
use orchester_server::{Server, clock::SystemClock, config::Config, error::ServerError};
use tokio::net::TcpListener;

mod shutdown;

enum Command {
  Serve,
  User(UserCommand),
  Backup { path: PathBuf },
}

enum UserCommand {
  Create { username: String, admin: bool },
  Reset { username: String },
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
  match command {
    Command::Serve => {
      let listen = config.listen;
      let server = Server::open(config, Arc::new(SystemClock)).await?;
      let shutdown = shutdown::Shutdown::install()?;
      let listener = TcpListener::bind(listen).await?;
      tracing::info!(address = %listener.local_addr()?, "server listening");
      orchester_server::serve(server, listener, shutdown.wait()).await
    }
    Command::User(command) => run_user(config, command).await,
    Command::Backup { path } => orchester_server::backup_existing(config.data_dir, path).await,
  }
}

async fn run_user(config: Config, command: UserCommand) -> Result<(), ServerError> {
  let server = Server::open(config, Arc::new(SystemClock)).await?;
  match command {
    UserCommand::Create { username, admin } => {
      let created = server
        .host_create_user(CreateUser {
          display_name: username.clone(),
          username,
          admin,
        })
        .await?;
      // The specification permits this one-time host-command output, never a log record.
      writeln!(io::stdout().lock(), "{}", created.initial_password.expose())?;
      Ok(())
    }
    UserCommand::Reset { username } => {
      let initial = server.host_reset_password(username).await?;
      writeln!(io::stdout().lock(), "{}", initial.expose())?;
      Ok(())
    }
  }
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
  let command = match args.as_slice() {
    [] => Command::Serve,
    [serve] if serve == "serve" => Command::Serve,
    [backup, path] if backup == "backup" => Command::Backup { path: path.into() },
    [user, create, flag, username] if user == "user" && create == "create" && flag == "--admin" => {
      Command::User(UserCommand::Create {
        username: username.clone(),
        admin: true,
      })
    }
    [user, create, username] if user == "user" && create == "create" => Command::User(UserCommand::Create {
      username: username.clone(),
      admin: false,
    }),
    [user, reset, username] if user == "user" && reset == "reset-password" => Command::User(UserCommand::Reset {
      username: username.clone(),
    }),
    _ => return Err(usage()),
  };
  Ok((command, path))
}

fn usage() -> ServerError {
  ServerError::Configuration(
    "usage: orchester-server [--config server.toml] [serve | user create [--admin] <username> | user reset-password <username> | backup <path>]",
  )
}
