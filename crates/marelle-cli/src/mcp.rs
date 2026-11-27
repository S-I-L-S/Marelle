// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Marelle contributors

//! A bounded MCP client over newline-delimited JSON-RPC on child-process
//! stdin/stdout. The process is kept alive through a single model/tool turn.

use std::collections::{BTreeMap, HashSet};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use marelle_config::Config;
use marelle_core::ToolCall;
use marelle_i18n::Locale;
use marelle_providers::ToolDefinition;
use serde_json::{Value, json};

use crate::tools::{self, ToolAsk, ToolDecision};

const MAX_LINE_BYTES: usize = 1024 * 1024;
const MAX_QUEUED_MESSAGES: usize = 8;
const MAX_TOOL_OUTPUT: usize = 32 * 1024;
const MAX_TOOLS_PER_SERVER: usize = 64;
const MAX_TOOLS_TOTAL: usize = 128;
const MAX_PAGES: usize = 16;
const MAX_SERVERS: usize = 32;
const MAX_TIMEOUT_SECS: u64 = 3600;
const PROTOCOL_VERSION: &str = "2025-11-25";

#[derive(Debug)]
enum ReaderEvent {
    Line(Vec<u8>),
    Failed(String),
    Closed,
}

#[derive(Debug, Clone)]
struct ServerConfig {
    id: String,
    command: String,
    args: Vec<String>,
    env: Vec<String>,
}

#[derive(Debug, Clone)]
struct PublishedTool {
    server: String,
    name: String,
    definition: ToolDefinition,
}

/// Connected, approved MCP servers for one user turn.
pub(crate) struct Runtime {
    clients: BTreeMap<String, Client>,
    published: Vec<PublishedTool>,
    definitions: Vec<ToolDefinition>,
    timeout: Duration,
}

impl Runtime {
    /// Starts configured servers only after their trust policy allows it.
    /// A broken server is reported and skipped, so built-in tools still work.
    pub(crate) fn connect_all(
        config: &Config,
        locale: Locale,
        mut approve: impl FnMut(&ToolAsk) -> io::Result<ToolDecision>,
    ) -> io::Result<(Self, Vec<String>)> {
        let mut errors = Vec::new();
        let timeout_secs = match config.get_u64("mcp.timeout_secs") {
            Ok(Some(value)) if (1..=MAX_TIMEOUT_SECS).contains(&value) => value,
            Ok(None) => 120,
            Ok(Some(_)) | Err(_) => {
                errors.push(format!(
                    "MCP: invalid mcp.timeout_secs; using the default of 120 seconds (maximum {MAX_TIMEOUT_SECS})."
                ));
                120
            }
        };
        let timeout = Duration::from_secs(timeout_secs);
        let mut runtime = Self {
            clients: BTreeMap::new(),
            published: Vec::new(),
            definitions: Vec::new(),
            timeout,
        };
        let (servers, server_errors) = server_configs(config);
        errors.extend(server_errors);

        for (index, server) in servers.into_iter().enumerate() {
            if index >= MAX_SERVERS {
                errors.push(format!(
                    "MCP: only the first {MAX_SERVERS} servers are enabled."
                ));
                break;
            }
            let trust_key = format!("permissions.mcp_servers.{}", server.id);
            let display = command_display(&server);
            let started = tools::confirm(
                config,
                &trust_key,
                "tool.label.mcp.server",
                display,
                locale,
                &mut approve,
            )?;
            if !started {
                continue;
            }

            match Client::connect(&server, timeout) {
                Ok(mut client) => match client.list_tools(timeout) {
                    Ok(tools) if !tools.is_empty() => {
                        for tool in tools {
                            let name = format!("mcp.{}.{}", server.id, tool.name);
                            if name.len() > 64 {
                                errors.push(format!(
                                    "MCP tool '{}': namespaced name exceeds the provider-safe 64-character limit.",
                                    tool.name
                                ));
                                continue;
                            }
                            if runtime.definitions.len() >= MAX_TOOLS_TOTAL {
                                errors.push(format!(
                                    "MCP: stopped adding tools at the {MAX_TOOLS_TOTAL}-tool limit."
                                ));
                                break;
                            }
                            runtime.definitions.push(ToolDefinition {
                                name: name.clone(),
                                description: tool.description.clone(),
                                parameters: tool.schema.clone(),
                            });
                            runtime.published.push(PublishedTool {
                                server: server.id.clone(),
                                name: tool.name,
                                definition: ToolDefinition {
                                    name,
                                    description: tool.description,
                                    parameters: tool.schema,
                                },
                            });
                        }
                        runtime.clients.insert(server.id.clone(), client);
                    }
                    Ok(_) => errors.push(format!(
                        "MCP server '{}': no tools were reported.",
                        server.id
                    )),
                    Err(error) => errors.push(format!("MCP server '{}': {error}", server.id)),
                },
                Err(error) => errors.push(format!("MCP server '{}': {error}", server.id)),
            }
        }
        Ok((runtime, errors))
    }

    pub(crate) fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    /// Checks policy before passing the call to its exact MCP server/tool.
    pub(crate) fn call(
        &mut self,
        call: &ToolCall,
        config: &Config,
        locale: Locale,
        mut approve: impl FnMut(&ToolAsk) -> io::Result<ToolDecision>,
    ) -> Result<String, String> {
        let Some(published) = self
            .published
            .iter()
            .find(|tool| tool.definition.name == call.name)
        else {
            return Err(format!("Unknown MCP tool: {}", call.name));
        };
        let permission_key = format!(
            "permissions.mcp_tools.{}.{}",
            published.server, published.name
        );
        let arguments: Value = serde_json::from_str(&call.arguments)
            .map_err(|_| "MCP tool arguments are not valid JSON.".to_owned())?;
        if !arguments.is_object() {
            return Err("MCP tool arguments must be a JSON object.".to_owned());
        }
        let detail = shorten_bytes(&arguments.to_string(), 2048);
        let allowed = tools::confirm(
            config,
            &permission_key,
            "tool.label.mcp.call",
            format!("{}\n{detail}", call.name),
            locale,
            &mut approve,
        )
        .map_err(|error| error.to_string())?;
        if !allowed {
            return Ok("MCP tool call denied.".to_owned());
        }
        let client = self
            .clients
            .get_mut(&published.server)
            .ok_or_else(|| "MCP server is no longer connected.".to_owned())?;
        let result = client.call_tool(&published.name, arguments, self.timeout)?;
        Ok(render_result(&result))
    }
}

struct ListedTool {
    name: String,
    description: String,
    schema: Value,
}

struct Client {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<ReaderEvent>,
    next_id: u64,
}

impl Client {
    fn connect(server: &ServerConfig, timeout: Duration) -> Result<Self, String> {
        let mut command = Command::new(&server.command);
        command
            .args(&server.args)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for name in inherited_environment_names() {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        for name in &server.env {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }

        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start '{}': {error}", server.command))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "server stdin was not available.".to_owned())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "server stdout was not available.".to_owned())?;
        // A slow server must not be able to grow an unbounded queue by
        // flooding stdout while Marelle is waiting on another operation.
        let (sender, lines) = mpsc::sync_channel(MAX_QUEUED_MESSAGES);
        thread::Builder::new()
            .name(format!("marelle-mcp-{}", server.id))
            .spawn(move || read_lines(stdout, sender))
            .map_err(|error| format!("could not read server output: {error}"))?;

        let mut client = Self {
            child,
            stdin,
            lines,
            next_id: 1,
        };
        let initialized = client.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "Marelle", "version": env!("CARGO_PKG_VERSION")}
            }),
            timeout,
        )?;
        let negotiated = initialized
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or_else(|| "initialize response has no protocolVersion.".to_owned())?;
        if !supported_version(negotiated) {
            return Err(format!("unsupported MCP protocol version '{negotiated}'."));
        }
        client.notify("notifications/initialized", json!({}))?;
        Ok(client)
    }

    fn list_tools(&mut self, timeout: Duration) -> Result<Vec<ListedTool>, String> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        let mut names = HashSet::new();
        for _ in 0..MAX_PAGES {
            let mut params = serde_json::Map::new();
            if let Some(cursor) = cursor.take() {
                params.insert("cursor".to_owned(), Value::String(cursor));
            }
            let response = self.request("tools/list", Value::Object(params), timeout)?;
            let page = response
                .get("tools")
                .and_then(Value::as_array)
                .ok_or_else(|| "tools/list response has no tools array.".to_owned())?;
            for value in page {
                if tools.len() >= MAX_TOOLS_PER_SERVER {
                    return Err(format!(
                        "server reported more than {MAX_TOOLS_PER_SERVER} tools."
                    ));
                }
                let name = value
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| valid_tool_name(name))
                    .ok_or_else(|| "server reported a tool with an invalid name.".to_owned())?
                    .to_owned();
                if !names.insert(name.clone()) {
                    return Err(format!("server reported duplicate tool '{name}'."));
                }
                let schema = value
                    .get("inputSchema")
                    .filter(|schema| schema.is_object())
                    .cloned()
                    .ok_or_else(|| format!("tool '{name}' has no object inputSchema."))?;
                if schema.to_string().len() > MAX_LINE_BYTES / 2 {
                    return Err(format!("tool '{name}' has an oversized input schema."));
                }
                let description = value
                    .get("description")
                    .and_then(Value::as_str)
                    .map(|description| shorten_bytes(description, 4096))
                    .unwrap_or_default();
                tools.push(ListedTool {
                    name,
                    description,
                    schema,
                });
            }
            cursor = response
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if cursor.is_none() {
                return Ok(tools);
            }
        }
        Err(format!("tools/list exceeded the {MAX_PAGES}-page limit."))
    }

    fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        self.request(
            "tools/call",
            json!({"name": name, "arguments": arguments}),
            timeout,
        )
    }

    fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        let deadline = Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!("request '{method}' timed out."));
            }
            let event = self
                .lines
                .recv_timeout(remaining)
                .map_err(|error| match error {
                    RecvTimeoutError::Timeout => format!("request '{method}' timed out."),
                    RecvTimeoutError::Disconnected => "server output reader stopped.".to_owned(),
                })?;
            let line = match event {
                ReaderEvent::Line(line) => line,
                ReaderEvent::Failed(error) => return Err(error),
                ReaderEvent::Closed => return Err("server closed stdout.".to_owned()),
            };
            let message: Value = serde_json::from_slice(&line)
                .map_err(|_| "server sent invalid JSON-RPC.".to_owned())?;
            if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
                return Err("server sent a message without jsonrpc 2.0.".to_owned());
            }
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    let code = error.get("code").and_then(Value::as_i64).unwrap_or(-32603);
                    let text = error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("server returned an error");
                    return Err(shorten_bytes(&format!("JSON-RPC {code}: {text}"), 1024));
                }
                return message
                    .get("result")
                    .cloned()
                    .ok_or_else(|| "server response has neither result nor error.".to_owned());
            }
            if message.get("method").and_then(Value::as_str).is_some()
                && let Some(request_id) = message.get("id").cloned()
            {
                // Client callbacks (roots, sampling and elicitation) are not
                // advertised, so reject them explicitly instead of hanging.
                self.send(&json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "error": {"code": -32601, "message": "Method not found"}
                }))?;
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    fn send(&mut self, value: &Value) -> Result<(), String> {
        let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_LINE_BYTES {
            return Err("outgoing MCP message exceeded the 1 MiB limit.".to_owned());
        }
        self.stdin
            .write_all(&bytes)
            .and_then(|()| self.stdin.write_all(b"\n"))
            .and_then(|()| self.stdin.flush())
            .map_err(|error| format!("could not write to server: {error}"))
    }

    fn shutdown(&mut self) {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let _ = self.send(&json!({"jsonrpc": "2.0", "id": id, "method": "shutdown"}));
        let _ = self.lines.recv_timeout(Duration::from_millis(250));
        let _ = self.notify("notifications/exit", json!({}));
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn server_configs(config: &Config) -> (Vec<ServerConfig>, Vec<String>) {
    let mut ids = Vec::new();
    for key in config.keys() {
        let Some(rest) = key.strip_prefix("mcp.servers.") else {
            continue;
        };
        let Some((id, _)) = rest.split_once('.') else {
            continue;
        };
        if !ids.iter().any(|known| known == id) {
            ids.push(id.to_owned());
        }
    }
    ids.sort();

    let mut servers = Vec::new();
    let mut errors = Vec::new();
    for id in ids.into_iter().take(MAX_SERVERS + 1) {
        if !valid_server_id(&id) {
            errors.push(format!("MCP: invalid server id '{id}'."));
            continue;
        }
        let command_key = format!("mcp.servers.{id}.command");
        let Some(command) = config
            .get(&command_key)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            errors.push(format!("MCP server '{id}': missing setting {command_key}."));
            continue;
        };
        if command.len() > 4096 {
            errors.push(format!("MCP server '{id}': command is too long."));
            continue;
        }
        let args = match config.get_list(&format!("mcp.servers.{id}.args")) {
            Ok(Some(args)) if args.len() <= 64 && args.iter().all(|arg| arg.len() <= 8192) => args,
            Ok(None) => Vec::new(),
            _ => {
                errors.push(format!(
                    "MCP server '{id}': args must be a list of at most 64 values."
                ));
                continue;
            }
        };
        let env = match config.get_list(&format!("mcp.servers.{id}.env")) {
            Ok(Some(env)) if env.len() <= 32 && env.iter().all(|name| valid_env_name(name)) => env,
            Ok(None) => Vec::new(),
            _ => {
                errors.push(format!(
                    "MCP server '{id}': env must list up to 32 variable names."
                ));
                continue;
            }
        };
        servers.push(ServerConfig {
            id,
            command: command.to_owned(),
            args,
            env,
        });
    }
    if servers.len() > MAX_SERVERS {
        errors.push(format!(
            "MCP: only {MAX_SERVERS} servers can be configured."
        ));
        servers.truncate(MAX_SERVERS);
    }
    (servers, errors)
}

fn valid_server_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_env_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(first) if first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn inherited_environment_names() -> &'static [&'static str] {
    &[
        "PATH",
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "SYSTEMROOT",
        "WINDIR",
        "TEMP",
        "TMP",
        "TMPDIR",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "TERM",
        "NO_COLOR",
    ]
}

fn command_display(server: &ServerConfig) -> String {
    let mut command = server.command.clone();
    for arg in &server.args {
        command.push(' ');
        command.push_str(&format!("{:?}", shorten_bytes(arg, 512)));
    }
    if server.env.is_empty() {
        command
    } else {
        format!(
            "{command}\nPassed environment names: {}",
            server.env.join(", ")
        )
    }
}

fn supported_version(version: &str) -> bool {
    matches!(
        version,
        "2025-11-25" | "2025-06-18" | "2025-03-26" | "2024-11-05"
    )
}

fn read_lines<R: Read>(stdout: R, sender: SyncSender<ReaderEvent>) {
    let mut reader = BufReader::new(stdout);
    loop {
        match read_bounded_line(&mut reader) {
            Ok(Some(line)) => {
                if sender.send(ReaderEvent::Line(line)).is_err() {
                    return;
                }
            }
            Ok(None) => {
                let _ = sender.send(ReaderEvent::Closed);
                return;
            }
            Err(error) => {
                let _ = sender.send(ReaderEvent::Failed(format!(
                    "could not read server output: {error}"
                )));
                return;
            }
        }
    }
}

fn read_bounded_line(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let count = newline.map_or(available.len(), |index| index + 1);
        if line.len().saturating_add(count) > MAX_LINE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "line exceeds the 1 MiB limit",
            ));
        }
        line.extend_from_slice(&available[..count]);
        reader.consume(count);
        if newline.is_some() {
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            return Ok(Some(line));
        }
    }
}

fn render_result(result: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(content) = result.get("content").and_then(Value::as_array) {
        for item in content {
            match item.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(text) = item.get("text").and_then(Value::as_str) {
                        parts.push(text.to_owned());
                    }
                }
                Some("resource") => {
                    if let Some(text) = item.pointer("/resource/text").and_then(Value::as_str) {
                        parts.push(text.to_owned());
                    } else {
                        parts.push("[MCP returned a non-text resource]".to_owned());
                    }
                }
                Some("resource_link") => {
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("resource");
                    let uri = item.get("uri").and_then(Value::as_str).unwrap_or("");
                    parts.push(format!("[MCP resource link: {name} {uri}]"));
                }
                Some("image") => {
                    parts.push("[MCP returned an image; binary data omitted]".to_owned())
                }
                Some("audio") => parts.push("[MCP returned audio; binary data omitted]".to_owned()),
                Some(kind) => parts.push(format!("[MCP returned {kind} content]")),
                None => parts.push("[MCP returned an untyped content block]".to_owned()),
            }
        }
    }
    if parts.is_empty()
        && let Some(structured) = result.get("structuredContent")
    {
        parts.push(structured.to_string());
    }
    if parts.is_empty() {
        parts.push("MCP tool returned no text.".to_owned());
    }
    let mut output = parts.join("\n");
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        output.insert_str(0, "MCP tool error: ");
    }
    shorten_bytes(&output, MAX_TOOL_OUTPUT)
}

fn shorten_bytes(text: &str, maximum: usize) -> String {
    if text.len() <= maximum {
        return text.to_owned();
    }
    let mut end = maximum;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[output truncated]", &text[..end])
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;

    use super::*;
    use marelle_config::Layer;

    #[test]
    fn server_ids_are_safe_namespaces() {
        assert!(valid_server_id("filesystem-1"));
        assert!(valid_server_id("local_tools"));
        assert!(!valid_server_id("../server"));
        assert!(!valid_server_id("tool.server"));
        assert!(!valid_server_id(""));
    }

    #[test]
    fn only_valid_environment_variable_names_are_passed() {
        assert!(valid_env_name("GITHUB_TOKEN"));
        assert!(valid_env_name("_PRIVATE"));
        assert!(!valid_env_name("9TOKEN"));
        assert!(!valid_env_name("TOKEN=secret"));
    }

    #[test]
    fn mcp_tool_names_follow_the_protocol_name_rules() {
        assert!(valid_tool_name("read_file"));
        assert!(valid_tool_name("admin.tools.list"));
        assert!(!valid_tool_name("read file"));
        assert!(!valid_tool_name("a".repeat(129).as_str()));
    }

    #[test]
    fn stdout_frames_have_a_hard_size_limit() {
        let oversized = vec![b'x'; MAX_LINE_BYTES + 1];
        let error = read_bounded_line(&mut oversized.as_slice()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn result_text_is_bounded_without_breaking_utf8() {
        let result = json!({"content": [{"type":"text", "text":"Ж".repeat(MAX_TOOL_OUTPUT)}]});
        let rendered = render_result(&result);
        assert!(rendered.len() <= MAX_TOOL_OUTPUT + "\n[output truncated]".len());
        assert!(rendered.is_char_boundary(rendered.len()));
        assert!(rendered.ends_with("[output truncated]"));
    }

    #[test]
    fn images_are_described_without_copying_binary_data() {
        let result = json!({"content": [{"type":"image", "data":"secret-base64"}]});
        assert_eq!(
            render_result(&result),
            "[MCP returned an image; binary data omitted]"
        );
    }

    #[test]
    fn python_server_completes_the_stdio_lifecycle_and_tool_call() {
        let python = ["python3", "python"].into_iter().find(|candidate| {
            std::process::Command::new(candidate)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        });
        let Some(python) = python else {
            return;
        };
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../ext/python/word_count_server.py");
        let mut config = Config::new();
        config.set(Layer::Project, "mcp.timeout_secs", "5");
        config.set(Layer::Project, "mcp.servers.words.command", python);
        config.set(
            Layer::Project,
            "mcp.servers.words.args",
            json!([script.to_string_lossy()]).to_string(),
        );
        config.set(Layer::Project, "permissions.mcp_servers.words", "allow");
        config.set(
            Layer::Project,
            "permissions.mcp_tools.words.word_count",
            "allow",
        );

        let (mut runtime, errors) =
            Runtime::connect_all(&config, Locale::En, |_| Ok(ToolDecision::AllowOnce)).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(runtime.definitions().len(), 1);
        assert_eq!(runtime.definitions()[0].name, "mcp.words.word_count");

        let result = runtime
            .call(
                &ToolCall {
                    id: "call-1".to_owned(),
                    name: "mcp.words.word_count".to_owned(),
                    arguments: json!({"text": "one two\nthree"}).to_string(),
                },
                &config,
                Locale::En,
                |_| Ok(ToolDecision::AllowOnce),
            )
            .unwrap();
        assert_eq!(result, "3");
    }
}
