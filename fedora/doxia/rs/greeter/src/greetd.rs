// greetd's IPC (greetd-ipc(7)): JSON messages over $GREETD_SOCK, each
// preceded by its length as a native-endian u32. One connection per sign-in
// attempt; greetd drops the half-made session when the connection closes.

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

#[derive(Debug, PartialEq)]
pub enum Outcome {
  // greetd starts the session once the greeter exits.
  Started,
  // A wrong password (or an account PAM turned down).
  Refused,
  // Anything else: no greetd, a PAM error, a session that would not start.
  Failed(String),
}

struct Client(UnixStream);

impl Client {
  fn request(&mut self, message: &Value) -> std::io::Result<Value> {
    let body = serde_json::to_vec(message)?;
    self.0.write_all(&(body.len() as u32).to_ne_bytes())?;
    self.0.write_all(&body)?;
    let mut len = [0u8; 4];
    self.0.read_exact(&mut len)?;
    let mut reply = vec![0u8; u32::from_ne_bytes(len) as usize];
    self.0.read_exact(&mut reply)?;
    Ok(serde_json::from_slice(&reply)?)
  }

  fn cancel(&mut self) {
    let _ = self.request(&json!({"type": "cancel_session"}));
  }
}

fn description(reply: &Value) -> String {
  reply["description"].as_str().unwrap_or("erro desconhecido").to_string()
}

// The whole sign-in, blocking: authenticate `user` with `password`, then ask
// for `cmd` with `env`. Only one password prompt is answered; a second one
// (an expired password, a one-time code) is not something this screen asks
// for, so it counts as refused.
pub fn login(user: &str, password: &str, cmd: &[String], env: &[String]) -> Outcome {
  let Some(path) = std::env::var_os("GREETD_SOCK") else { return Outcome::Failed("greetd não está rodando".into()) };
  let mut client = match UnixStream::connect(path) {
    Ok(s) => Client(s),
    Err(e) => return Outcome::Failed(e.to_string()),
  };
  let mut answered = false;
  let mut reply = client.request(&json!({"type": "create_session", "username": user}));
  loop {
    let r = match reply {
      Ok(r) => r,
      Err(e) => return Outcome::Failed(e.to_string()),
    };
    match r["type"].as_str() {
      Some("success") => break,
      Some("error") => {
        client.cancel();
        return if r["error_type"] == "auth_error" { Outcome::Refused } else { Outcome::Failed(description(&r)) };
      }
      Some("auth_message") => {
        let response = match r["auth_message_type"].as_str() {
          Some("secret") if !answered => {
            answered = true;
            Value::from(password)
          }
          Some("secret" | "visible") => {
            client.cancel();
            return Outcome::Refused;
          }
          // Information and errors from PAM only want an acknowledgement.
          _ => Value::Null,
        };
        reply = client.request(&json!({"type": "post_auth_message_response", "response": response}));
      }
      _ => {
        client.cancel();
        return Outcome::Failed(format!("resposta inesperada do greetd: {r}"));
      }
    }
  }
  match client.request(&json!({"type": "start_session", "cmd": cmd, "env": env})) {
    Ok(r) if r["type"] == "success" => Outcome::Started,
    Ok(r) => {
      client.cancel();
      Outcome::Failed(description(&r))
    }
    Err(e) => Outcome::Failed(e.to_string()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::os::unix::net::UnixListener;

  // A greetd that asks for the password once and accepts only "certa".
  fn fake(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("greetd.sock");
    let listener = UnixListener::bind(&path).unwrap();
    std::thread::spawn(move || {
      for stream in listener.incoming() {
        let mut s = stream.unwrap();
        fn read(s: &mut UnixStream) -> Option<Value> {
          let mut len = [0u8; 4];
          s.read_exact(&mut len).ok()?;
          let mut body = vec![0u8; u32::from_ne_bytes(len) as usize];
          s.read_exact(&mut body).ok()?;
          serde_json::from_slice(&body).ok()
        }
        while let Some(m) = read(&mut s) {
          let reply = match m["type"].as_str().unwrap() {
            "create_session" => json!({"type": "auth_message", "auth_message_type": "secret", "auth_message": "Password: "}),
            "post_auth_message_response" if m["response"] == "certa" => json!({"type": "success"}),
            "post_auth_message_response" => json!({"type": "error", "error_type": "auth_error", "description": "no"}),
            "start_session" => json!({"type": "success"}),
            _ => json!({"type": "success"}),
          };
          let body = serde_json::to_vec(&reply).unwrap();
          let _ = s.write_all(&(body.len() as u32).to_ne_bytes());
          let _ = s.write_all(&body);
        }
      }
    });
    path
  }

  #[test]
  fn conversation() {
    let dir = std::env::temp_dir().join(format!("doxia-greeter-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sock = fake(&dir);
    unsafe { std::env::set_var("GREETD_SOCK", &sock) };
    let cmd = vec!["true".to_string()];
    assert_eq!(login("ana", "errada", &cmd, &[]), Outcome::Refused);
    assert_eq!(login("ana", "certa", &cmd, &[]), Outcome::Started);
    unsafe { std::env::set_var("GREETD_SOCK", dir.join("none")) };
    assert!(matches!(login("ana", "certa", &cmd, &[]), Outcome::Failed(_)));
    let _ = std::fs::remove_dir_all(&dir);
  }
}
