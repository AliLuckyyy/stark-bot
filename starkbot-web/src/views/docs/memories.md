---
name: Memories
---

StarkBot's memory system enables long-term context retention across conversations.

## Overview

Memories persist information beyond a single chat session, allowing the agent to:

- Remember user preferences
- Recall important facts
- Maintain context across sessions
- Build up knowledge over time

---

## Memory Types

### Conversation Memory

Short-term memory within a chat session:
- Recent messages
- Current context
- Tool execution history

Stored in `chat_sessions` table.

### Long-Term Memory

Persistent memories extracted from conversations:
- Important facts
- User preferences
- Recurring information

Stored in `memories` table.

### Daily Logs

Date-specific memories for recurring context:
- Daily activities
- Notable events
- Temporal information

---

## Memory Markers

The agent extracts memories using special markers in responses:

### [REMEMBER:]

Store a permanent memory:

```
[REMEMBER: User prefers dark mode and shorter responses]
```

### [REMEMBER_IMPORTANT:]

Store a high-priority memory:

```
[REMEMBER_IMPORTANT: User's project deadline is January 15th]
```

### [DAILY_LOG:]

Add to date-specific log:

```
[DAILY_LOG: Completed code review for PR #123]
```

---

## How Memories Work

### Storage Flow

```
Agent Response
      ↓
Dispatcher extracts markers
      ↓
Parse memory content
      ↓
Store in database
      ↓
Available for future context
```

### Retrieval Flow

```
New Message
      ↓
Build context prompt
      ↓
Include relevant memories
      ↓
AI has access to stored knowledge
```

### Memory Injection

Memories are included in the system prompt:

```
## Long-Term Memories
- User prefers dark mode and shorter responses
- Project deadline is January 15th
- Favorite programming language: Rust

## Today's Log
- Completed code review for PR #123
- Deployed v2.1 to staging
```

---

## Managing Memories

### View Memories

Navigate to **Memories** in the dashboard to see all stored memories.

### Memory Details

Each memory shows:
- Content
- Creation date
- Priority level
- Source (which conversation)

### Delete Memories

Remove outdated or incorrect memories:

1. Find the memory in the list
2. Click the delete button
3. Confirm deletion

> **Note:** Deleted memories are permanently removed.

---

## Best Practices

### 1. Be Specific

Good:
```
[REMEMBER: User's timezone is Pacific (UTC-8)]
```

Not as useful:
```
[REMEMBER: User is on the west coast]
```

### 2. Avoid Duplicates

The agent should check existing memories before creating new ones to avoid redundancy.

### 3. Use Appropriate Priority

- `[REMEMBER:]` for general preferences and facts
- `[REMEMBER_IMPORTANT:]` for critical information
- `[DAILY_LOG:]` for time-sensitive events

### 4. Periodic Cleanup

Regularly review memories to:
- Remove outdated information
- Correct inaccuracies
- Consolidate related memories

---

## Memory in Action

### Example Conversation

**User:** I prefer responses in bullet points.

**Agent:** I'll remember that preference.
`[REMEMBER: User prefers responses formatted as bullet points]`

**Later conversation:**

**User:** What were the main topics we discussed yesterday?

**Agent:** (Uses daily log from yesterday to provide summary in bullet format)

---

### Skill Integration

Skills can leverage memories for personalized behavior:

```markdown
---
name: personalized-greeting
description: Greet user with personalized message
tools: []
---

# Personalized Greeting

Check memories for:
- User's name
- Preferred greeting style
- Recent activities

Generate a greeting that acknowledges the user personally
and references recent context if appropriate.
```

---

## API Access

### List Memories

```
GET /api/memories
```

Response:
```json
{
  "memories": [
    {
      "id": "uuid",
      "content": "User prefers dark mode",
      "priority": "normal",
      "created_at": "2024-01-15T10:30:00Z"
    }
  ]
}
```

### Delete Memory

```
DELETE /api/memories/:id
```

---

## Technical Details

### Storage

Memories are stored in SQLite:

```sql
CREATE TABLE memories (
  id TEXT PRIMARY KEY,
  content TEXT NOT NULL,
  priority TEXT DEFAULT 'normal',
  source_session TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### Context Window

Memories are included in the context up to a configurable limit to avoid exceeding token limits. Higher priority memories are included first.

### Privacy

- Memories are stored locally in your SQLite database
- No external transmission of memory data
- You have full control over what's stored and can delete at any time
