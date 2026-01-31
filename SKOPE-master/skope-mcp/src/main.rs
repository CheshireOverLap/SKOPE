//! SKOPE MCP Server
//!
//! Model Context Protocol server for SKOPE Engine integration with Claude Desktop.
//! Provides tools for creating spells, spawning entities, and querying scene info.

mod protocol;
mod tools;
mod resources;

use std::path::PathBuf;
use std::io::{self, BufRead, Write};
use clap::Parser;
use log::{info, debug, error};

use protocol::{JsonRpcRequest, JsonRpcResponse, JsonRpcError};
use tools::ToolRegistry;
use resources::ResourceRegistry;

/// SKOPE MCP Server - Claude Desktop integration
#[derive(Parser, Debug)]
#[command(name = "skope-mcp")]
#[command(about = "MCP Server for SKOPE Engine")]
struct Args {
    /// Path to SKOPE project directory
    #[arg(short, long, default_value = ".")]
    project: PathBuf,
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();

    let args = Args::parse();

    info!("SKOPE MCP Server starting...");
    info!("Project directory: {:?}", args.project);

    if let Err(e) = run_server(&args.project) {
        error!("Server error: {}", e);
        std::process::exit(1);
    }
}

fn run_server(project_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let tool_registry = ToolRegistry::new(project_path.clone());
    let resource_registry = ResourceRegistry::new(project_path.clone());

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    // Send initialization (server capabilities)
    // MCP servers should respond to initialize request

    for line in stdin.lock().lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }

        debug!("Received: {}", line);

        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => handle_request(request, &tool_registry, &resource_registry),
            Err(e) => JsonRpcResponse::error(
                None,
                JsonRpcError::parse_error(&format!("Parse error: {}", e))
            ),
        };

        let response_json = serde_json::to_string(&response)?;
        debug!("Response: {}", response_json);

        writeln!(stdout, "{}", response_json)?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle_request(
    request: JsonRpcRequest,
    tools: &ToolRegistry,
    resources: &ResourceRegistry,
) -> JsonRpcResponse {
    match request.method.as_str() {
        // MCP Protocol methods
        "initialize" => handle_initialize(request.id),
        "initialized" => JsonRpcResponse::success(request.id, serde_json::json!({})),

        // Tool methods
        "tools/list" => handle_tools_list(request.id, tools),
        "tools/call" => handle_tool_call(request.id, request.params, tools),

        // Resource methods
        "resources/list" => handle_resources_list(request.id, resources),
        "resources/read" => handle_resource_read(request.id, request.params, resources),

        // Prompt methods (optional)
        "prompts/list" => handle_prompts_list(request.id),

        // Unknown method
        _ => JsonRpcResponse::error(
            request.id,
            JsonRpcError::method_not_found(&request.method)
        ),
    }
}

fn handle_initialize(id: Option<serde_json::Value>) -> JsonRpcResponse {
    JsonRpcResponse::success(id, serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {},
            "resources": {
                "subscribe": false,
                "listChanged": false
            },
            "prompts": {}
        },
        "serverInfo": {
            "name": "skope-mcp",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

fn handle_tools_list(id: Option<serde_json::Value>, tools: &ToolRegistry) -> JsonRpcResponse {
    JsonRpcResponse::success(id, serde_json::json!({
        "tools": tools.list_tools()
    }))
}

fn handle_tool_call(
    id: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
    tools: &ToolRegistry,
) -> JsonRpcResponse {
    let params = match params {
        Some(p) => p,
        None => return JsonRpcResponse::error(
            id,
            JsonRpcError::invalid_params("Missing params")
        ),
    };

    let tool_name = params.get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("");

    let arguments = params.get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    match tools.call_tool(tool_name, arguments) {
        Ok(result) => JsonRpcResponse::success(id, serde_json::json!({
            "content": [{
                "type": "text",
                "text": result
            }]
        })),
        Err(e) => JsonRpcResponse::error(
            id,
            JsonRpcError::internal_error(&e.to_string())
        ),
    }
}

fn handle_resources_list(id: Option<serde_json::Value>, resources: &ResourceRegistry) -> JsonRpcResponse {
    JsonRpcResponse::success(id, serde_json::json!({
        "resources": resources.list_resources()
    }))
}

fn handle_resource_read(
    id: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
    resources: &ResourceRegistry,
) -> JsonRpcResponse {
    let params = match params {
        Some(p) => p,
        None => return JsonRpcResponse::error(
            id,
            JsonRpcError::invalid_params("Missing params")
        ),
    };

    let uri = params.get("uri")
        .and_then(|u| u.as_str())
        .unwrap_or("");

    match resources.read_resource(uri) {
        Ok(content) => JsonRpcResponse::success(id, serde_json::json!({
            "contents": [{
                "uri": uri,
                "mimeType": "text/plain",
                "text": content
            }]
        })),
        Err(e) => JsonRpcResponse::error(
            id,
            JsonRpcError::internal_error(&e.to_string())
        ),
    }
}

fn handle_prompts_list(id: Option<serde_json::Value>) -> JsonRpcResponse {
    // No prompts for now
    JsonRpcResponse::success(id, serde_json::json!({
        "prompts": []
    }))
}
