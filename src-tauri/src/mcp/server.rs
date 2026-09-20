use crate::mcp::protocol;
use crate::state::AppState;
use serde_json::{json, Value};
use std::io::Read;
use std::thread;
use std::time::Instant;
use tauri::{AppHandle, Manager};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

pub const MCP_ADDRESS: &str = "http://127.0.0.1:47831/mcp";
const LISTEN_ADDRESS: &str = "127.0.0.1:47831";
const MAX_BODY_BYTES: u64 = 1024 * 1024;

pub fn start_mcp_server(app: AppHandle) {
    thread::spawn(move || match Server::http(LISTEN_ADDRESS) {
        Ok(server) => {
            set_status(&app, true, None);
            tracing::info!(address = MCP_ADDRESS, "local MCP server started");
            for request in server.incoming_requests() {
                handle_request(request, &app);
            }
        }
        Err(error) => {
            tracing::error!(error_kind = %error, "failed to bind local MCP server");
            set_status(&app, false, Some("本地端口 47831 不可用".into()));
        }
    });
}

fn set_status(app: &AppHandle, running: bool, error: Option<String>) {
    let state = app.state::<AppState>();
    match state.mcp.lock() {
        Ok(mut status) => {
            status.running = running;
            status.error = error;
        }
        Err(_) => tracing::error!("MCP status lock poisoned"),
    };
}

fn handle_request(mut request: Request, app: &AppHandle) {
    let started = Instant::now();
    let path = request.url().split('?').next().unwrap_or(request.url());
    if path == "/health" && request.method() == &Method::Get {
        let state = app.state::<AppState>();
        let unlocked = state.database_unlocked().unwrap_or(false);
        respond_json(
            request,
            StatusCode(200),
            json!({"running":true,"database_unlocked":unlocked}),
        );
        return;
    }
    if path != "/mcp" {
        respond_json(request, StatusCode(404), json!({"error":"not found"}));
        return;
    }
    if request.method() != &Method::Post {
        respond_json(
            request,
            StatusCode(405),
            json!({"error":"method not allowed"}),
        );
        return;
    }
    if !origin_allowed(&request) {
        respond_json(request, StatusCode(403), json!({"error":"origin denied"}));
        return;
    }
    let declared_length = request.body_length().unwrap_or(0) as u64;
    if declared_length > MAX_BODY_BYTES {
        respond_json(
            request,
            StatusCode(413),
            json!({"error":"request too large"}),
        );
        return;
    }
    let mut body = Vec::new();
    if request
        .as_reader()
        .take(MAX_BODY_BYTES + 1)
        .read_to_end(&mut body)
        .is_err()
    {
        respond_json(request, StatusCode(400), json!({"error":"invalid body"}));
        return;
    }
    if body.len() as u64 > MAX_BODY_BYTES {
        respond_json(
            request,
            StatusCode(413),
            json!({"error":"request too large"}),
        );
        return;
    }
    let value: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => {
            respond_json(
                request,
                StatusCode(400),
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"无效 JSON"}}),
            );
            return;
        }
    };
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("invalid")
        .to_string();
    match protocol::handle_json_rpc(value, app) {
        Some(reply) => respond_json(request, StatusCode(200), reply),
        None => respond_empty(request, StatusCode(202)),
    }
    tracing::info!(
        method,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "MCP request handled"
    );
}

fn origin_allowed(request: &Request) -> bool {
    let origin = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Origin"))
        .map(|header| header.value.as_str());
    match origin {
        None => true,
        Some(value) => {
            value == "null"
                || value.starts_with("http://127.0.0.1")
                || value.starts_with("http://localhost")
                || value.starts_with("tauri://localhost")
        }
    }
}

fn respond_json(request: Request, status: StatusCode, value: Value) {
    let header = Header::from_bytes("Content-Type", "application/json").expect("valid header");
    let response = Response::from_string(value.to_string())
        .with_status_code(status)
        .with_header(header);
    if let Err(error) = request.respond(response) {
        tracing::warn!(error_kind = %error, "failed to send MCP response");
    }
}

fn respond_empty(request: Request, status: StatusCode) {
    if let Err(error) = request.respond(Response::empty(status)) {
        tracing::warn!(error_kind = %error, "failed to send MCP response");
    }
}
