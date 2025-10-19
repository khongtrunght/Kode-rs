# Kode Architecture Summary for Rust Port

## System Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                   KODE CLI ENTRY POINT                          │
│                  (src/entrypoints/cli.tsx)                      │
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│              LAYER 1: USER INTERACTION (REPL)                   │
│                   (src/screens/REPL.tsx)                        │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ PromptInput  │  │MessageRenderer│  │PermissionUI  │         │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                  │
│  Input Modes: prompt | bash | koding                           │
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│           LAYER 2: ORCHESTRATION (Query System)                 │
│                      (src/query.ts)                             │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Message Flow Control                                     │  │
│  │  - LLM Query                                             │  │
│  │  - Tool Use Detection                                    │  │
│  │  - Concurrent/Serial Execution                          │  │
│  │  - Context Auto-Compaction                              │  │
│  └──────────────────────────────────────────────────────────┘  │
└──────────────────────────┬──────────────────────────────────────┘
                ┌──────────┴──────────┐
                │                     │
     ┌──────────▼──────────┐  ┌──────▼────────────┐
     │  LAYER 3: SERVICES  │  │ LAYER 3: TOOLS   │
     │  (External)         │  │ (Execution)      │
     └─────────────────────┘  └──────────────────┘
                │                     │
    ┌───────────┼───────────┐  ┌──────┴────────────┬──────────────┐
    │           │           │  │                   │              │
┌───▼──┐ ┌─────▼──┐ ┌──────▼──┐ ┌──────────┐ ┌────────────┐ ┌────────┐
│ LLM  │ │ Model  │ │ Config  │ │ File I/O │ │ Command    │ │ Web    │
│ Svcs │ │ Manager│ │ System  │ │ Tools    │ │Execution   │ │ Tools  │
└──────┘ └────────┘ └─────────┘ └──────────┘ └────────────┘ └────────┘
   │         │           │           │             │             │
   ▼         ▼           ▼           ▼             ▼             ▼
┌────────┬─────────┐  ┌────────┐  ┌─────────┐  ┌──────────┐  ┌────────┐
│Claude  │OpenAI   │  │JSON    │  │FileRead │  │BashTool  │  │WebSrch │
│API     │API      │  │Files   │  │FileEdit │  │Bash      │  │URLFetch│
│        │GPT-5    │  │        │  │FileWrite│  │          │  │        │
└────────┴─────────┘  └────────┘  └─────────┘  └──────────┘  └────────┘
```

---

## Component Dependency Graph

```
Core Dependencies:
  Message Types ←── Normalization Utils
       ↓
  Query System ←── LLM Services
       ↓
  Tool System ←── Message Types
       ↓
  REPL ←── Query System + Tool System

Tool Dependencies:
  Tool Core Interface
       ├─→ FileReadTool ──→ File I/O Utils
       ├─→ FileEditTool ──→ File I/O Utils + Diff Utils
       ├─→ BashTool ──────→ Subprocess Utils + Permission System
       ├─→ GrepTool ──────→ Regex + File Traversal
       ├─→ TaskTool ──────→ Agent System + Query System (recursive)
       ├─→ WebSearchTool
       └─→ URLFetcher ────→ HTML/Markdown Conversion

Service Dependencies:
  LLM Services
       ├─→ Model Manager
       ├─→ Retry Logic
       ├─→ Prompt Caching
       └─→ Cost Tracker

Configuration System
       ├─→ Model Manager
       ├─→ Permission System
       └─→ Agent System
```

---

## File Structure (TypeScript → Rust)

```
src/
├── entrypoints/
│   └── cli.tsx                    → main.rs (CLI entry)
│
├── screens/
│   └── REPL.tsx                   → ui/repl.rs
│
├── services/
│   ├── claude.ts                  → services/anthropic_service.rs
│   ├── openai.ts                  → services/openai_service.rs
│   └── mcpClient.ts               → services/mcp_client.rs
│
├── tools/
│   ├── [ToolName]/
│   │   ├── [ToolName].tsx         → tools/[tool_name].rs
│   │   └── prompt.ts              → tools/[tool_name]_prompts.rs
│   └── ... (20+ tools)            → tools/*.rs
│
├── utils/
│   ├── config.ts                  → config/mod.rs
│   ├── model.ts                   → model_manager.rs
│   ├── agentLoader.ts             → agent_loader.rs
│   ├── messageContextManager.ts   → context_manager.rs
│   ├── debugLogger.ts             → logging.rs
│   ├── markdown.ts                → markdown.rs
│   ├── tokens.ts                  → token_counter.rs
│   └── ... (40+ utilities)
│
├── types/
│   ├── conversation.ts            → types/message.rs
│   ├── modelCapabilities.ts       → types/capabilities.rs
│   └── ... (other types)
│
├── query.ts                        → query.rs (orchestration)
├── Tool.ts                         → tools/tool_trait.rs
├── tools.ts                        → tools/registry.rs
├── permissions.ts                 → permissions.rs
└── messages.ts                     → message_utils.rs
```

---

## Data Flow Examples

### Example 1: User Asks Question (Prompt Mode)

```
User Input
    ↓
REPL.stdin → parseUserInput()
    ↓
UserMessage created
    ↓
query(messages, systemPrompt, context, tools)
    ↓
LLM Service: queryAnthropicNative()
    ↓
stream decode messages/tool_uses
    ↓
for each chunk:
    ├─ if text_delta: append to buffer
    ├─ if tool_use_start: create tool block
    └─ if tool_use_delta: append JSON
    ↓
AssistantMessage (complete)
    ↓
Check for tool_use blocks
    ├─ Yes → execute tools
    └─ No → return to user
    ↓
Render in REPL
```

### Example 2: File Editing

```
User Input: "fix typo in src/main.ts"
    ↓
query() → LLM decides to use FileEditTool
    ↓
LLM generates tool use:
{
  "name": "FileEditTool",
  "input": {
    "file_path": "src/main.ts",
    "old_string": "teh quick",
    "new_string": "the quick"
  }
}
    ↓
checkPermissionsAndCallTool()
    ├─ Validate input
    ├─ Check permissions (safe mode)
    ├─ Read file first (required)
    └─ Apply edit
    ↓
FileEditTool.call()
    ├─ Parse file content
    ├─ Find and replace string
    ├─ Generate diff
    └─ Write file (atomic)
    ↓
ToolResult: diff + success message
    ↓
send to LLM for confirmation
    ↓
Render diff in REPL
```

### Example 3: Agent Delegation (TaskTool)

```
User: "Review this code for bugs"
    ↓
LLM decides: TaskTool
{
  "description": "Code review for bugs",
  "prompt": "Review src/utils/api.ts for bugs",
  "subagent_type": "code-reviewer"
}
    ↓
TaskTool.call()
    ├─ Load agent config: code-reviewer
    ├─ Filter tools based on agent.tools
    ├─ Create isolated query context
    └─ Call query() with filtered tools
    ↓
Sub-query executes
    ├─ Agent can only use: FileRead, Grep, Glob
    ├─ Agent reads files
    ├─ Agent searches for patterns
    └─ Agent returns findings
    ↓
Sub-query result → LLM response
    ↓
ProgressMessage yields
    ├─ "Analyzing code..."
    ├─ "Checking for common patterns..."
    └─ "Review complete"
    ↓
Final result to user
```

---

## Tool Interface Trait (Rust)

```rust
pub trait Tool: Send + Sync {
    // Identity
    fn name(&self) -> &str;
    async fn user_facing_name(&self) -> Option<String> { None }

    // Documentation
    async fn description(&self) -> String;
    async fn prompt(&self, safe_mode: bool) -> String;

    // Schema
    fn input_schema(&self) -> JsonSchema;

    // Lifecycle
    async fn is_enabled(&self) -> bool { Ok(true) }
    fn is_read_only(&self) -> bool;
    fn is_concurrency_safe(&self) -> bool { true }
    fn needs_permissions(&self, input: &JsonValue) -> bool;
    async fn validate_input(
        &self,
        input: &JsonValue,
        context: &ToolContext,
    ) -> Result<(), String> { Ok(()) }

    // Execution
    async fn call(
        &self,
        input: JsonValue,
        context: ToolContext,
    ) -> ToolStream;

    // Rendering
    fn render_for_assistant(&self, output: &ToolResult) -> String;
    fn render_message(&self, input: &JsonValue, verbose: bool) -> String;
    fn render_result(&self, output: &ToolResult) -> Option<RenderOutput> { None }
}

pub type ToolStream = Box<dyn Stream<Item = ToolOutput> + Send>;

pub enum ToolOutput {
    Progress(ProgressUpdate),
    Result(ToolResult),
}
```

---

## Message Type Hierarchy (Rust)

```rust
#[derive(Serialize, Deserialize)]
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
    Progress(ProgressMessage),
}

pub struct UserMessage {
    pub role: Role,
    pub content: Vec<ContentBlock>,
    pub uuid: Uuid,
}

pub struct AssistantMessage {
    pub role: Role,
    pub content: Vec<ContentBlock>,
    pub usage: Usage,
    pub cost_usd: f64,
    pub uuid: Uuid,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "image")]
    Image {
        source: ImageSource,
    },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: JsonValue,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}
```

---

## Configuration Schema (Rust)

```rust
#[derive(Serialize, Deserialize)]
pub struct GlobalConfig {
    pub api_keys: Option<HashMap<String, String>>,
    pub model_profiles: Vec<ModelProfile>,
    pub model_pointers: ModelPointers,
    pub default_model_name: String,
    pub primary_provider: ProviderType,
    pub stream: bool,
    pub mcp_servers: Option<HashMap<String, MCPServerConfig>>,
    pub user_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct ModelProfile {
    pub name: String,
    pub provider: ProviderType,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub max_tokens: u32,
    pub context_length: u32,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub is_active: bool,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ModelPointers {
    pub main: String,
    pub task: String,
    pub reasoning: String,
    pub quick: String,
}
```

---

## Permission Model (Rust)

```rust
pub struct PermissionRequest {
    pub tool: String,
    pub input: JsonValue,
    pub description: String,
    pub is_dangerous: bool,
}

pub enum PermissionResult {
    Approved,
    Rejected,
    AlwaysApprove,
}

pub struct PermissionSystem {
    session_cache: HashMap<String, bool>,
    permanent_cache: HashMap<String, bool>,
}

impl PermissionSystem {
    pub async fn check_permission(
        &self,
        tool: &str,
        input: &JsonValue,
        safe_mode: bool,
    ) -> Result<bool, String> {
        // Implementation
    }
}
```

---

## Phase 1: MVP Checklist

### Week 1-2: Foundation
- [ ] Rust project setup with cargo
- [ ] CLI argument parsing (clap)
- [ ] Configuration loading (serde + JSON)
- [ ] Message type definitions
- [ ] Basic async runtime setup
- [ ] REPL UI skeleton (ratatui)

### Week 2-3: Core Systems
- [ ] Query system core
- [ ] Tool trait definition
- [ ] Tool registry
- [ ] Basic REPL UI interaction

### Week 2-3 (Parallel): LLM Integration
- [ ] Anthropic service client
- [ ] Streaming response handling
- [ ] Retry logic
- [ ] Token counting

### Week 4: File Tools
- [ ] FileReadTool
- [ ] FileEditTool
- [ ] GlobTool
- [ ] File permissions validation

### Week 4-5: Execution
- [ ] BashTool basics
- [ ] Subprocess management
- [ ] Command permission checking

### Week 5: Polish
- [ ] Error handling throughout
- [ ] Better UI feedback
- [ ] Performance optimization
- [ ] Integration testing

---

## Key Files by Complexity

### Simplest (Start Here)
1. Message types (messages.rs)
2. Configuration (config.rs)
3. Cost tracker (cost_tracker.rs)
4. Debug logger (logging.rs)

### Moderate (Phase 1)
5. Tool interface (tools/tool_trait.rs)
6. Tool registry (tools/registry.rs)
7. File tools (tools/*.rs)
8. Bash tool (tools/bash.rs)

### Complex (Critical)
9. Query system (query.rs)
10. LLM services (services/*.rs)
11. REPL / UI (ui/repl.rs)
12. Agent system (agent_loader.rs)

### Expert-Only (Phase 3+)
13. MCP client (services/mcp_client.rs)
14. Context manager (context_manager.rs)
15. Prompt caching logic

---

## Testing Strategy

### Unit Tests
- Message serialization/deserialization
- Configuration validation
- Tool input validation
- Permission checks
- Cost calculations

### Integration Tests
- Query flow with tools
- File read/write operations
- Command execution
- LLM API communication (with VCR cassettes)

### E2E Tests
- Full workflow: question → tool use → response
- Multiple sequential queries
- Agent delegation
- Error recovery

### VCR Cassettes
- Record actual API responses
- Replay for testing without API calls
- Located in `.vcr/` directory

---

## Performance Targets

| Metric | Target | Current (Node.js) |
|--------|--------|-------------------|
| Startup time | <100ms | ~200ms |
| First token time | <500ms | ~400ms |
| Memory idle | <50MB | ~80MB |
| Memory with context | <200MB | ~250MB |
| Tool execution overhead | <50ms | ~30ms |

