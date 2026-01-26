---
name: StarkBot Documentation
---

Welcome to the StarkBot documentation. StarkBot is a cloud-deployable agentic assistant that interfaces with multiple messaging platforms and integrates AI-powered conversation handling with task automation.

## What is StarkBot?

StarkBot is an autonomous AI assistant framework that:

- **Accepts messages** from multiple platforms (Telegram, Slack, Discord, Web)
- **Leverages AI** (Claude, OpenAI, or Llama) with tool-calling capabilities
- **Executes tools** (web search, file operations, shell commands, messaging)
- **Stores memories** for long-term context awareness
- **Automates tasks** via cron scheduling and heartbeat triggers
- **Provides a dashboard** for configuration, monitoring, and management

## Key Features

### Multi-Platform Messaging
Connect to Telegram, Slack, and Discord simultaneously. Messages are normalized and processed through a unified pipeline.

### AI-Powered Conversations
Support for multiple AI providers with configurable models and parameters. Extended thinking capabilities for complex tasks.

### Tool Execution
Built-in tools for web search, file operations, shell command execution, and cross-platform messaging.

### Skills System
Extensible custom skills that can be uploaded as Markdown or ZIP files.

### Scheduling & Automation
CRON-based job scheduling and heartbeat triggers for recurring tasks.

### Real-Time Updates
WebSocket-based event broadcasting for live tool execution progress and message updates.

## Quick Links

- [Getting Started](/docs/getting-started) - Set up and run StarkBot
- [Architecture](/docs/architecture) - Understand the system design
- [API Reference](/docs/api) - Backend API endpoints
- [Tools](/docs/tools) - Available tools and usage
- [Skills](/docs/skills) - Creating custom skills
- [Channels](/docs/channels) - Platform integrations
- [Scheduling](/docs/scheduling) - Cron jobs and heartbeat
- [Configuration](/docs/configuration) - Environment variables

## Tech Stack

| Component | Technology |
|-----------|------------|
| Backend | Rust + Actix-web |
| Frontend | React + TypeScript + Vite |
| Database | SQLite |
| Styling | Tailwind CSS |
| WebSocket | tokio-tungstenite |
| AI Providers | Anthropic Claude, OpenAI, Llama |
