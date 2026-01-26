---
name: API Reference
---

All API endpoints require authentication via Bearer token (except login).

## Authentication

### Login

```
POST /api/auth/login
```

**Request Body:**
```json
{
  "secret_key": "your-secret-key"
}
```

**Response:**
```json
{
  "token": "session-uuid-token"
}
```

Use the token in subsequent requests:
```
Authorization: Bearer <token>
```

---

## Chat

### Send Message

```
POST /api/chat
```

**Request Body:**
```json
{
  "message": "Hello, StarkBot!",
  "session_id": "optional-session-id"
}
```

**Response:**
```json
{
  "response": "Hello! How can I help you today?",
  "session_id": "chat-session-uuid"
}
```

---

## API Keys

### List API Keys

```
GET /api/api-keys
```

**Response:**
```json
{
  "keys": [
    { "service": "anthropic", "configured": true },
    { "service": "openai", "configured": false },
    { "service": "brave_search", "configured": true }
  ]
}
```

### Add/Update API Key

```
POST /api/api-keys
```

**Request Body:**
```json
{
  "service": "anthropic",
  "api_key": "sk-ant-..."
}
```

### Delete API Key

```
DELETE /api/api-keys/:service
```

---

## Channels

### List Channels

```
GET /api/channels
```

**Response:**
```json
{
  "channels": [
    {
      "id": "uuid",
      "platform": "telegram",
      "name": "My Telegram Bot",
      "status": "running"
    }
  ]
}
```

### Create Channel

```
POST /api/channels
```

**Telegram:**
```json
{
  "platform": "telegram",
  "name": "My Bot",
  "config": {
    "bot_token": "123456:ABC..."
  }
}
```

**Slack:**
```json
{
  "platform": "slack",
  "name": "Slack Bot",
  "config": {
    "bot_token": "xoxb-...",
    "app_token": "xapp-..."
  }
}
```

**Discord:**
```json
{
  "platform": "discord",
  "name": "Discord Bot",
  "config": {
    "bot_token": "..."
  }
}
```

### Update Channel

```
PUT /api/channels/:id
```

### Delete Channel

```
DELETE /api/channels/:id
```

### Start/Stop Channel

```
POST /api/channels/:id/start
POST /api/channels/:id/stop
```

---

## Agent Settings

### Get Settings

```
GET /api/agent/settings
```

**Response:**
```json
{
  "provider": "claude",
  "model": "claude-sonnet-4-20250514",
  "temperature": 0.7,
  "max_tokens": 4096
}
```

### Update Settings

```
PUT /api/agent/settings
```

**Request Body:**
```json
{
  "provider": "claude",
  "model": "claude-opus-4-20250514",
  "temperature": 0.5
}
```

---

## Scheduling

### List Cron Jobs

```
GET /api/cron
```

**Response:**
```json
{
  "jobs": [
    {
      "id": "uuid",
      "name": "Daily Summary",
      "cron_expression": "0 9 * * *",
      "message": "Generate daily summary",
      "enabled": true,
      "next_run": "2024-01-15T09:00:00Z"
    }
  ]
}
```

### Create Cron Job

```
POST /api/cron
```

**Request Body:**
```json
{
  "name": "Weekly Report",
  "cron_expression": "0 9 * * MON",
  "message": "Generate weekly report and send to Discord"
}
```

### Delete Cron Job

```
DELETE /api/cron/:id
```

### Run Job Now

```
POST /api/cron/:id/run
```

### Pause/Resume Job

```
POST /api/cron/:id/pause
POST /api/cron/:id/resume
```

### Get Job History

```
GET /api/cron/:id/runs
```

### Heartbeat Config

```
GET /api/heartbeat
PUT /api/heartbeat
```

---

## Skills

### List Skills

```
GET /api/skills
```

**Response:**
```json
{
  "skills": [
    {
      "name": "weather",
      "description": "Get weather information",
      "arguments": ["location"]
    }
  ]
}
```

### Upload Skill

```
POST /api/skills/upload
Content-Type: multipart/form-data
```

Upload a `.md` file or `.zip` archive.

### Delete Skill

```
DELETE /api/skills/:name
```

---

## Tools

### List Tools

```
GET /api/tools
```

**Response:**
```json
{
  "tools": [
    {
      "name": "web_search",
      "description": "Search the web",
      "group": "web",
      "parameters": {
        "query": "string"
      }
    }
  ]
}
```

---

## Memories

### List Memories

```
GET /api/memories
```

### Delete Memory

```
DELETE /api/memories/:id
```

---

## Sessions

### List Sessions

```
GET /api/sessions
```

### Get Session Messages

```
GET /api/sessions/:id
```

---

## Error Responses

All endpoints return errors in this format:

```json
{
  "error": "Error message description"
}
```

Common HTTP status codes:
- `401` - Unauthorized (invalid/missing token)
- `404` - Not found
- `400` - Bad request (invalid input)
- `500` - Internal server error
