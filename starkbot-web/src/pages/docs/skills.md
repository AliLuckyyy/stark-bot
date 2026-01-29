---
name: Skills
---

Skills extend StarkBot with custom capabilities. They're defined in Markdown format and can include prompts, instructions, and tool access.

## What Are Skills?

Skills are reusable modules that:

- Define specialized behaviors for the agent
- Can be triggered by name or context
- Provide domain-specific knowledge
- Can access specific tools

## Skill Format

Skills are defined in Markdown with YAML frontmatter:

```markdown
---
name: weather
description: Get weather information for a location
arguments:
  - name: location
    description: City or location name
    required: true
tools:
  - web_search
  - web_fetch
---

# Weather Skill

When asked about weather, follow these steps:

1. Use web_search to find current weather for the location
2. Use web_fetch to get detailed forecast if needed
3. Summarize the weather conditions clearly

Include:
- Current temperature
- Conditions (sunny, cloudy, rain, etc.)
- High/low for the day
- Notable alerts or warnings
```

## Frontmatter Fields

| Field | Type | Description |
|-------|------|-------------|
| name | string | Unique skill identifier |
| description | string | What the skill does |
| arguments | array | Parameters the skill accepts |
| tools | array | Tools the skill can use |

### Arguments

Each argument can have:

```yaml
arguments:
  - name: location
    description: The target location
    required: true
    default: "New York"
```

---

## Creating Skills

### Method 1: Upload via Dashboard

1. Navigate to **Skills** page
2. Click **Upload Skill**
3. Select a `.md` file or `.zip` archive
4. Skill is immediately available

### Method 2: ZIP Archive

For complex skills with multiple files:

```
my-skill.zip
├── skill.md          # Main skill definition
├── templates/        # Optional templates
│   └── report.md
└── data/             # Optional data files
    └── config.json
```

---

## Example Skills

### GitHub PR Skill

```markdown
---
name: github-pr
description: Create and manage GitHub pull requests
arguments:
  - name: action
    description: Action to perform (create, review, merge)
    required: true
  - name: repo
    description: Repository name (owner/repo)
    required: true
tools:
  - exec
  - read_file
  - write_file
---

# GitHub PR Skill

## Create PR
1. Check current branch with `git branch`
2. Ensure changes are committed
3. Push branch and create PR using gh CLI

## Review PR
1. Fetch PR details with `gh pr view`
2. Review changed files
3. Provide summary of changes

## Merge PR
1. Verify CI checks pass
2. Merge using `gh pr merge`
```

---

### Daily Summary Skill

```markdown
---
name: daily-summary
description: Generate daily activity summary
arguments:
  - name: date
    description: Date to summarize (defaults to today)
    required: false
tools:
  - read_file
  - agent_send
---

# Daily Summary Skill

Generate a summary of the day's activities:

1. Read activity logs from the database
2. Categorize by type (messages, tool uses, cron jobs)
3. Calculate key metrics
4. Format as a readable summary
5. Optionally send to configured channel
```

---

### Web Research Skill

```markdown
---
name: research
description: Conduct web research on a topic
arguments:
  - name: topic
    description: Topic to research
    required: true
  - name: depth
    description: Research depth (quick, standard, thorough)
    default: standard
tools:
  - web_search
  - web_fetch
---

# Web Research Skill

## Quick Research
- Single search query
- Top 3 results summarized

## Standard Research
- Multiple search queries
- Fetch and analyze top results
- Cross-reference information

## Thorough Research
- Comprehensive search coverage
- Deep dive into authoritative sources
- Fact verification across sources
- Structured report output
```

---

## Using Skills

### In Chat

Simply ask the agent to use the skill:

> "Use the weather skill to get the forecast for Tokyo"

Or the agent may automatically invoke a skill based on context.

### Via Slash Command

In the Agent Chat, type:

```
/skills
```

To see available skills and their descriptions.

### In Cron Jobs

Reference skills in scheduled tasks:

```json
{
  "name": "Morning Briefing",
  "cron_expression": "0 8 * * *",
  "message": "Use the daily-summary skill and send to Discord"
}
```

---

## Managing Skills

### View Skills

Navigate to **Skills** in the dashboard to see all installed skills.

### Delete Skills

Click the delete button next to any skill to remove it.

### Update Skills

Upload a new version with the same name to replace an existing skill.

---

## Best Practices

1. **Clear Names** - Use descriptive, lowercase names with hyphens
2. **Detailed Descriptions** - Help the AI understand when to use the skill
3. **Minimal Tools** - Only request tools the skill actually needs
4. **Step-by-Step Instructions** - Guide the AI through the process
5. **Error Handling** - Include instructions for common failure cases
