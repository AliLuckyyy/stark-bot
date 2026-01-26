---
name: Getting Started
---

This guide walks you through setting up and running StarkBot.

## Prerequisites

- Docker and Docker Compose
- Node.js 18+ (for local frontend development)
- Rust toolchain (for local backend development)

## Quick Start with Docker

### 1. Clone the Repository

```bash
git clone https://github.com/your-org/stark-bot.git
cd stark-bot
```

### 2. Configure Environment

Create a `.env` file in the project root:

```bash
SECRET_KEY=your-secure-secret-key
PORT=8080
GATEWAY_PORT=8081
DATABASE_URL=./.db/stark.db
RUST_LOG=info
```

> **Important:** Use a strong, unique `SECRET_KEY` - this is used for authentication.

### 3. Run with Docker Compose

**Production:**
```bash
docker-compose up --build
```

**Development (with hot-reload):**
```bash
docker-compose -f docker-compose.dev.yml up --build
```

### 4. Access the Dashboard

Open `http://localhost:8080` in your browser. Log in using your `SECRET_KEY`.

---

## Local Development Setup

### Backend

```bash
cd stark-backend
cargo run
```

The backend runs on port 8080 (HTTP) and 8081 (WebSocket).

### Frontend

```bash
cd stark-frontend
npm install
npm run dev
```

The frontend dev server runs with proxy configuration to the backend.

---

## Initial Configuration

After logging in, configure StarkBot:

### 1. Add API Keys

Navigate to **API Keys** and add credentials for:

- **Anthropic** - For Claude AI models
- **OpenAI** - For GPT models
- **Brave Search** or **SerpAPI** - For web search tool

### 2. Configure Agent Settings

Go to **Agent Settings** to select:

- AI Provider (Claude, OpenAI, Llama)
- Model (claude-sonnet-4-20250514, gpt-4, etc.)
- Temperature and other parameters

### 3. Connect Channels (Optional)

In **Channels**, add your messaging platforms:

- **Telegram**: Bot token from @BotFather
- **Slack**: Bot token and app token
- **Discord**: Bot token from Developer Portal

### 4. Test the Agent

Visit **Agent Chat** and send a message to verify everything works.

---

## Directory Structure

```
stark-bot/
├── stark-backend/          # Rust backend
│   └── src/
│       ├── main.rs         # Entry point
│       ├── controllers/    # API handlers
│       ├── channels/       # Platform integrations
│       ├── ai/             # AI provider clients
│       └── tools/          # Built-in tools
├── stark-frontend/         # React frontend
│   └── src/
│       ├── pages/          # Page components
│       ├── components/     # UI components
│       └── lib/            # Utilities
├── .env                    # Environment config
├── docker-compose.yml      # Production setup
└── docker-compose.dev.yml  # Development setup
```

---

## Next Steps

- [Architecture](/docs/architecture) - Understand how StarkBot works
- [Tools](/docs/tools) - Learn about available tools
- [Skills](/docs/skills) - Create custom skills
