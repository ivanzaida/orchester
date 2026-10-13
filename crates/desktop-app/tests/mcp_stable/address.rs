//! The fixed address: the port of the settings across restarts, a taken port said and no other taken, off and on,
//! and the loopback `Host` the guard asks for.

use super::*;

#[test]
fn app_mcp_listens_on_the_port_set_across_restarts_and_writes_no_discovery_file() {
  let port = free_port();
  let installed = Installed::start(port);
  assert_eq!(
    installed.server.status().server,
    ServerState::Listening {
      port: std::num::NonZeroU16::new(port).expect("a port"),
    }
  );
  assert_eq!(installed.server.discovery_path(), None, "no discovery file");
  let token = installed.token();
  assert_eq!(knocks(port, &token), 200);
  let installed = installed.restart();
  assert_eq!(installed.port(), port, "the same port after a restart");
  assert_eq!(installed.token(), token, "the same token after a restart");
  assert_eq!(knocks(port, &token), 200);
}

#[test]
fn app_mcp_default_port_is_47821() {
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("a session");
  assert_eq!(workspace.mcp_port().expect("read").get(), 47821);
  assert!(workspace.mcp_server_enabled().expect("read"));
  launch.end();
}

#[test]
fn app_mcp_taken_port_is_said_and_no_other_port_is_taken() {
  let holder = TcpListener::bind(("127.0.0.1", 0)).expect("another program's port");
  let port = holder.local_addr().expect("its address").port();
  let installed = Installed::start(port);
  assert_eq!(
    installed.server.status().server,
    ServerState::PortTaken {
      port: McpPort::new(u32::from(port)).expect("a port"),
    }
  );
  assert_eq!(installed.server.port(), None, "it does not move to another port");
  drop(holder);
  installed.server.try_again();
  assert_eq!(installed.port(), port, "once the port is free, Try again listens there");
}

#[test]
fn app_mcp_use_port_moves_the_server_and_keeps_the_port() {
  let installed = Installed::start(free_port());
  let token = installed.token();
  let next = free_port();
  installed
    .server
    .use_port(McpPort::new(u32::from(next)).expect("a port"))
    .expect("the port is kept");
  assert_eq!(installed.port(), next);
  assert_eq!(installed.workspace.mcp_port().expect("read").get(), next);
  assert_eq!(knocks(next, &token), 200);
  let installed = installed.restart();
  assert_eq!(installed.port(), next, "the new port after a restart");
}

#[test]
fn app_mcp_off_listens_nowhere_and_keeps_the_token() {
  let port = free_port();
  let installed = Installed::start(port);
  let token = installed.token();
  installed.server.set_enabled(false).expect("turned off");
  assert_eq!(installed.server.status().server, ServerState::Off);
  assert!(
    std::net::TcpStream::connect(("127.0.0.1", port)).is_err(),
    "nothing listens"
  );
  assert!(!installed.workspace.mcp_server_enabled().expect("read"));
  let installed = installed.restart();
  assert_eq!(
    installed.server.status().server,
    ServerState::Off,
    "off after a restart"
  );
  installed.server.set_enabled(true).expect("turned on");
  assert_eq!(installed.port(), port);
  assert_eq!(installed.token(), token, "the token stayed stored");
}

#[test]
fn app_mcp_refuses_a_host_that_is_not_its_loopback_address() {
  let installed = Installed::start(free_port());
  let (port, auth) = (installed.port(), bearer(&installed.token()));
  let headers = [("Authorization", auth.as_str())];
  for host in [
    format!("evil.example:{port}"),
    format!("127.0.0.1:{}", port.wrapping_add(1)),
    format!("127.0.0.1.evil.example:{port}"),
    "127.0.0.1".to_owned(),
    format!("[::1]:{port}"),
    format!("127.0.0.1:+{port}"),
  ] {
    assert_eq!(post_as(port, &host, &headers, &knock()).status, 403, "{host}");
  }
  for host in [
    format!("127.0.0.1:{port}"),
    format!("localhost:{port}"),
    format!("LOCALHOST:{port}"),
  ] {
    assert_eq!(post_as(port, &host, &headers, &knock()).status, 200, "{host}");
  }
}

#[test]
fn app_mcp_control_of_a_qa_copy_is_refused() {
  let data = Data::new();
  let launch = data.launch();
  let mut tree = Tree::new();
  let link = WindowLink::default();
  let settings = McpSettings::with_window_control(false);
  let app = App::new();
  let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
  let config = mcp::config(settings, &link, &appearance, None, app.i18n());
  let server = AppMcp::start(
    &mut tree,
    data.runtime.handle(),
    McpMode::Qa(data.dir.path().join("mcp")),
    (config, settings),
    &link.input_gate(),
  );
  assert!(server.discovery_path().is_some(), "a QA copy writes its discovery file");
  assert!(matches!(server.set_enabled(false), Err(ControlError::NotInstallation)));
  assert!(matches!(server.rotate_token(), Err(ControlError::NotInstallation)));
  server.shutdown();
  launch.end();
}
