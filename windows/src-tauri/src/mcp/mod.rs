mod tools;

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

use crate::state::AppState;
use tools::{call_tool, tool_definitions, tool_result_error, tool_result_text};

pub const MCP_PORT: u16 = 19789;

type SharedEditor = AppState;

#[derive(Clone)]
struct McpAppState {
    editor: SharedEditor,
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

pub fn start_mcp_server(editor: SharedEditor) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .expect("tokio runtime");
        rt.block_on(async {
            let app_state = McpAppState { editor };
            let app = Router::new()
                .route("/", get(sse_connect).post(handle_mcp))
                .route("/mcp", get(sse_connect).post(handle_mcp))
                .route(
                    "/.well-known/oauth-protected-resource",
                    get(oauth_resource),
                )
                .layer(CorsLayer::permissive())
                .with_state(app_state);

            let addr = SocketAddr::from(([127, 0, 0, 1], MCP_PORT));
            match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => {
                    if let Err(e) = axum::serve(listener, app).await {
                        eprintln!("MCP server stopped: {e}");
                    }
                }
                Err(e) => eprintln!("MCP bind failed on port {MCP_PORT}: {e}"),
            }
        });
    });
}

async fn oauth_resource() -> impl IntoResponse {
    Json(json!({ "resource": format!("http://127.0.0.1:{MCP_PORT}") }))
}

async fn sse_connect() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/event-stream")],
        ": connected\n\n",
    )
}

async fn handle_mcp(
    State(state): State<McpAppState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let accept = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let request: JsonRpcRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            return json_response(
                None,
                None,
                Some(JsonRpcError {
                    code: -32700,
                    message: format!("Parse error: {e}"),
                }),
            );
        }
    };

    if request.id.is_none()
        && (request.method.starts_with("notifications/") || request.method == "initialized")
    {
        return StatusCode::ACCEPTED.into_response();
    }

    let id = request.id.clone().unwrap_or(Value::Null);

    let result = if request.method == "tools/call" {
        let params = match request.params.clone() {
            Some(p) => p,
            None => {
                return json_response(
                    Some(id),
                    None,
                    Some(JsonRpcError {
                        code: -32602,
                        message: "missing params".into(),
                    }),
                );
            }
        };
        let editor = state.editor.clone();
        match tokio::task::spawn_blocking(move || dispatch_tool_call(&editor, &params)).await {
            Ok(inner) => inner,
            Err(e) => Err(format!("tool dispatch failed: {e}")),
        }
    } else {
        dispatch(&state, &request.method, request.params.as_ref())
    };

    if accept.contains("text/event-stream") {
        let payload = match &result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": e } }),
        };
        let data = format!("data: {payload}\n\n");
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .body(Body::from(data))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }

    match result {
        Ok(r) => json_response(Some(id), Some(r), None),
        Err(msg) => json_response(
            Some(id),
            None,
            Some(JsonRpcError {
                code: -32000,
                message: msg,
            }),
        ),
    }
}

fn dispatch_tool_call(editor: &AppState, params: &Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing tool name")?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    match call_tool(editor, name, &args) {
        Ok(v) => Ok(tool_result_text(v)),
        Err(e) => Ok(tool_result_error(&e)),
    }
}

fn dispatch(state: &McpAppState, method: &str, params: Option<&Value>) -> Result<Value, String> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": { "subscribe": false, "listChanged": false }
            },
            "serverInfo": { "name": "palmier-pro", "version": "1.0.0" },
            "instructions": "Palmier Pro Windows MCP server. Open a project in the app before editing."
        })),
        "notifications/initialized" | "initialized" => Ok(json!({})),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "resources/list" => Ok(json!({
            "resources": [
                {
                    "name": "Platform Info",
                    "uri": "palmier://platform/windows",
                    "description": "Windows port capabilities",
                    "mimeType": "application/json"
                }
            ]
        })),
        "resources/read" => {
            let uri = params
                .and_then(|p| p.get("uri"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if uri == "palmier://platform/windows" {
                Ok(json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": serde_json::to_string_pretty(&json!({
                            "platform": "windows",
                            "generativeAI": false,
                            "exportCodecs": ["h264", "h265"]
                        })).unwrap_or_default()
                    }]
                }))
            } else {
                Err(format!("unknown resource: {uri}"))
            }
        }
        _ => Err(format!("method not found: {method}")),
    }
}

fn json_response(id: Option<Value>, result: Option<Value>, error: Option<JsonRpcError>) -> Response {
    let resp = JsonRpcResponse {
        jsonrpc: "2.0",
        id: id.unwrap_or(Value::Null),
        result,
        error,
    };
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        Json(resp),
    )
        .into_response()
}
