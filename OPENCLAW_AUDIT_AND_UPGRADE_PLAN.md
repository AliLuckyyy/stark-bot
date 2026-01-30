# OpenClaw Audit & StarkBot Upgrade Plan

## Implementation Status: COMPLETED

**Date:** 2026-01-30
**Status:** Phase 1-2 Implemented

### What Was Implemented:

1. **Scoped Committer Tool** (`committer.rs`) - Safe, scoped commits with:
   - Secret detection (13 patterns including API keys, tokens, private keys)
   - Sensitive file detection (.env, credentials.json, etc.)
   - Conventional commit format enforcement
   - Protected branch protection
   - Dry-run mode
   - Auto-push option

2. **Deploy Tool** (`deploy.rs`) - Full deployment lifecycle:
   - Push/pull/fetch with safety checks
   - PR creation via GitHub CLI
   - PR status monitoring
   - Workflow status monitoring
   - Trigger deployments
   - Merge PRs with auto-merge option

3. **PR Quality Check Tool** (`pr_quality.rs`) - Pre-PR validation:
   - Debug code detection (console.log, println!, dbg!, etc.)
   - TODO/FIXME without issue references
   - PR size validation (files, lines changed)
   - Diff summary generation

4. **Enhanced Git Tool** - Added operations:
   - push (with --force-with-lease safety)
   - pull (with --rebase)
   - fetch (with --prune)
   - clone
   - remote management

5. **Session Lane Serialization** (`session_lanes.rs`):
   - Per-session request serialization
   - Prevents race conditions
   - Workspace locking for git operations
   - Auto-pruning of idle sessions

6. **Extended Hook System** - New hook events:
   - BeforeCommit / AfterCommit
   - BeforePush / AfterPush
   - BeforePrCreate / AfterPrCreate
   - SessionStart / SessionEnd

---

## Executive Summary

This document analyzes how OpenClaw achieves effective code writing and GitHub integration, then provides a comprehensive plan to bring StarkBot to feature parity.

---

## Part 1: OpenClaw Analysis - How It Effectively Writes and Commits Code

### 1.1 Architecture Overview

OpenClaw uses a **Hub-and-Spoke Gateway Model**:

```
                    ┌─────────────────────┐
                    │      Gateway        │
                    │  (ws://127.0.0.1:   │
                    │      18789)         │
                    └──────────┬──────────┘
                               │
        ┌──────────────────────┼──────────────────────┐
        │                      │                      │
   ┌────▼────┐           ┌─────▼─────┐          ┌────▼────┐
   │ Channels │           │ Pi Agent  │          │ Clients │
   │ (Multi-  │           │ (Coding   │          │ (macOS, │
   │ platform)│           │  Runtime) │          │ iOS,CLI)│
   └──────────┘           └───────────┘          └─────────┘
```

**Key Insight**: The Gateway is the single coordination point, owning all messaging surfaces and exposing a typed WebSocket API. This eliminates state synchronization issues.

### 1.2 Effective Code Writing Mechanisms

#### A. Scoped Commit System
OpenClaw **never** uses `git add . && git commit` directly. Instead:

```bash
# Uses dedicated committer script for proper staging scope
scripts/committer "<msg>" <file1> <file2> ...
```

**Why this matters**: Prevents accidental commits of unrelated changes, secrets, or work-in-progress files.

#### B. Agent Loop with Tool Streaming

```
intake → context assembly → model inference → tool execution → streaming replies → persistence
```

The loop provides:
- **Immediate feedback**: Tool execution events stream in real-time
- **Block streaming**: Output appears as it's generated
- **Persistence**: All state survives restarts

#### C. Session Lane Serialization

```
Session Lane: [Agent A] → [Agent A] → [Agent A]
                           ↑
                     No interleaving
```

Runs serialize per session key, preventing:
- Tool execution races
- History corruption
- Concurrent branch conflicts

#### D. Skill-Based Code Generation

Skills are modular instruction sets:
```
workspace/skills/  (highest priority)
~/.openclaw/skills/  (user skills)
bundled/  (lowest priority)
```

Each skill contains `SKILL.md` with YAML frontmatter defining:
- Required binaries
- Environment variables
- OS targeting
- Activation patterns

### 1.3 GitHub Integration Details

#### A. Structured Git Operations
- Protected against dangerous commands
- Automatic formatting resolution
- Signal updates via `git pull --rebase`

#### B. PR Workflow
- Squash when history is messy
- Add PR author as co-contributor
- Workspace isolation per agent

#### C. Multi-Agent Safety Rules
- No `git stash` unless explicitly requested
- No worktree manipulation
- No concurrent branch switching
- Focus on scoped changes only

### 1.4 Plugin & Hook System

**Internal Hooks:**
- Bootstrap context injection
- Command event handling

**Plugin Hooks:**
- `before_agent_start`
- `before_tool_call`
- `tool_result_persist`
- Session lifecycle boundaries

This allows external systems to:
- Inject context before code generation
- Validate tool calls before execution
- Persist results to external systems
- Track session boundaries

---

## Part 2: StarkBot Current State Assessment

### 2.1 What StarkBot Already Has

| Feature | Status | Notes |
|---------|--------|-------|
| Agent Loop | ✅ | 25 iteration max, tool execution |
| WebSocket Broadcasting | ✅ | Real-time progress events |
| File Operations | ✅ | write_file, edit_file, apply_patch |
| Git Operations | ✅ | Full git command support |
| GitHub CLI | ✅ | Clone, PR, issues via `gh` |
| Memory System | ✅ | Semantic extraction & retrieval |
| Subagent System | ✅ | Parallel task execution (50 max) |
| Multi-Provider LLM | ✅ | Claude, OpenAI, Llama |
| Session Management | ✅ | Conversation history |
| Context Compaction | ✅ | Long conversation handling |
| Skill System | ✅ | 18 skill definitions |

### 2.2 Key Gaps vs OpenClaw

| Gap | Impact | Priority |
|-----|--------|----------|
| No scoped commit script | Risky commits | HIGH |
| No session lane serialization | Race conditions possible | HIGH |
| No plugin/hook system | Limited extensibility | MEDIUM |
| No skill registry (ClawHub-like) | Manual skill management | MEDIUM |
| No dedicated committer tool | Less reliable commits | HIGH |
| No automated formatting resolution | PR noise | MEDIUM |
| No branch conflict detection | Merge issues | MEDIUM |
| No workspace isolation per agent | State leakage | HIGH |

---

## Part 3: Upgrade Plan

### Phase 1: Core Git Safety (Week 1-2)

#### 1.1 Create Scoped Committer Tool

**New file**: `stark-backend/src/tools/builtin/committer.rs`

```rust
// Tool: committer
// Purpose: Safe, scoped git commits
//
// Features:
// - Only stages specified files
// - Validates files exist and are modified
// - Prevents secrets from being committed
// - Enforces conventional commit format
// - Adds Co-Authored-By automatically

pub struct CommitterTool;

impl Tool for CommitterTool {
    fn name(&self) -> &str { "committer" }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": { "type": "string", "description": "Commit message" },
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Files to stage and commit"
                },
                "conventional": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enforce conventional commit format"
                }
            },
            "required": ["message", "files"]
        })
    }
}
```

**Implementation requirements**:
1. Validate all files exist in workspace
2. Check files are actually modified (`git status --porcelain`)
3. Scan for secrets patterns (`.env`, `credentials`, API keys)
4. Validate conventional commit format if enabled
5. Stage only specified files
6. Create commit with agent attribution

#### 1.2 Add Workspace Isolation

**Modify**: `stark-backend/src/tools/context.rs`

```rust
pub struct ToolContext {
    // Existing fields...

    // NEW: Per-agent workspace isolation
    pub agent_workspace: PathBuf,  // /workspace/agents/{agent_id}/
    pub shared_workspace: PathBuf, // /workspace/shared/
    pub isolation_mode: WorkspaceIsolation,
}

pub enum WorkspaceIsolation {
    Shared,      // All agents share workspace (current behavior)
    Isolated,    // Each agent gets own workspace
    Hybrid,      // Personal + shared directories
}
```

#### 1.3 Session Lane Serialization

**New file**: `stark-backend/src/execution/session_lanes.rs`

```rust
use tokio::sync::Semaphore;
use std::collections::HashMap;

pub struct SessionLaneManager {
    // One semaphore per session for serialization
    lanes: DashMap<String, Arc<Semaphore>>,
    // Global lane for cross-session operations
    global_lane: Arc<Semaphore>,
}

impl SessionLaneManager {
    pub async fn acquire_lane(&self, session_id: &str) -> LaneGuard {
        let semaphore = self.lanes
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone();

        let permit = semaphore.acquire_owned().await.unwrap();
        LaneGuard { permit }
    }

    pub async fn acquire_global(&self) -> LaneGuard {
        let permit = self.global_lane.acquire_owned().await.unwrap();
        LaneGuard { permit }
    }
}
```

**Integration point**: `dispatcher.rs` should acquire session lane before starting agent loop.

---

### Phase 2: Enhanced Git Intelligence (Week 3-4)

#### 2.1 Branch Conflict Detection

**New file**: `stark-backend/src/tools/builtin/git_safety.rs`

```rust
pub struct GitSafetyChecker;

impl GitSafetyChecker {
    /// Check if current branch has upstream conflicts
    pub async fn check_conflicts(&self, workspace: &Path) -> Result<ConflictReport> {
        // 1. Fetch latest from remote
        // 2. Check if current branch diverged
        // 3. Identify conflicting files
        // 4. Return detailed report
    }

    /// Auto-resolve formatting-only changes
    pub async fn auto_resolve_formatting(&self, workspace: &Path) -> Result<Vec<String>> {
        // 1. Identify files with only whitespace/formatting changes
        // 2. Accept incoming for those files
        // 3. Return list of auto-resolved files
    }

    /// Prevent dangerous operations
    pub fn validate_command(&self, cmd: &str) -> Result<(), GitSafetyError> {
        let dangerous = [
            "push --force", "push -f",
            "reset --hard",
            "clean -f",
            "checkout .",  // discard all changes
        ];
        // Warn or block dangerous commands
    }
}
```

#### 2.2 Smart Commit Message Generation

**Add to committer tool**:

```rust
impl CommitterTool {
    async fn generate_commit_message(&self, staged_files: &[PathBuf]) -> String {
        // 1. Analyze diffs
        // 2. Categorize changes (feat, fix, refactor, docs, etc.)
        // 3. Generate conventional commit message
        // 4. Include scope based on changed directories
    }
}
```

#### 2.3 PR Quality Checks

**New file**: `stark-backend/src/tools/builtin/pr_quality.rs`

```rust
pub struct PRQualityTool;

impl Tool for PRQualityTool {
    fn name(&self) -> &str { "pr_quality_check" }

    // Checks:
    // - All tests pass
    // - No debug code (console.log, println!, dbg!)
    // - No TODO/FIXME without issue references
    // - Conventional commit messages
    // - PR size within limits
    // - Required files updated (CHANGELOG, etc.)
}
```

---

### Phase 3: Plugin & Hook System (Week 5-6)

#### 3.1 Hook Registry

**New file**: `stark-backend/src/plugins/hooks.rs`

```rust
pub enum HookPoint {
    BeforeAgentStart,
    AfterAgentStart,
    BeforeToolCall,
    AfterToolCall,
    BeforeCommit,
    AfterCommit,
    SessionStart,
    SessionEnd,
}

pub struct HookRegistry {
    hooks: HashMap<HookPoint, Vec<Box<dyn Hook>>>,
}

#[async_trait]
pub trait Hook: Send + Sync {
    async fn execute(&self, context: &HookContext) -> Result<HookResult>;
}

pub enum HookResult {
    Continue,
    Modify(Value),  // Modify the context/parameters
    Abort(String),  // Stop execution with reason
}
```

#### 3.2 Plugin Loader

**New file**: `stark-backend/src/plugins/loader.rs`

```rust
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
    hook_registry: HookRegistry,
}

impl PluginManager {
    pub async fn load_plugins(&mut self, plugin_dir: &Path) -> Result<()> {
        // 1. Scan for plugin manifests
        // 2. Validate dependencies
        // 3. Load in dependency order
        // 4. Register hooks
    }
}
```

#### 3.3 Built-in Hooks

Create default hooks for:
- **Secret detection**: Block commits containing secrets
- **Format validation**: Auto-format code before commit
- **Test runner**: Run tests before PR creation
- **Changelog updater**: Add entries to CHANGELOG

---

### Phase 4: Skill Registry & Management (Week 7-8)

#### 4.1 Skill Versioning

**Modify**: `stark-backend/src/skills/mod.rs`

```rust
pub struct Skill {
    pub name: String,
    pub version: semver::Version,
    pub source: SkillSource,
    pub dependencies: Vec<SkillDependency>,
    pub checksum: String,
}

pub enum SkillSource {
    Bundled,           // Built into StarkBot
    Local(PathBuf),    // User's local skills
    Workspace(PathBuf), // Project-specific skills
    Remote(Url),       // Downloaded from registry
}
```

#### 4.2 Skill Registry Client

**New file**: `stark-backend/src/skills/registry.rs`

```rust
pub struct SkillRegistry {
    registry_url: Url,
    cache_dir: PathBuf,
}

impl SkillRegistry {
    pub async fn search(&self, query: &str) -> Result<Vec<SkillMetadata>>;
    pub async fn install(&self, name: &str, version: Option<&str>) -> Result<Skill>;
    pub async fn update(&self, name: &str) -> Result<Skill>;
    pub async fn uninstall(&self, name: &str) -> Result<()>;
}
```

#### 4.3 Skill Priority System

Implement precedence:
1. Workspace skills (`./skills/`) - highest
2. User skills (`~/.starkbot/skills/`)
3. Installed skills (`~/.starkbot/registry/`)
4. Bundled skills - lowest

---

### Phase 5: Advanced Code Intelligence (Week 9-10)

#### 5.1 AST-Aware Editing

**New file**: `stark-backend/src/tools/builtin/smart_edit.rs`

```rust
pub struct SmartEditTool;

impl SmartEditTool {
    /// Edit by symbol name rather than line numbers
    pub async fn edit_symbol(
        &self,
        file: &Path,
        symbol: &str,  // e.g., "MyClass.my_method"
        new_content: &str,
    ) -> Result<()>;

    /// Add import statement intelligently
    pub async fn add_import(
        &self,
        file: &Path,
        import: &str,
    ) -> Result<()>;

    /// Rename symbol across codebase
    pub async fn rename_symbol(
        &self,
        workspace: &Path,
        old_name: &str,
        new_name: &str,
    ) -> Result<RenameReport>;
}
```

#### 5.2 Test Generation Tool

**New file**: `stark-backend/src/tools/builtin/test_gen.rs`

```rust
pub struct TestGeneratorTool;

impl Tool for TestGeneratorTool {
    fn name(&self) -> &str { "generate_tests" }

    // Features:
    // - Analyze function signatures
    // - Generate test cases for edge cases
    // - Create mocks for dependencies
    // - Follow project's test patterns
}
```

#### 5.3 Documentation Generator

**New file**: `stark-backend/src/tools/builtin/doc_gen.rs`

```rust
pub struct DocGeneratorTool;

impl Tool for DocGeneratorTool {
    fn name(&self) -> &str { "generate_docs" }

    // Features:
    // - Extract doc comments
    // - Generate API documentation
    // - Create README sections
    // - Update CHANGELOG entries
}
```

---

## Implementation Priority Matrix

| Feature | Effort | Impact | Priority |
|---------|--------|--------|----------|
| Scoped Committer | Medium | High | P0 |
| Session Serialization | Medium | High | P0 |
| Workspace Isolation | High | High | P0 |
| Branch Conflict Detection | Low | Medium | P1 |
| Smart Commit Messages | Low | Medium | P1 |
| Hook System | High | High | P1 |
| Plugin Loader | High | Medium | P2 |
| Skill Registry | High | Medium | P2 |
| AST-Aware Editing | Very High | High | P2 |
| Test Generation | High | Medium | P3 |
| Doc Generation | Medium | Low | P3 |

---

## Quick Wins (Implement This Week)

### 1. Add Committer Tool Wrapper

Create a simple shell script first, then implement in Rust:

```bash
#!/bin/bash
# scripts/committer.sh
set -e

MSG="$1"
shift
FILES="$@"

# Validate no secrets
for f in $FILES; do
    if [[ "$f" =~ \.(env|pem|key)$ ]]; then
        echo "ERROR: Cannot commit sensitive file: $f"
        exit 1
    fi
done

# Stage only specified files
git add $FILES

# Commit with attribution
git commit -m "$MSG

Co-Authored-By: StarkBot <bot@starkbot.ai>"
```

### 2. Add Git Safety Checks to Existing Tool

**Modify**: `stark-backend/src/tools/builtin/git.rs`

```rust
fn validate_command(&self, operation: &str, args: &[String]) -> Result<()> {
    let dangerous_patterns = [
        ("push", vec!["--force", "-f"]),
        ("reset", vec!["--hard"]),
        ("clean", vec!["-f", "-fd"]),
    ];

    for (op, patterns) in dangerous_patterns {
        if operation == op && args.iter().any(|a| patterns.contains(&a.as_str())) {
            return Err(ToolError::DangerousOperation(format!(
                "Dangerous git operation: {} {}. Use with explicit confirmation.",
                operation, args.join(" ")
            )));
        }
    }
    Ok(())
}
```

### 3. Add Session Locking

**Modify**: `stark-backend/src/channels/dispatcher.rs`

Add simple mutex-based session locking before full lane implementation:

```rust
lazy_static! {
    static ref SESSION_LOCKS: DashMap<String, Arc<Mutex<()>>> = DashMap::new();
}

async fn dispatch_message(&self, msg: Message) -> Result<Response> {
    let lock = SESSION_LOCKS
        .entry(msg.session_id.clone())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();

    let _guard = lock.lock().await;
    // ... existing dispatch logic
}
```

---

## Success Metrics

After implementing this plan, StarkBot should:

1. **Zero accidental commits**: No unintended files ever committed
2. **Zero race conditions**: Concurrent messages to same session handled safely
3. **Zero force push disasters**: Dangerous git operations blocked by default
4. **50% faster PR creation**: Automated formatting, tests, changelog
5. **90% commit message quality**: Conventional commits enforced
6. **Pluggable architecture**: External tools can hook into workflow

---

## Appendix: File Locations for Changes

```
stark-backend/src/
├── tools/
│   └── builtin/
│       ├── committer.rs          # NEW: Scoped commit tool
│       ├── git_safety.rs         # NEW: Safety checks
│       ├── pr_quality.rs         # NEW: PR validation
│       ├── smart_edit.rs         # NEW: AST-aware editing
│       ├── test_gen.rs           # NEW: Test generation
│       ├── doc_gen.rs            # NEW: Doc generation
│       └── git.rs                # MODIFY: Add safety validation
├── execution/
│   └── session_lanes.rs          # NEW: Session serialization
├── plugins/
│   ├── mod.rs                    # NEW: Plugin system
│   ├── hooks.rs                  # NEW: Hook registry
│   └── loader.rs                 # NEW: Plugin loader
├── skills/
│   └── registry.rs               # NEW: Skill registry client
└── channels/
    └── dispatcher.rs             # MODIFY: Add session locking
```

---

*Report generated: 2026-01-30*
*Based on: OpenClaw v1.x analysis vs StarkBot current state*
