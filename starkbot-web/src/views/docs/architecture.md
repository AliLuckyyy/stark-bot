---
name: Architecture
---

StarkBot follows a modular architecture with clear separation between the backend services, frontend dashboard, and external integrations.

## System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        External Platforms                        │
│     Telegram          Slack           Discord          Web       │
└──────┬─────────────────┬───────────────┬───────────────┬────────┘
       │                 │               │               │
       ▼                 ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Channel Handlers                            │
│   telegram.rs      slack.rs       discord.rs      (REST API)    │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Message Dispatcher                            │
│  - Normalize messages    - Build AI prompts                      │
│  - Manage context        - Handle tool calls                     │
│  - Extract memories      - Store history                         │
└──────────────────────────┬──────────────────────────────────────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
         ┌────────┐  ┌─────────┐  ┌──────────┐
         │   AI   │  │  Tools  │  │ Database │
         │ Client │  │ Registry│  │ (SQLite) │
         └────────┘  └─────────┘  └──────────┘
```

---

## Backend Components

### Entry Point (`main.rs`)

The backend initializes components in this order:

1. Load environment configuration
2. Initialize logging
3. Create **AppState** with all services:
   - Database connection
   - Tool Registry
   - Skill Registry
   - WebSocket Gateway
   - Message Dispatcher
   - Execution Tracker
   - Scheduler

4. Start concurrent services:
   - HTTP server (port 8080)
   - WebSocket gateway (port 8081)
   - Channel listeners
   - Scheduler service

### Message Dispatcher

The dispatcher is the central hub for processing all messages:

```
Message In → Normalize → Get Context → Build Prompt → AI Call → Tool Loop → Store → Respond
```

**Processing Steps:**

1. **Normalize** - Convert platform-specific message to standard format
2. **Get Context** - Retrieve chat session history
3. **Build Prompt** - Assemble system prompt with context
4. **AI Call** - Send to configured AI provider
5. **Tool Loop** - Execute any tool calls (max 10 iterations)
6. **Extract Memories** - Parse `[REMEMBER:]` markers
7. **Store** - Save to chat history
8. **Respond** - Send back to originating platform

### AI Client

Unified interface for multiple AI providers:

| Provider | Model Examples |
|----------|---------------|
| Claude | claude-sonnet-4-20250514, claude-opus-4-20250514 |
| OpenAI | gpt-4, gpt-4-turbo, gpt-3.5-turbo |
| Llama | llama-3-70b, custom endpoints |

Features:
- Streaming responses
- Tool calling support
- Extended thinking (Claude)
- Configurable temperature

### Tool Registry

Built-in tools organized by access level:

| Group | Tools |
|-------|-------|
| Web | `web_search`, `web_fetch` |
| Filesystem | `read_file`, `write_file`, `list_files`, `apply_patch` |
| Exec | `exec` (shell commands) |
| Messaging | `agent_send` |
| System | `subagent`, `subagent_status` |

### Skill Registry

Custom skills loaded from:
- Markdown files (`.md`)
- ZIP archives (multiple files)
- Database storage

Skills extend the agent's capabilities with custom prompts and tool access.

### WebSocket Gateway

Real-time event broadcasting on port 8081:

- `channel_message` - New message received
- `tool_execution` - Tool started
- `tool_result` - Tool completed
- `agent_thinking` - AI processing
- `channel_error` - Error occurred

### Scheduler

Background job runner checking every 10 seconds:

- **Cron Jobs** - CRON expression-based scheduling
- **Heartbeat** - Time-interval triggers (hourly, daily, weekly)

---

## Frontend Architecture

### Tech Stack

- **React 18** - UI framework
- **TypeScript** - Type safety
- **React Router** - Client-side routing
- **Tailwind CSS** - Styling
- **Vite** - Build tool

### Page Structure

| Page | Purpose |
|------|---------|
| Dashboard | Overview and stats |
| Agent Chat | Conversation interface |
| Channels | Platform connections |
| Agent Settings | AI model config |
| API Keys | External service credentials |
| Tools | Available tools list |
| Skills | Custom skill management |
| Scheduling | Cron and heartbeat config |
| Sessions | Chat history |
| Memories | Long-term storage |
| Identities | User mappings |

### Real-Time Updates

The frontend maintains a WebSocket connection to the gateway:

```typescript
// Gateway client auto-connects and handles events
useGateway({
  onToolExecution: (event) => setProgress(event),
  onToolResult: (event) => updateResults(event),
  onMessage: (event) => addMessage(event)
});
```

---

## Data Flow Examples

### Message Processing

```
1. User sends "What's the weather in NYC?"
   ↓
2. Telegram handler receives update
   ↓
3. Dispatcher normalizes message
   ↓
4. Context retrieved from chat_sessions
   ↓
5. AI receives prompt with history
   ↓
6. AI decides to call web_search tool
   ↓
7. Tool executes, results fed back to AI
   ↓
8. AI generates final response
   ↓
9. Response stored in chat_sessions
   ↓
10. Response sent to Telegram
```

### Cron Job Execution

```
1. Scheduler checks for due jobs
   ↓
2. Job with cron "0 9 * * MON" is due
   ↓
3. Create NormalizedMessage with job.prompt
   ↓
4. Dispatcher processes like regular message
   ↓
5. Response logged to job history
   ↓
6. Next run time calculated
```

---

## Security Model

- **Authentication** - Session tokens validated against SECRET_KEY
- **API Keys** - Encrypted at rest in SQLite
- **Tool Restrictions** - Dangerous commands blocklisted
- **CORS** - Configured for allowed origins
