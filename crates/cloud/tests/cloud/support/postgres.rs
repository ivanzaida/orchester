//! The PostgreSQL the tests run against: `ORCHESTER_CLOUD_TEST_DATABASE_URL` when set (a server URL whose user may
//! create databases), otherwise a throwaway container of the official image on 127.0.0.1, removed when the run ends.
//! Without either the run fails; it never passes without a database.

use std::{process::Command, time::Duration};

use uuid::Uuid;

const IMAGE: &str = "postgres:18-alpine";

pub struct Postgres {
  /// `postgres://user:password@host:port` without a database.
  base: String,
  container: Option<String>,
  pub password: Option<String>,
}

impl Postgres {
  pub async fn start() -> Result<Self, String> {
    if let Ok(url) = std::env::var("ORCHESTER_CLOUD_TEST_DATABASE_URL") {
      let base = url.trim_end_matches('/').to_owned();
      let server = Self {
        base,
        container: None,
        password: None,
      };
      server.wait_ready(Duration::from_secs(10)).await?;
      return Ok(server);
    }
    let name = format!("orchester-cloud-test-{}", Uuid::new_v4().simple());
    let password = Uuid::new_v4().simple().to_string();
    let started = Command::new("docker")
      .args(["run", "-d", "--rm", "--name", &name, "-p", "127.0.0.1::5432"])
      .args(["-e", &format!("POSTGRES_PASSWORD={password}"), IMAGE])
      .output()
      .map_err(|error| format!("cannot run docker ({error}); set ORCHESTER_CLOUD_TEST_DATABASE_URL instead"))?;
    if !started.status.success() {
      return Err(format!(
        "docker could not start {IMAGE}: {}; set ORCHESTER_CLOUD_TEST_DATABASE_URL instead",
        String::from_utf8_lossy(&started.stderr).trim()
      ));
    }
    let mut server = Self {
      base: String::new(),
      container: Some(name.clone()),
      password: Some(password.clone()),
    };
    let port = Command::new("docker")
      .args(["port", &name, "5432/tcp"])
      .output()
      .map_err(|error| format!("cannot ask docker for the port: {error}"))?;
    let address = String::from_utf8_lossy(&port.stdout)
      .lines()
      .find(|line| line.starts_with("127.0.0.1:"))
      .map(str::to_owned)
      .ok_or_else(|| "docker did not publish the PostgreSQL port on 127.0.0.1".to_owned())?;
    server.base = format!("postgres://postgres:{password}@{address}");
    server.wait_ready(Duration::from_secs(60)).await?;
    Ok(server)
  }

  /// The image initialises the cluster on a socket first and opens TCP only when it is ready.
  async fn wait_ready(&self, deadline: Duration) -> Result<(), String> {
    let until = tokio::time::Instant::now() + deadline;
    while tokio::time::Instant::now() < until {
      let url = self.url("postgres");
      let connect = tokio_postgres::connect(&url, tokio_postgres::NoTls);
      if let Ok(Ok((client, connection))) = tokio::time::timeout(Duration::from_secs(2), connect).await {
        let driver = tokio::spawn(connection);
        let ready = client.simple_query("SELECT 1").await.is_ok();
        drop(client);
        driver.abort();
        if ready {
          return Ok(());
        }
      }
      tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(format!(
      "PostgreSQL did not accept connections within {} seconds",
      deadline.as_secs()
    ))
  }

  pub fn url(&self, database: &str) -> String {
    format!("{}/{database}?sslmode=disable", self.base)
  }

  pub async fn admin(&self) -> tokio_postgres::Client {
    self.database("postgres").await
  }

  /// A plain connection to one database, to look at what the cloud stored.
  pub async fn database(&self, name: &str) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(&self.url(name), tokio_postgres::NoTls)
      .await
      .expect("connect to the test database");
    tokio::spawn(async move {
      if connection.await.is_err() {
        eprintln!("a test database connection ended with an error");
      }
    });
    client
  }

  /// A fresh, empty database for one test.
  pub async fn create_database(&self) -> String {
    let name = format!("cloud_test_{}", Uuid::new_v4().simple());
    self
      .admin()
      .await
      .batch_execute(&format!("CREATE DATABASE {name}"))
      .await
      .expect("create the test's database");
    name
  }

  pub async fn drop_database(&self, name: &str) {
    self
      .admin()
      .await
      .batch_execute(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
      .await
      .expect("drop the test's database");
  }

  pub fn stop(&self) {
    let Some(name) = &self.container else {
      return;
    };
    match Command::new("docker").args(["rm", "-f", name]).output() {
      Ok(output) if output.status.success() => {}
      _ => eprintln!("could not remove the test container {name}; remove it with `docker rm -f {name}`"),
    }
  }
}
