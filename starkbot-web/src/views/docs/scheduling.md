---
name: Scheduling
---

StarkBot supports automated task execution through cron jobs and heartbeat triggers.

## Overview

| Type | Description | Use Case |
|------|-------------|----------|
| Cron Jobs | CRON expression scheduling | Specific times/dates |
| Heartbeat | Interval-based triggers | Regular check-ins |

---

## Cron Jobs

Cron jobs execute prompts at specified times using standard CRON expressions.

### Creating a Cron Job

1. Navigate to **Scheduling** in dashboard
2. Click **Add Cron Job**
3. Fill in:
   - **Name** - Descriptive name
   - **CRON Expression** - When to run
   - **Message** - Prompt to send to agent

### CRON Expression Format

```
┌───────────── minute (0-59)
│ ┌───────────── hour (0-23)
│ │ ┌───────────── day of month (1-31)
│ │ │ ┌───────────── month (1-12)
│ │ │ │ ┌───────────── day of week (0-6, Sunday=0)
│ │ │ │ │
* * * * *
```

### Common Patterns

| Expression | Description |
|------------|-------------|
| `0 9 * * *` | Every day at 9:00 AM |
| `0 9 * * MON` | Every Monday at 9:00 AM |
| `0 */4 * * *` | Every 4 hours |
| `30 8 * * MON-FRI` | Weekdays at 8:30 AM |
| `0 0 1 * *` | First day of month at midnight |
| `*/15 * * * *` | Every 15 minutes |

### Example Jobs

#### Daily Summary
```
Name: Daily Summary
CRON: 0 18 * * *
Message: Generate a summary of today's activities and notable events
```

#### Weekly Report
```
Name: Weekly Report
CRON: 0 9 * * MON
Message: Create a weekly report of all completed tasks and send to Discord
```

#### Hourly Check
```
Name: System Health Check
CRON: 0 * * * *
Message: Check system health and alert if any issues detected
```

### Job Management

| Action | Description |
|--------|-------------|
| Run Now | Execute immediately |
| Pause | Temporarily disable |
| Resume | Re-enable paused job |
| Delete | Remove permanently |

### Execution History

View past runs for each job:
- Execution time
- Success/failure status
- Response summary

---

## Heartbeat

Heartbeat provides simpler interval-based scheduling without CRON syntax.

### Configuration

Navigate to **Scheduling** > **Heartbeat** tab.

### Intervals

| Interval | Description |
|----------|-------------|
| Hourly | Every hour on the hour |
| Daily | Once per day at specified time |
| Weekly | Once per week on specified day |
| Custom | Custom interval in minutes |

### Example Configuration

```json
{
  "enabled": true,
  "interval": "daily",
  "time": "09:00",
  "message": "Good morning! Here's your daily briefing.",
  "channel_id": "discord-channel-uuid"
}
```

### Use Cases

- **Morning briefings** - Daily summary at start of day
- **Health checks** - Periodic system monitoring
- **Reminders** - Regular notifications
- **Data sync** - Periodic data updates

---

## How Scheduling Works

### Scheduler Service

The backend runs a scheduler service that:

1. Checks for due jobs every 10 seconds
2. Executes due jobs via the message dispatcher
3. Records execution results
4. Calculates next run time

### Execution Flow

```
Scheduler Check (every 10s)
       ↓
Find Due Jobs
       ↓
For Each Job:
  ├─→ Broadcast "job_started" event
  ├─→ Create NormalizedMessage
  ├─→ Dispatcher.dispatch()
  ├─→ AI processes message
  ├─→ Record execution result
  └─→ Update next_run_time
```

### Job Status

| Status | Description |
|--------|-------------|
| Active | Scheduled and will run |
| Paused | Temporarily disabled |
| Running | Currently executing |

---

## Best Practices

### 1. Clear Job Names
Use descriptive names that indicate purpose:
- "Daily Sales Report" not "Job 1"
- "Monday Team Standup" not "Weekly"

### 2. Appropriate Intervals
- Don't schedule too frequently (resource usage)
- Consider timezone implications
- Avoid overlapping jobs

### 3. Idempotent Messages
Design prompts that handle repeated execution:
- "Generate today's report" (good)
- "Append to report" (may cause issues)

### 4. Error Handling
Include instructions for handling failures:
```
Generate the daily report. If data is unavailable,
notify the team via Discord with the error details.
```

### 5. Monitor History
Regularly check execution history for:
- Failed jobs
- Unexpected results
- Performance issues

---

## Integration with Skills

Combine scheduling with skills for powerful automation:

```
Name: Weather Alert
CRON: 0 7 * * *
Message: Use the weather skill to check conditions in Seattle.
         If rain is expected, send a reminder to bring an umbrella.
```

```
Name: PR Review Reminder
CRON: 0 10 * * MON-FRI
Message: Use the github-pr skill to list open PRs older than 2 days.
         Send a summary to Slack if any need attention.
```

---

## Timezone Handling

- All times are in UTC by default
- The dashboard displays times in your local timezone
- CRON expressions use server timezone (typically UTC)

Tip: Use explicit timezone references in job names:
- "Daily Report (9 AM PT)"
- "Standup Reminder (UTC)"
