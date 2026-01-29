# Agent Test Fixture

A minimal test harness for testing agentic tool loops without booting the full app.

## Usage

```bash
cargo run --bin agent_test
```

The binary automatically loads environment variables from `.env` in the project root.

## Environment Variables

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| `TEST_AGENT_ENDPOINT` | API endpoint URL | `https://api.moonshot.ai/v1/chat/completions` |
| `TEST_AGENT_SECRET` | API key for the endpoint | `sk-...` |

### Optional

| Variable | Description | Default |
|----------|-------------|---------|
| `TEST_QUERY` | The query to send to the agent | `What's the weather in Boston?` |
| `TEST_AGENT_ARCHETYPE` | Model archetype to use | `kimi` |

## Supported Archetypes

| Archetype | Default Model | Notes |
|-----------|---------------|-------|
| `kimi` | `kimi-k2-turbo-preview` | Moonshot AI |
| `llama` | `llama3.3` | Expects JSON response format |
| `openai` | `gpt-4` | OpenAI-compatible APIs |
| `claude` | `claude-3-sonnet` | Anthropic |

## Test Tools

The fixture provides three mock tools for testing the agent loop:

- **get_weather** - Returns mock weather data for a location
- **web_search** - Returns mock search results
- **calculator** - Returns mock calculation results

## Example

```bash
# Using .env file
cargo run --bin agent_test

# Or inline
TEST_QUERY="what's 2 + 2?" \
TEST_AGENT_ENDPOINT="https://api.moonshot.ai/v1/chat/completions" \
TEST_AGENT_SECRET="your-api-key" \
TEST_AGENT_ARCHETYPE="kimi" \
cargo run --bin agent_test
```

## Output

The fixture prints detailed debug output for each iteration:
- Request body (JSON)
- Response body (JSON)
- Tool calls detected and their results
- Final response when the loop completes

The loop runs for a maximum of 10 iterations before timing out.
