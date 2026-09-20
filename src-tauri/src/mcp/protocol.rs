use serde_json::{json, Value};
use tauri::AppHandle;

pub fn handle_json_rpc(body: Value, app: &AppHandle) -> Option<Value> {
    let id = body.get("id").cloned();
    let method = body.get("method").and_then(Value::as_str);
    if id.is_none() {
        return None;
    }
    let id = id.unwrap_or(Value::Null);
    let response = match method {
        Some("initialize") => {
            let protocol_version = body
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2025-06-18");
            ok(
                id,
                json!({
                    "protocolVersion":protocol_version,
                    "capabilities":{"tools":{"listChanged":false}},
                    "serverInfo":{"name":"solo-cost","version":env!("CARGO_PKG_VERSION")},
                    "instructions":"只读查询收入、到手、销售分成、人工收入和剩余利润。"
                }),
            )
        }
        Some("ping") => ok(id, json!({})),
        Some("tools/list") => ok(id, json!({"tools":super::tools::definitions()})),
        Some("tools/call") => {
            let Some(name) = body.pointer("/params/name").and_then(Value::as_str) else {
                return Some(error(id, -32602, "工具名称无效"));
            };
            let arguments = body
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            ok(id, super::tools::call(app, name, arguments))
        }
        Some(_) => error(id, -32601, "不支持的方法"),
        None => error(id, -32600, "无效的 JSON-RPC 请求"),
    };
    Some(response)
}

fn ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc":"2.0",
        "id":id,
        "error":{"code":code,"message":message}
    })
}

