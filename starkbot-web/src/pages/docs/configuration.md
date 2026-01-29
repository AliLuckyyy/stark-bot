---
name: Configuration
---

StarkBot is configured through environment variables and the web dashboard.

## Environment Variables

Set these in your `.env` file or container environment.

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| SECRET_KEY | Authentication secret | `my-secure-secret-key` |

### Optional

| Variable | Default | Description |
|----------|---------|-------------|
| PORT | 8080 | HTTP server port |
| GATEWAY_PORT | 8081 | WebSocket gateway port |
| DATABASE_URL | ./.db/stark.db | SQLite database path |
| RUST_LOG | info | Log level (debug, info, warn, error) |

### Example .env

```bash
# Authentication
SECRET_KEY=my-super-secret-key-change-me

# Server ports
PORT=8080
GATEWAY_PORT=8081

# Database
DATABASE_URL=./.db/stark.db

# Logging
RUST_LOG=info
```

---

## Dashboard Configuration

Most settings are configured through the web dashboard.

### API Keys

Navigate to **API Keys** to configure external services:

| Service | Purpose | Required For |
|---------|---------|--------------|
| anthropic | Claude AI | Claude models |
| openai | OpenAI API | GPT models |
| brave_search | Brave Search API | web_search tool |
| serpapi | SerpAPI | web_search (alternative) |
| github | GitHub API | GitHub integrations |

### Agent Settings

Configure AI behavior in **Agent Settings**:

| Setting | Description | Options |
|---------|-------------|---------|
| Provider | AI provider | claude, openai, llama |
| Model | Specific model | claude-sonnet-4-20250514, gpt-4, etc. |
| Temperature | Creativity level | 0.0 - 1.0 |
| Max Tokens | Response length limit | 1024 - 8192 |

---

## Docker Configuration

### docker-compose.yml (Production)

```yaml
version: '3.8'
services:
  starkbot:
    build: .
    ports:
      - "8080:8080"
      - "8081:8081"
    volumes:
      - ./data:/app/.db
    environment:
      - SECRET_KEY=${SECRET_KEY}
      - PORT=8080
      - GATEWAY_PORT=8081
      - DATABASE_URL=./.db/stark.db
      - RUST_LOG=info
```

### docker-compose.dev.yml (Development)

```yaml
version: '3.8'
services:
  backend:
    build:
      context: .
      dockerfile: Dockerfile.dev
    ports:
      - "8082:8082"
      - "8081:8081"
    volumes:
      - ./stark-backend:/app/stark-backend
      - ./data:/app/.db
    environment:
      - SECRET_KEY=dev-secret
      - PORT=8082
      - GATEWAY_PORT=8081
      - RUST_LOG=debug

  frontend:
    build:
      context: ./stark-frontend
      dockerfile: Dockerfile.dev
    ports:
      - "8080:8080"
    volumes:
      - ./stark-frontend:/app
    depends_on:
      - backend
```

---

## Security Configuration

### SECRET_KEY

The SECRET_KEY is critical for security:

- Used for session token generation
- Required for dashboard login
- Should be a strong, unique value

**Generate a secure key:**

```bash
openssl rand -base64 32
```

### API Key Storage

API keys are encrypted before storage in SQLite:

- Never stored in plain text
- Decrypted only when needed
- Not exposed in API responses

### CORS

CORS is configured to allow requests from:
- Same origin
- Configured frontend URLs

---

## Logging

### Log Levels

Set with `RUST_LOG` environment variable:

| Level | Description |
|-------|-------------|
| error | Only errors |
| warn | Warnings and errors |
| info | General information (recommended) |
| debug | Detailed debugging |
| trace | Very verbose |

### Structured Logging

Logs include:
- Timestamp
- Level
- Module path
- Message

Example:
```
2024-01-15T10:30:00Z INFO stark_backend::dispatcher - Processing message from telegram
2024-01-15T10:30:01Z DEBUG stark_backend::ai::claude - Sending request to Claude API
```

---

## Database

### SQLite Configuration

StarkBot uses SQLite for data persistence:

```
DATABASE_URL=./.db/stark.db
```

### Tables

| Table | Purpose |
|-------|---------|
| auth_sessions | User authentication |
| api_keys | Encrypted service credentials |
| channels | Platform configurations |
| agent_settings | AI model settings |
| chat_sessions | Conversation history |
| cron_jobs | Scheduled tasks |
| heartbeat_config | Interval triggers |
| memories | Long-term storage |
| identities | User mappings |
| skills | Custom skills |

### Backup

SQLite database is a single file. Back up by copying:

```bash
cp .db/stark.db .db/stark.db.backup
```

### Volume Mounting

In Docker, mount the database directory:

```yaml
volumes:
  - ./data:/app/.db
```

---

## Network Configuration

### Ports

| Port | Service | Protocol |
|------|---------|----------|
| 8080 | HTTP Server | TCP |
| 8081 | WebSocket Gateway | TCP |

### Firewall Rules

For production, ensure:
- Port 8080 accessible for web traffic
- Port 8081 accessible for WebSocket

### Reverse Proxy (Optional)

Example nginx configuration:

```nginx
server {
    listen 80;
    server_name starkbot.example.com;

    location / {
        proxy_pass http://localhost:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
    }

    location /ws {
        proxy_pass http://localhost:8081;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

---

## Troubleshooting

### Common Issues

**Can't connect to dashboard:**
- Check PORT environment variable
- Verify firewall allows port 8080
- Check container logs

**WebSocket not connecting:**
- Verify GATEWAY_PORT is correct
- Check browser console for errors
- Ensure WebSocket port is accessible

**API key not working:**
- Verify key is correct
- Check service-specific requirements
- Review logs for API errors

**Database errors:**
- Ensure DATABASE_URL path is writable
- Check disk space
- Verify volume mounting in Docker
