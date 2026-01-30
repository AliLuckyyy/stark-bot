//! Agent Test Fixture
//!
//! Tests the agent loop with REAL tool implementations for CodeEngineer tasks.
//! This is a standalone test binary that implements the tools directly.
//!
//! Usage:
//!   TEST_QUERY="build a simple todo app" \
//!   TEST_AGENT_ENDPOINT="https://api.openai.com/v1/chat/completions" \
//!   TEST_AGENT_SECRET="your-api-key" \
//!   TEST_WORKSPACE="/tmp/agent-test-workspace" \
//!   cargo run --bin agent_test
//!
//! Environment variables:
//!   TEST_QUERY           - The user query to test
//!   TEST_AGENT_ENDPOINT  - LLM API endpoint (OpenAI-compatible)
//!   TEST_AGENT_SECRET    - API key for the LLM
//!   TEST_AGENT_MODEL     - Model name (auto-detected from endpoint, or specify manually)
//!   TEST_WORKSPACE       - Workspace directory for file operations
//!   TEST_SKILLS_DIR      - Path to skills directory (default: ./skills)
//!   TEST_MAX_ITERATIONS  - Max tool loop iterations (default: 25)

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap as StdHashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand};
use std::sync::Mutex;
use std::time::Duration;

// ============================================================================
// Types for OpenAI-compatible API
// ============================================================================

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolSpec>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCallResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolSpec {
    #[serde(rename = "type")]
    tool_type: String,
    function: ToolFunction,
}

#[derive(Debug, Clone, Serialize)]
struct ToolFunction {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolCallResponse {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallResponse>>,
}

// ============================================================================
// Tool Definitions - Real CodeEngineer Tools
// ============================================================================

fn get_code_engineer_tools() -> Vec<ToolSpec> {
    vec![
        // read_file
        ToolSpec {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                description: "Read the contents of a file. Use this to examine existing code or files.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to read (relative to workspace)"
                        }
                    },
                    "required": ["path"]
                }),
            },
        },
        // write_file
        ToolSpec {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "write_file".to_string(),
                description: "Create or overwrite a file with the given content. Use this to create new files or completely replace file contents.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to write (relative to workspace)"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write to the file"
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
        },
        // list_files
        ToolSpec {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "list_files".to_string(),
                description: "List files and directories in a path.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path to list (relative to workspace, default: '.')"
                        }
                    },
                    "required": []
                }),
            },
        },
        // exec
        ToolSpec {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "exec".to_string(),
                description: "Execute a shell command. Use for npm, cargo, git, and other CLI tools. Commands run in the workspace directory. Use background: true for long-running commands like servers.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute"
                        },
                        "timeout": {
                            "type": "integer",
                            "description": "Timeout in seconds (default: 60, max: 300)"
                        },
                        "background": {
                            "type": "boolean",
                            "description": "Run command in background, returns immediately with process ID. Use for servers and long-running commands."
                        }
                    },
                    "required": ["command"]
                }),
            },
        },
        // process_status
        ToolSpec {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "process_status".to_string(),
                description: "Check status, get output, or manage background processes started with exec background: true.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["status", "output", "kill", "list"],
                            "description": "Operation: status (check process), output (get recent output), kill (terminate), list (show all)"
                        },
                        "process_id": {
                            "type": "string",
                            "description": "The process ID (e.g., 'proc_1') from exec background mode"
                        },
                        "lines": {
                            "type": "integer",
                            "description": "Number of output lines to retrieve (default: 50)"
                        }
                    },
                    "required": ["operation"]
                }),
            },
        },
        // git
        ToolSpec {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "git".to_string(),
                description: "Execute git operations. Supports: status, diff, log, add, commit, branch, checkout, init.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["status", "diff", "log", "add", "commit", "branch", "checkout", "init"],
                            "description": "Git operation to perform"
                        },
                        "files": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Files to operate on (for add, diff)"
                        },
                        "message": {
                            "type": "string",
                            "description": "Commit message (for commit operation)"
                        },
                        "branch": {
                            "type": "string",
                            "description": "Branch name (for checkout, branch)"
                        },
                        "create": {
                            "type": "boolean",
                            "description": "Create new branch (for checkout)"
                        }
                    },
                    "required": ["operation"]
                }),
            },
        },
        // glob
        ToolSpec {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "glob".to_string(),
                description: "Find files matching a glob pattern.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern like '**/*.ts' or 'src/**/*.js'"
                        }
                    },
                    "required": ["pattern"]
                }),
            },
        },
        // grep
        ToolSpec {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "grep".to_string(),
                description: "Search for a pattern in files.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Regex pattern to search for"
                        },
                        "path": {
                            "type": "string",
                            "description": "Directory or file to search in"
                        }
                    },
                    "required": ["pattern"]
                }),
            },
        },
    ]
}

// ============================================================================
// Tool Execution - REAL implementations
// ============================================================================

fn execute_tool(name: &str, args: &Value, workspace: &Path) -> String {
    println!("\n   🔧 Executing: {}", name);
    println!("   📥 Args: {}", serde_json::to_string(args).unwrap_or_default());

    let result = match name {
        "read_file" => execute_read_file(args, workspace),
        "write_file" => execute_write_file(args, workspace),
        "list_files" => execute_list_files(args, workspace),
        "exec" => execute_exec(args, workspace),
        "process_status" => execute_process_status(args),
        "git" => execute_git(args, workspace),
        "glob" => execute_glob(args, workspace),
        "grep" => execute_grep(args, workspace),
        _ => format!("Unknown tool: {}", name),
    };

    // Truncate long output
    let display = if result.len() > 1000 {
        format!("{}...[truncated, {} chars total]", &result[..1000], result.len())
    } else {
        result.clone()
    };
    println!("   📤 Result: {}", display);

    result
}

fn execute_read_file(args: &Value, workspace: &Path) -> String {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let full_path = workspace.join(path);

    match fs::read_to_string(&full_path) {
        Ok(content) => content,
        Err(e) => format!("Error reading file: {}", e),
    }
}

fn execute_write_file(args: &Value, workspace: &Path) -> String {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let full_path = workspace.join(path);

    // Create parent directories if needed
    if let Some(parent) = full_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return format!("Error creating directories: {}", e);
        }
    }

    match fs::write(&full_path, content) {
        Ok(_) => format!("Successfully wrote {} bytes to {}", content.len(), path),
        Err(e) => format!("Error writing file: {}", e),
    }
}

fn execute_list_files(args: &Value, workspace: &Path) -> String {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let full_path = workspace.join(path);

    match fs::read_dir(&full_path) {
        Ok(entries) => {
            let mut files: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if e.path().is_dir() {
                        format!("{}/", name)
                    } else {
                        name
                    }
                })
                .collect();
            files.sort();
            files.join("\n")
        }
        Err(e) => format!("Error listing directory: {}", e),
    }
}

// Track background processes (simple in-memory store for test harness)
lazy_static::lazy_static! {
    static ref BACKGROUND_PROCESSES: Mutex<StdHashMap<String, BackgroundProcess>> = Mutex::new(StdHashMap::new());
    static ref PROCESS_COUNTER: Mutex<u32> = Mutex::new(0);
}

struct BackgroundProcess {
    id: String,
    pid: u32,
    command: String,
    #[allow(dead_code)]
    child: Option<Child>,
    output: Vec<String>,
    completed: bool,
    exit_code: Option<i32>,
}

fn execute_exec(args: &Value, workspace: &Path) -> String {
    let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let background = args.get("background").and_then(|v| v.as_bool()).unwrap_or(false);

    // Server command detection
    let server_patterns = [
        "npm start", "npm run dev", "npm run serve", "yarn start", "yarn dev",
        "node index.js", "node server.js", "node app.js",
        "python -m http.server", "python manage.py runserver", "flask run",
        "cargo run", "go run", "rails server", "rails s",
    ];
    let lower_cmd = command.to_lowercase();
    let is_server = server_patterns.iter().any(|p| lower_cmd.contains(p));

    if is_server && !background {
        return format!(
            "Detected server/long-running command: `{}`\n\n\
            Server commands run indefinitely and will block or timeout.\n\
            To run this command, use `background: true` to run it asynchronously.\n\n\
            Example:\n```json\n{{\n  \"command\": \"{}\",\n  \"background\": true\n}}\n```",
            command,
            command.replace("\"", "\\\"")
        );
    }

    if background {
        println!("   🖥️  Starting background: {}", command);

        match ProcessCommand::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(workspace)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => {
                let pid = child.id();
                let mut counter = PROCESS_COUNTER.lock().unwrap();
                *counter += 1;
                let process_id = format!("proc_{}", *counter);

                let bg_process = BackgroundProcess {
                    id: process_id.clone(),
                    pid,
                    command: command.to_string(),
                    child: Some(child),
                    output: Vec::new(),
                    completed: false,
                    exit_code: None,
                };

                BACKGROUND_PROCESSES.lock().unwrap().insert(process_id.clone(), bg_process);

                format!(
                    "Started background process\n\
                    Process ID: {}\n\
                    PID: {}\n\
                    Command: {}\n\n\
                    Use `process_status` tool with process_id=\"{}\" to check status or get output.",
                    process_id, pid, command, process_id
                )
            }
            Err(e) => format!("Failed to start background process: {}", e),
        }
    } else {
        println!("   🖥️  Running: {}", command);

        let output = ProcessCommand::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(workspace)
            .output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

                let mut result = String::new();
                if !stdout.is_empty() {
                    result.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !result.is_empty() {
                        result.push_str("\n[stderr]: ");
                    }
                    result.push_str(&stderr);
                }
                if result.is_empty() {
                    result = format!("Command completed with exit code {}", exit_code);
                }
                result
            }
            Err(e) => format!("Failed to execute command: {}", e),
        }
    }
}

fn execute_process_status(args: &Value) -> String {
    let operation = args.get("operation").and_then(|v| v.as_str()).unwrap_or("list");
    let process_id = args.get("process_id").and_then(|v| v.as_str());

    match operation {
        "status" => {
            let pid = match process_id {
                Some(id) => id,
                None => return "Error: process_id is required for 'status' operation".to_string(),
            };

            let processes = BACKGROUND_PROCESSES.lock().unwrap();
            match processes.get(pid) {
                Some(proc) => {
                    let status = if proc.completed { "completed" } else { "running" };
                    format!(
                        "Process: {}\nStatus: {}\nPID: {}\nCommand: {}{}",
                        proc.id,
                        status,
                        proc.pid,
                        proc.command,
                        if let Some(code) = proc.exit_code {
                            format!("\nExit code: {}", code)
                        } else {
                            String::new()
                        }
                    )
                }
                None => format!("Process '{}' not found", pid),
            }
        }

        "output" => {
            let pid = match process_id {
                Some(id) => id,
                None => return "Error: process_id is required for 'output' operation".to_string(),
            };

            let lines = args.get("lines").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

            let processes = BACKGROUND_PROCESSES.lock().unwrap();
            match processes.get(pid) {
                Some(proc) => {
                    if proc.output.is_empty() {
                        format!("No output captured yet for process '{}'", pid)
                    } else {
                        let output: Vec<_> = proc.output.iter().rev().take(lines).collect();
                        format!(
                            "Output from process '{}' (last {} lines):\n\n{}",
                            pid,
                            output.len(),
                            output.into_iter().rev().cloned().collect::<Vec<_>>().join("\n")
                        )
                    }
                }
                None => format!("Process '{}' not found", pid),
            }
        }

        "kill" => {
            let pid = match process_id {
                Some(id) => id,
                None => return "Error: process_id is required for 'kill' operation".to_string(),
            };

            let mut processes = BACKGROUND_PROCESSES.lock().unwrap();
            match processes.get_mut(pid) {
                Some(proc) => {
                    if let Some(ref mut child) = proc.child {
                        match child.kill() {
                            Ok(_) => {
                                proc.completed = true;
                                format!("Process '{}' has been killed", pid)
                            }
                            Err(e) => format!("Failed to kill process '{}': {}", pid, e),
                        }
                    } else {
                        format!("Process '{}' has no active child handle", pid)
                    }
                }
                None => format!("Process '{}' not found", pid),
            }
        }

        "list" => {
            let processes = BACKGROUND_PROCESSES.lock().unwrap();
            if processes.is_empty() {
                return "No background processes found.".to_string();
            }

            let mut result = String::from("Background processes:\n\n");
            for proc in processes.values() {
                let status = if proc.completed { "completed" } else { "running" };
                let short_cmd = if proc.command.len() > 50 {
                    format!("{}...", &proc.command[..47])
                } else {
                    proc.command.clone()
                };
                result.push_str(&format!(
                    "- {} (PID {}): {}\n  Command: {}\n\n",
                    proc.id, proc.pid, status, short_cmd
                ));
            }
            result
        }

        _ => format!("Unknown operation '{}'. Use: status, output, kill, or list", operation),
    }
}

fn execute_git(args: &Value, workspace: &Path) -> String {
    let operation = args.get("operation").and_then(|v| v.as_str()).unwrap_or("");

    let git_args: Vec<&str> = match operation {
        "status" => vec!["status", "--porcelain"],
        "diff" => vec!["diff"],
        "log" => vec!["log", "--oneline", "-10"],
        "init" => vec!["init"],
        "add" => {
            let files = args.get("files")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                .unwrap_or_default();
            if files.is_empty() {
                return "Error: No files specified for git add".to_string();
            }
            // Execute with files
            let mut cmd_args = vec!["add"];
            let output = ProcessCommand::new("git")
                .arg("add")
                .args(&files)
                .current_dir(workspace)
                .output();
            return match output {
                Ok(o) => format!("Staged {} file(s)", files.len()),
                Err(e) => format!("Git error: {}", e),
            };
        }
        "commit" => {
            let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("Update");
            let output = ProcessCommand::new("git")
                .args(["commit", "-m", message])
                .current_dir(workspace)
                .output();
            return match output {
                Ok(o) => {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    format!("{}{}", stdout, stderr)
                }
                Err(e) => format!("Git error: {}", e),
            };
        }
        "branch" => {
            if let Some(branch) = args.get("branch").and_then(|v| v.as_str()) {
                vec!["branch", branch]
            } else {
                vec!["branch", "-a"]
            }
        }
        "checkout" => {
            let branch = args.get("branch").and_then(|v| v.as_str()).unwrap_or("main");
            let create = args.get("create").and_then(|v| v.as_bool()).unwrap_or(false);
            if create {
                vec!["checkout", "-b", branch]
            } else {
                vec!["checkout", branch]
            }
        }
        _ => return format!("Unknown git operation: {}", operation),
    };

    let output = ProcessCommand::new("git")
        .args(&git_args)
        .current_dir(workspace)
        .output();

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            if stdout.is_empty() && stderr.is_empty() {
                format!("Git {} completed successfully", operation)
            } else {
                format!("{}{}", stdout, stderr)
            }
        }
        Err(e) => format!("Git error: {}", e),
    }
}

fn execute_glob(args: &Value, workspace: &Path) -> String {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("*");

    // Use find command for glob-like behavior
    let output = ProcessCommand::new("find")
        .args([".", "-name", pattern, "-type", "f"])
        .current_dir(workspace)
        .output();

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                "No files found matching pattern".to_string()
            } else {
                stdout.to_string()
            }
        }
        Err(e) => format!("Error: {}", e),
    }
}

fn execute_grep(args: &Value, workspace: &Path) -> String {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

    let output = ProcessCommand::new("grep")
        .args(["-rn", pattern, path])
        .current_dir(workspace)
        .output();

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                "No matches found".to_string()
            } else {
                stdout.to_string()
            }
        }
        Err(e) => format!("Error: {}", e),
    }
}

// ============================================================================
// System Prompt
// ============================================================================

fn get_system_prompt(workspace: &Path, skills: &[String]) -> String {
    format!(r#"You are a CodeEngineer agent that builds software. Your workspace is: {}

## Available Tools

- `write_file` - Create or overwrite files (path, content)
- `read_file` - Read file contents (path)
- `list_files` - List directory contents (path)
- `exec` - Run shell commands (command) - use for npm, cargo, pip, etc.
- `git` - Git operations (operation: status/diff/log/add/commit/init, files, message, branch)
- `glob` - Find files by pattern
- `grep` - Search in files

## How to Build Software

1. Create project structure with `write_file` or `exec` (npx create-*, cargo new, etc.)
2. Write source files with `write_file`
3. Install dependencies with `exec` (npm install, pip install, etc.)
4. Test with `exec` (npm test, cargo test, etc.)
5. Initialize git with `git` operation: "init"
6. Stage and commit with `git` operations: "add" then "commit"

## Important

- All file paths are relative to the workspace
- Use `exec` for running any shell command
- Create parent directories automatically when writing files
- Write complete, working code

## Skills Available
{}

Build what the user asks for. Use the tools to create real files and run real commands."#,
        workspace.display(),
        if skills.is_empty() { "None".to_string() } else { skills.join(", ") }
    )
}

// ============================================================================
// Skills
// ============================================================================

fn list_available_skills(skills_dir: &str) -> Vec<String> {
    let mut skills = Vec::new();
    if let Ok(entries) = fs::read_dir(skills_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    skills.push(name.to_string());
                }
            }
        }
    }
    skills
}

// ============================================================================
// Main Agent Loop
// ============================================================================

async fn run_agent_loop(
    client: &Client,
    endpoint: &str,
    api_key: &str,
    model: &str,
    query: &str,
    workspace: &Path,
    skills: &[String],
    max_iterations: usize,
) -> Result<String, String> {
    let tools = get_code_engineer_tools();
    let system_prompt = get_system_prompt(workspace, skills);

    let mut messages: Vec<Message> = vec![
        Message {
            role: "system".to_string(),
            content: Some(system_prompt),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        Message {
            role: "user".to_string(),
            content: Some(query.to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];

    let mut iteration = 0;

    loop {
        iteration += 1;
        println!("\n============================================================");
        println!("📤 ITERATION {} / {}", iteration, max_iterations);
        println!("============================================================");

        if iteration > max_iterations {
            return Err(format!("Max iterations ({}) reached", max_iterations));
        }

        let request = ChatRequest {
            model: model.to_string(),
            messages: messages.clone(),
            max_tokens: 4096,
            tools: Some(tools.clone()),
            tool_choice: Some(json!("auto")),
        };

        println!("\n📋 Sending request to {} (model: {})", endpoint, model);

        let response = client
            .post(endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

        if !status.is_success() {
            println!("\n❌ API Error ({}): {}", status, response_text);
            return Err(format!("API error {}: {}", status, response_text));
        }

        let chat_response: ChatResponse = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse response: {} - body: {}", e, response_text))?;

        let choice = chat_response.choices.first().ok_or("No choices in response")?;

        println!("\n📊 Response:");
        println!("   finish_reason: {:?}", choice.finish_reason);
        if let Some(content) = &choice.message.content {
            let preview = if content.len() > 300 { format!("{}...", &content[..300]) } else { content.clone() };
            println!("   content: {}", preview);
        }
        println!("   tool_calls: {:?}", choice.message.tool_calls.as_ref().map(|t| t.len()));

        // Check for tool calls
        if let Some(tool_calls) = &choice.message.tool_calls {
            if !tool_calls.is_empty() {
                println!("\n🔧 Processing {} tool call(s):", tool_calls.len());

                // Add assistant message with tool calls
                messages.push(Message {
                    role: "assistant".to_string(),
                    content: choice.message.content.clone(),
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });

                // Execute each tool
                for tc in tool_calls {
                    println!("\n   📍 Tool: {} (id: {})", tc.function.name, tc.id);

                    let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
                    let result = execute_tool(&tc.function.name, &args, workspace);

                    messages.push(Message {
                        role: "tool".to_string(),
                        content: Some(result),
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tc.function.name.clone()),
                    });
                }

                continue; // Next iteration
            }
        }

        // No tool calls - final response
        let final_content = choice.message.content.clone().unwrap_or_default();
        println!("\n✅ Final response:");
        println!("{}", final_content);

        return Ok(final_content);
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    println!("🤖 StarkBot Agent Test");
    println!("======================\n");

    // Read environment variables
    let query = env::var("TEST_QUERY").unwrap_or_else(|_| {
        "Build a simple todo app with TypeScript. Create a basic CLI todo app with add, list, and remove commands.".to_string()
    });

    let endpoint = env::var("TEST_AGENT_ENDPOINT").unwrap_or_else(|_| {
        eprintln!("❌ TEST_AGENT_ENDPOINT not set!");
        eprintln!("   Example: https://api.openai.com/v1/chat/completions");
        std::process::exit(1);
    });

    let secret = env::var("TEST_AGENT_SECRET").unwrap_or_else(|_| {
        eprintln!("❌ TEST_AGENT_SECRET not set!");
        std::process::exit(1);
    });

    let model = env::var("TEST_AGENT_MODEL").unwrap_or_else(|_| {
        // Auto-detect model based on endpoint
        if endpoint.contains("moonshot") {
            "moonshot-v1-128k".to_string()
        } else if endpoint.contains("anthropic") {
            "claude-sonnet-4-20250514".to_string()
        } else {
            "gpt-4o".to_string()
        }
    });

    let workspace_str = env::var("TEST_WORKSPACE").unwrap_or_else(|_| {
        "/tmp/agent-test-workspace".to_string()
    });
    let workspace = PathBuf::from(&workspace_str);

    let skills_dir = env::var("TEST_SKILLS_DIR").unwrap_or_else(|_| {
        if Path::new("skills").exists() {
            "skills".to_string()
        } else if Path::new("../skills").exists() {
            "../skills".to_string()
        } else {
            "./skills".to_string()
        }
    });

    let max_iterations: usize = env::var("TEST_MAX_ITERATIONS")
        .unwrap_or_else(|_| "25".to_string())
        .parse()
        .unwrap_or(25);

    let skills = list_available_skills(&skills_dir);

    println!("📝 Configuration:");
    println!("   Query:      {}", query);
    println!("   Endpoint:   {}", endpoint);
    println!("   Model:      {}", model);
    println!("   Workspace:  {}", workspace.display());
    println!("   Skills:     {} ({} found)", skills_dir, skills.len());
    println!("   Max Iters:  {}", max_iterations);

    // Clean and create workspace
    if workspace.exists() {
        println!("\n🧹 Cleaning existing workspace...");
        let _ = fs::remove_dir_all(&workspace);
    }
    if let Err(e) = fs::create_dir_all(&workspace) {
        eprintln!("❌ Failed to create workspace: {}", e);
        std::process::exit(1);
    }
    println!("✅ Workspace ready: {}", workspace.display());

    // Create HTTP client
    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .expect("Failed to create HTTP client");

    // Run the agent loop
    println!("\n🚀 Starting agent loop...\n");

    match run_agent_loop(
        &client,
        &endpoint,
        &secret,
        &model,
        &query,
        &workspace,
        &skills,
        max_iterations,
    ).await {
        Ok(response) => {
            println!("\n============================================================");
            println!("🎉 SUCCESS");
            println!("============================================================");
            println!("{}", response);

            // Show what was created
            println!("\n📁 Workspace contents:");
            fn list_recursive(path: &Path, prefix: &str) {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        let name = p.file_name().unwrap_or_default().to_string_lossy();
                        if p.is_dir() {
                            println!("{}📁 {}/", prefix, name);
                            list_recursive(&p, &format!("{}  ", prefix));
                        } else {
                            println!("{}📄 {}", prefix, name);
                        }
                    }
                }
            }
            list_recursive(&workspace, "   ");
        }
        Err(e) => {
            println!("\n============================================================");
            println!("❌ ERROR");
            println!("============================================================");
            println!("{}", e);
            std::process::exit(1);
        }
    }
}
