---
name: Channels
---

Channels connect StarkBot to messaging platforms. Messages from any channel are processed through the same AI pipeline.

## Supported Platforms

| Platform | Status | Features |
|----------|--------|----------|
| Telegram | Supported | Bot API, user identification |
| Slack | Supported | Bot + App tokens, threads |
| Discord | Supported | Serenity library, guilds |
| Web | Built-in | Dashboard chat interface |

---

## Telegram

### Setup

1. Create a bot with [@BotFather](https://t.me/BotFather)
2. Get your bot token
3. Add the channel in StarkBot dashboard

### Configuration

```json
{
  "platform": "telegram",
  "name": "My Telegram Bot",
  "config": {
    "bot_token": "123456789:ABCdefGHI..."
  }
}
```

### Features

- **Polling** - Receives messages via long polling
- **User Identification** - Tracks Telegram user IDs
- **Rich Responses** - Supports markdown formatting
- **Commands** - Bot commands like /start, /help

### Testing

1. Start the channel in dashboard
2. Open Telegram and message your bot
3. Verify response in both Telegram and dashboard logs

---

## Slack

### Setup

1. Create a Slack App at [api.slack.com](https://api.slack.com/apps)
2. Enable Socket Mode
3. Add Bot Token Scopes:
   - `chat:write`
   - `channels:history`
   - `channels:read`
   - `app_mentions:read`
4. Install to workspace
5. Get Bot Token and App Token

### Configuration

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

### Features

- **Socket Mode** - Real-time message delivery
- **Threads** - Respects Slack threading
- **Channels** - Works in public/private channels
- **DMs** - Direct message support
- **Mentions** - Responds to @mentions

### Event Subscriptions

Enable these events in your Slack App:
- `message.channels`
- `message.groups`
- `message.im`
- `app_mention`

---

## Discord

### Setup

1. Create application at [Discord Developer Portal](https://discord.com/developers/applications)
2. Create a Bot under the application
3. Get the bot token
4. Generate OAuth2 invite URL with scopes:
   - `bot`
   - `applications.commands`
5. Add bot permissions:
   - Send Messages
   - Read Message History
   - View Channels
6. Invite bot to your server

### Configuration

```json
{
  "platform": "discord",
  "name": "Discord Bot",
  "config": {
    "bot_token": "MTIz..."
  }
}
```

### Features

- **Serenity Framework** - Robust Discord library
- **Guilds** - Multi-server support
- **Channels** - Text channel messages
- **User Tracking** - Discord user identification
- **Rich Embeds** - Formatted responses

### Gateway Intents

The bot requires these intents:
- `GUILD_MESSAGES`
- `MESSAGE_CONTENT`
- `DIRECT_MESSAGES`

Enable "Message Content Intent" in Developer Portal.

---

## Web Channel

The built-in web channel is always available through the dashboard.

### Access

Navigate to **Agent Chat** in the dashboard.

### Features

- **Session Management** - Persistent conversations
- **Slash Commands** - Built-in commands (/help, /new, /reset)
- **Real-Time** - WebSocket-powered updates
- **Export** - Download conversation history

### Slash Commands

| Command | Description |
|---------|-------------|
| /help | Show available commands |
| /new | Start new conversation |
| /reset | Clear conversation history |
| /clear | Clear chat display |
| /skills | List available skills |
| /tools | List available tools |
| /model | Show current AI model |
| /export | Download conversation as JSON |

---

## Channel Management

### Adding Channels

1. Navigate to **Channels** in dashboard
2. Click **Add Channel**
3. Select platform
4. Enter configuration
5. Save and start

### Starting/Stopping

Each channel can be independently started or stopped:

- **Start** - Begin listening for messages
- **Stop** - Pause the channel (config preserved)

### Status Indicators

| Status | Meaning |
|--------|---------|
| Running | Actively listening |
| Stopped | Not listening |
| Error | Connection failed |

### Editing Channels

Click on a channel to edit its configuration. Changes take effect after restarting the channel.

### Deleting Channels

Remove channels you no longer need. This stops the channel and removes its configuration.

---

## Message Flow

All channels follow the same message processing flow:

```
Platform Message
       ↓
Channel Handler (telegram.rs, slack.rs, discord.rs)
       ↓
Normalize to NormalizedMessage
       ↓
Message Dispatcher
       ↓
AI Processing + Tool Execution
       ↓
Response back to Platform
```

### NormalizedMessage

Platform-specific messages are converted to a standard format:

```rust
struct NormalizedMessage {
    platform: String,      // "telegram", "slack", "discord", "web"
    channel_id: String,    // Platform-specific channel ID
    user_id: String,       // Platform-specific user ID
    username: String,      // Display name
    content: String,       // Message text
    timestamp: DateTime,   // When received
}
```

---

## Identities

StarkBot tracks user identities across platforms. View and manage these in the **Identities** page.

This enables:
- Cross-platform user recognition
- Personalized responses
- User-specific memory
