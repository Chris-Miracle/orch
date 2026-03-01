use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{io_err, DaemonError};
use crate::paths::socket_path;

/// JSON newline-delimited request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonRequest {
    pub cmd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codebase: Option<String>,
}

/// JSON newline-delimited response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DaemonResponse {
    pub fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// Send one JSON request to the daemon socket and return one response.
pub fn send_request(home: &Path, request: &DaemonRequest) -> Result<DaemonResponse, DaemonError> {
    let socket = socket_path(home);
    if !socket.exists() {
        return Err(DaemonError::DaemonNotRunning { socket });
    }

    let mut stream = UnixStream::connect(&socket).map_err(|err| {
        if matches!(
            err.kind(),
            std::io::ErrorKind::NotFound
                | std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::ConnectionReset
        ) {
            DaemonError::DaemonNotRunning {
                socket: socket.clone(),
            }
        } else {
            io_err(&socket, err)
        }
    })?;

    let payload = serde_json::to_string(request)?;
    stream
        .write_all(payload.as_bytes())
        .map_err(|e| io_err(&socket, e))?;
    stream.write_all(b"\n").map_err(|e| io_err(&socket, e))?;
    stream.flush().map_err(|e| io_err(&socket, e))?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let read = reader
        .read_line(&mut line)
        .map_err(|e| io_err(&socket, e))?;
    if read == 0 {
        return Err(DaemonError::Protocol(
            "daemon closed connection before responding".to_string(),
        ));
    }

    let response: DaemonResponse = serde_json::from_str(line.trim_end())?;
    Ok(response)
}

pub fn request_status(home: &Path) -> Result<Value, DaemonError> {
    let request = DaemonRequest {
        cmd: "status".to_string(),
        codebase: None,
    };

    let mut last_not_running: Option<DaemonError> = None;
    for attempt in 0..5 {
        match send_request(home, &request) {
            Ok(response) => return response_into_data(response),
            Err(err @ DaemonError::DaemonNotRunning { .. }) => {
                last_not_running = Some(err);
                if attempt < 4 {
                    sleep(Duration::from_millis(100));
                    continue;
                }
            }
            Err(err) => return Err(err),
        }
    }

    Err(last_not_running.unwrap_or_else(|| {
        DaemonError::Protocol("daemon status retry loop exited unexpectedly".to_string())
    }))
}

pub fn request_stop(home: &Path) -> Result<(), DaemonError> {
    let response = send_request(
        home,
        &DaemonRequest {
            cmd: "stop".to_string(),
            codebase: None,
        },
    )?;
    response_into_data(response).map(|_| ())
}

pub fn request_sync(home: &Path, codebase: Option<String>) -> Result<Value, DaemonError> {
    let response = send_request(
        home,
        &DaemonRequest {
            cmd: "sync".to_string(),
            codebase,
        },
    )?;
    response_into_data(response)
}

fn response_into_data(response: DaemonResponse) -> Result<Value, DaemonError> {
    if response.ok {
        Ok(response.data.unwrap_or(Value::Null))
    } else {
        Err(DaemonError::Protocol(
            response
                .error
                .unwrap_or_else(|| "unknown daemon error".to_string()),
        ))
    }
}
