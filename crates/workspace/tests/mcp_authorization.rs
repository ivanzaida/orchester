//! A remote MCP server's authorization as the Connections screen edits it: each scheme from the fields to a server
//! and back, why fields cannot be saved, and which changes widen what agents may do. Run with `cargo test -p
//! orchester-workspace --test mcp_authorization mcp`.

use orchester_core::{
  connection::mcp::{BasicUsername, McpAuthorization, McpHeaderName, McpServer, McpTransport},
  id::{ConnectionId, SecretId},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  time::Timestamp,
};
use orchester_workspace::connections::mcp::{
  AuthorizationChoice, AuthorizationText, McpServerText, TransportChoice, widenings,
};

fn secret(name: &str) -> Secret {
  let new = NewSecret {
    name: SecretName::new(name).expect("valid name"),
    kind: SecretKind::AccessToken,
    environment: Environment::Production,
  };
  Secret::new(SecretId::generate(), new, Timestamp::from_unix_millis(1))
}

fn remote(authorization: AuthorizationText) -> McpServerText {
  McpServerText {
    name: "knowledge".to_owned(),
    transport: TransportChoice::Http,
    url: "https://mcp.example.dev/mcp".to_owned(),
    authorization,
    ..McpServerText::new_server()
  }
}

fn fields(choice: AuthorizationChoice, secret: Option<SecretId>, username: &str, header: &str) -> AuthorizationText {
  AuthorizationText {
    choice,
    secret,
    username: username.to_owned(),
    header: header.to_owned(),
  }
}

fn server(text: &McpServerText, secrets: &[Secret]) -> McpServer {
  let draft = text.draft(&[], secrets).expect("valid fields");
  McpServer::new(
    ConnectionId::generate(),
    draft.name,
    draft.transport,
    draft.environment,
    Timestamp::from_unix_millis(1),
  )
  .expect("a valid server")
}

fn authorization(server: &McpServer) -> &McpAuthorization {
  let McpTransport::Http(http) = server.transport() else {
    panic!("a remote server");
  };
  http.authorization()
}

#[test]
fn mcp_authorization_fields_become_each_scheme_and_read_back() {
  let token = secret("REMOTE_TOKEN");
  let secrets = [token.clone()];
  let id = Some(token.id());
  let cases = [
    (fields(AuthorizationChoice::None, None, "", ""), McpAuthorization::None),
    (
      fields(AuthorizationChoice::Bearer, id, "", ""),
      McpAuthorization::Bearer { token: token.id() },
    ),
    (
      fields(AuthorizationChoice::Basic, id, "deploy", ""),
      McpAuthorization::Basic {
        username: BasicUsername::new("deploy").unwrap(),
        password: token.id(),
      },
    ),
    (
      fields(AuthorizationChoice::BasicCredential, id, "", ""),
      McpAuthorization::BasicCredential { credential: token.id() },
    ),
    (
      fields(AuthorizationChoice::Header, id, "", "X-API-Key"),
      McpAuthorization::Header {
        name: McpHeaderName::new("X-API-Key").unwrap(),
        value: token.id(),
      },
    ),
  ];
  for (entered, expected) in cases {
    let text = remote(entered.clone());
    let saved = server(&text, &secrets);
    assert_eq!(authorization(&saved), &expected);
    assert_eq!(McpServerText::of(&saved, &secrets), text, "{expected:?} reads back");
  }
}

#[test]
fn mcp_authorization_fields_say_why_they_cannot_be_saved() {
  let token = secret("REMOTE_TOKEN");
  let secrets = [token.clone()];
  let key = |message: Option<orchester_workspace::messages::Message>| message.map(|message| message.key);
  let errors = remote(fields(AuthorizationChoice::Basic, None, " ", ""))
    .draft(&[], &secrets)
    .expect_err("no user name, no password");
  assert_eq!(key(errors.username), Some("connections.mcp.errors.usernameRequired"));
  assert_eq!(key(errors.secret), Some("connections.mcp.errors.secretRequired"));
  let errors = remote(fields(AuthorizationChoice::Basic, Some(token.id()), "a:b", ""))
    .draft(&[], &secrets)
    .expect_err("a colon in the user name");
  assert_eq!(key(errors.username), Some("connections.mcp.errors.usernameColon"));
  assert_eq!(errors.secret, None);
  let errors = remote(fields(
    AuthorizationChoice::Header,
    Some(token.id()),
    "",
    "Content-Type",
  ))
  .draft(&[], &secrets)
  .expect_err("a header the CLI sets");
  assert_eq!(key(errors.header), Some("connections.mcp.errors.headerReserved"));
  let errors = remote(fields(AuthorizationChoice::Header, Some(token.id()), "", "X API"))
    .draft(&[], &secrets)
    .expect_err("not a header name");
  assert_eq!(key(errors.header), Some("connections.mcp.errors.headerInvalid"));
  let errors = remote(fields(
    AuthorizationChoice::BasicCredential,
    Some(SecretId::generate()),
    "",
    "",
  ))
  .draft(&[], &secrets)
  .expect_err("a secret the project does not list");
  assert_eq!(key(errors.secret), Some("connections.mcp.errors.secretGone"));
  // Fields a scheme does not use keep what was typed and are not checked.
  let none = remote(fields(
    AuthorizationChoice::None,
    Some(SecretId::generate()),
    "a:b",
    "Host",
  ));
  assert!(none.draft(&[], &secrets).is_ok());
}

#[test]
fn mcp_authorization_changes_that_send_a_secret_widen() {
  let (token, other) = (secret("REMOTE_TOKEN"), secret("OTHER_TOKEN"));
  let secrets = [token.clone(), other.clone()];
  let bearer = remote(fields(AuthorizationChoice::Bearer, Some(token.id()), "", ""));
  let stored = server(&bearer, &secrets);
  let keys = |text: McpServerText| -> Vec<&'static str> {
    let draft = text.draft(&[], &secrets).expect("valid fields");
    widenings(Some(&stored), &draft)
      .iter()
      .map(|reason| reason.key)
      .collect()
  };
  assert!(keys(bearer).is_empty(), "nothing changed");
  assert!(
    keys(remote(AuthorizationText::default())).is_empty(),
    "dropping the authorization narrows"
  );
  for wider in [
    fields(AuthorizationChoice::Bearer, Some(other.id()), "", ""),
    fields(AuthorizationChoice::Basic, Some(token.id()), "deploy", ""),
    fields(AuthorizationChoice::Header, Some(token.id()), "", "X-API-Key"),
  ] {
    assert_eq!(
      keys(remote(wider.clone())),
      ["appMcp.widens.mcpAuthorization"],
      "{wider:?}"
    );
  }
  let basic = server(
    &remote(fields(AuthorizationChoice::Basic, Some(token.id()), "deploy", "")),
    &secrets,
  );
  let draft = remote(fields(AuthorizationChoice::Basic, Some(token.id()), "admin", ""))
    .draft(&[], &secrets)
    .expect("valid fields");
  assert_eq!(
    widenings(Some(&basic), &draft)
      .iter()
      .map(|reason| reason.key)
      .collect::<Vec<_>>(),
    ["appMcp.widens.mcpAuthorization"],
    "another user name sends the password for another account"
  );
}
