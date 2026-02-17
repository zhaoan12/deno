// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU32;

use parking_lot::Mutex;

use super::BrokerResponse;
use super::utc_now_rfc3339;
use crate::ipc_pipe::IpcPipe;

// TODO(bartlomieju): currently randomly selected exit code, it should
// be documented
static BROKER_EXIT_CODE: i32 = 87;

static PERMISSION_BROKER: OnceLock<PermissionBroker> = OnceLock::new();
static PID: OnceLock<u32> = OnceLock::new();

pub fn set_broker(broker: PermissionBroker) {
  assert!(PERMISSION_BROKER.set(broker).is_ok());
  assert!(PID.set(std::process::id()).is_ok());
}

pub fn has_broker() -> bool {
  PERMISSION_BROKER.get().is_some()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PermissionBrokerRequest<'a> {
  v: u32,
  pid: u32,
  id: u32,
  datetime: String,
  permission: &'a str,
  value: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum PermissionBrokerDecision {
  Allow,
  Deny,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionBrokerResponse {
  id: u32,
  result: PermissionBrokerDecision,
  reason: Option<String>,
}

pub struct PermissionBroker {
  stream: Mutex<IpcPipe>,
  next_id: AtomicU32,
}

impl PermissionBroker {
  pub fn new(socket_path: impl Into<PathBuf>) -> Self {
    let socket_path = socket_path.into();
    let stream = match IpcPipe::connect(&socket_path) {
      Ok(s) => s,
      Err(err) => {
        log::error!("Failed to create permission broker: {:?}", err);
        std::process::exit(BROKER_EXIT_CODE);
      }
    };
    Self {
      stream: Mutex::new(stream),
      next_id: std::sync::atomic::AtomicU32::new(1),
    }
  }

  fn check(
    &self,
    permission: &str,
    stringified_value: Option<String>,
  ) -> std::io::Result<BrokerResponse> {
    let mut stream = self.stream.lock();
    let id = self
      .next_id
      .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let request = PermissionBrokerRequest {
      v: 1,
      pid: *PID.get().unwrap(),
      id,
      datetime: utc_now_rfc3339(false),
      permission,
      value: stringified_value,
    };

    let msg = serialize_broker_request(&request);
    log::trace!("-> broker req   {}", msg);
    stream.write_all(msg.as_bytes())?;

    // Read response using line reader
    let mut reader = BufReader::new(&mut *stream);
    let mut response_line = String::new();
    let bytes_read = reader.read_line(&mut response_line)?;
    if bytes_read == 0 {
      return Err(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "Permission broker closed the pipe before sending a response",
      ));
    }

    let response =
      serde_json::from_str::<PermissionBrokerResponse>(response_line.trim())
        .map_err(|err| {
          std::io::Error::other(format!(
            "Permission broker returned invalid JSON response: {err}",
          ))
        })?;

    log::trace!("<- broker resp  {:?}", response);

    if response.id != id {
      return Err(std::io::Error::other(format!(
        "Permission broker response ID mismatch (expected {id}, got {})",
        response.id
      )));
    }

    let prompt_response = match response.result {
      PermissionBrokerDecision::Allow => BrokerResponse::Allow,
      PermissionBrokerDecision::Deny => BrokerResponse::Deny {
        message: response.reason,
      },
    };

    Ok(prompt_response)
  }
}

pub fn maybe_check_with_broker(
  name: &str,
  stringified_value_fn: impl Fn() -> Option<String>,
) -> Option<BrokerResponse> {
  let broker = PERMISSION_BROKER.get()?;

  let resp = match broker.check(name, stringified_value_fn()) {
    Ok(resp) => resp,
    Err(err) => {
      log::error!("{:?}", err);
      std::process::exit(BROKER_EXIT_CODE);
    }
  };
  Some(resp)
}

fn serialize_broker_request(request: &PermissionBrokerRequest<'_>) -> String {
  format!("{}\n", serde_json::to_string(request).unwrap())
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn test_serialize_broker_request() {
    let request = PermissionBrokerRequest {
      v: 1,
      pid: 42,
      id: 7,
      datetime: "2026-02-17T10:00:00Z".to_string(),
      permission: "read",
      value: Some("/tmp/file.txt".to_string()),
    };

    let serialized = serialize_broker_request(&request);
    assert!(serialized.ends_with('\n'));
    assert_eq!(
      serde_json::from_str::<serde_json::Value>(serialized.trim()).unwrap(),
      json!({
        "v": 1,
        "pid": 42,
        "id": 7,
        "datetime": "2026-02-17T10:00:00Z",
        "permission": "read",
        "value": "/tmp/file.txt"
      })
    );
  }
}
