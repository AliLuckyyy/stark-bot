---
name: Tools
---

Tools extend StarkBot's capabilities beyond conversation. The AI can decide to use tools during message processing to perform actions and gather information.

## How Tools Work

1. User sends a message
2. AI analyzes the request
3. If a tool is needed, AI generates a tool call
4. Tool executes with provided parameters
5. Results are fed back to AI
6. AI continues or generates final response
7. Up to 10 tool iterations per message

## Tool Groups

Tools are organized into groups for access control:

| Group | Description |
|-------|-------------|
| Web | Internet access for search and fetching |
| Filesystem | File operations (read, write, list) |
| Exec | Shell command execution |
| Messaging | Send messages to channels |
| System | Agent management |

---

## Web Tools

### web_search

Search the web for information.

**Parameters:**
| Name | Type | Description |
|------|------|-------------|
| query | string | Search query |

**Example:**
```json
{
  "name": "web_search",
  "parameters": {
    "query": "latest React 19 features"
  }
}
```

**Requires:** Brave Search or SerpAPI key configured.

---

### web_fetch

Fetch content from a URL.

**Parameters:**
| Name | Type | Description |
|------|------|-------------|
| url | string | URL to fetch |
| selector | string | (Optional) CSS selector to extract |

**Example:**
```json
{
  "name": "web_fetch",
  "parameters": {
    "url": "https://example.com/api/data",
    "selector": ".content"
  }
}
```

---

## Filesystem Tools

### read_file

Read contents of a file.

**Parameters:**
| Name | Type | Description |
|------|------|-------------|
| path | string | File path to read |

**Example:**
```json
{
  "name": "read_file",
  "parameters": {
    "path": "/app/config.json"
  }
}
```

---

### write_file

Write content to a file.

**Parameters:**
| Name | Type | Description |
|------|------|-------------|
| path | string | File path to write |
| content | string | Content to write |

**Example:**
```json
{
  "name": "write_file",
  "parameters": {
    "path": "/app/output.txt",
    "content": "Hello, World!"
  }
}
```

---

### list_files

List files in a directory.

**Parameters:**
| Name | Type | Description |
|------|------|-------------|
| path | string | Directory path |
| recursive | boolean | (Optional) List recursively |

**Example:**
```json
{
  "name": "list_files",
  "parameters": {
    "path": "/app/src",
    "recursive": true
  }
}
```

---

### apply_patch

Apply a patch to modify a file.

**Parameters:**
| Name | Type | Description |
|------|------|-------------|
| path | string | File to patch |
| patch | string | Unified diff format patch |

Useful for making targeted edits without rewriting entire files.

---

## Exec Tool

### exec

Execute shell commands.

**Parameters:**
| Name | Type | Description |
|------|------|-------------|
| command | string | Command to execute |
| cwd | string | (Optional) Working directory |
| timeout | number | (Optional) Timeout in ms |

**Example:**
```json
{
  "name": "exec",
  "parameters": {
    "command": "git status",
    "cwd": "/app"
  }
}
```

**Security:**
- Dangerous commands are blocked (`rm -rf /`, `format`, etc.)
- Shell metacharacters restricted
- Execution timeout enforced

---

## Messaging Tool

### agent_send

Send a message to a configured channel.

**Parameters:**
| Name | Type | Description |
|------|------|-------------|
| channel_id | string | Target channel ID |
| message | string | Message to send |

**Example:**
```json
{
  "name": "agent_send",
  "parameters": {
    "channel_id": "discord-channel-uuid",
    "message": "Build completed successfully!"
  }
}
```

Useful for notifications and cross-platform messaging.

---

## System Tools

### subagent

Spawn a child agent for parallel tasks.

**Parameters:**
| Name | Type | Description |
|------|------|-------------|
| task | string | Task description |
| tools | array | (Optional) Tool subset |

**Example:**
```json
{
  "name": "subagent",
  "parameters": {
    "task": "Research competitor pricing",
    "tools": ["web_search", "web_fetch"]
  }
}
```

---

### subagent_status

Check status of a spawned subagent.

**Parameters:**
| Name | Type | Description |
|------|------|-------------|
| agent_id | string | Subagent ID |

---

## Tool Execution Events

The WebSocket gateway broadcasts tool events:

```json
// Tool started
{
  "type": "tool_execution",
  "tool": "web_search",
  "parameters": { "query": "..." }
}

// Tool completed
{
  "type": "tool_result",
  "tool": "web_search",
  "success": true,
  "result": "..."
}
```

The dashboard displays these in real-time during agent processing.

---

## Adding Custom Tools

Custom tools can be added through the skill system. See [Skills](/docs/skills) for details on creating tools with custom functionality.
