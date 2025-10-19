# Kode TypeScript to Rust Porting Plan

## Overview
This document outlines the comprehensive plan for porting Kode from TypeScript/Bun to Rust, maintaining feature parity while leveraging Rust's performance and safety guarantees.

## Project Structure Mapping

### TypeScript Source Structure
```
src/
├── commands/           # CLI commands
├── components/         # Ink/React UI components
├── constants/          # Configuration constants
├── context/            # Project context management
├── entrypoints/        # CLI entry points
├── hooks/              # React hooks
├── screens/            # Main UI screens (REPL)
├── services/           # External service integrations
├── tools/              # Tool implementations (20 tools)
├── types/              # Type definitions
├── utils/              # Utility functions
├── Tool.ts             # Core Tool interface
├── tools.ts            # Tool registry
├── permissions.ts      # Permission system
├── query.ts            # AI query orchestration
└── context.ts          # Codebase context
```

### Rust Target Structure
```
src/
├── main.rs             # CLI entry point
├── lib.rs              # Library exports
├── cli/                # CLI argument parsing & routing (clap)
├── tui/                # Terminal UI (ratatui + crossterm)
│   ├── app.rs          # Main TUI app state
│   ├── repl.rs         # REPL screen
│   └── components/     # UI components
├── tools/              # Tool trait and implementations
│   ├── mod.rs          # Tool trait definition
│   ├── registry.rs     # Tool registry
│   ├── bash.rs         # BashTool
│   ├── file_read.rs    # FileReadTool
│   ├── file_write.rs   # FileWriteTool
│   ├── file_edit.rs    # FileEditTool
│   ├── glob.rs         # GlobTool
│   ├── grep.rs         # GrepTool
│   ├── ls.rs           # lsTool
│   ├── memory.rs       # Memory tools
│   ├── notebook.rs     # Notebook tools
│   ├── task.rs         # TaskTool (agent orchestration)
│   ├── think.rs        # ThinkTool
│   ├── todo.rs         # TodoWriteTool
│   ├── url_fetcher.rs  # URLFetcherTool
│   ├── web_search.rs   # WebSearchTool
│   ├── mcp.rs          # MCPTool
│   └── architect.rs    # ArchitectTool
├── services/           # External integrations
│   ├── mod.rs
│   ├── anthropic.rs    # Claude API client
│   ├── openai.rs       # OpenAI API client
│   ├── mcp_client.rs   # MCP server client
│   └── adapters/       # Model adapter implementations
├── config/             # Configuration management
│   ├── mod.rs
│   ├── models.rs       # Model configuration
│   └── settings.rs     # User settings
├── agents/             # Agent system
│   ├── mod.rs
│   ├── loader.rs       # Dynamic agent loading
│   └── registry.rs     # Agent registry
├── context/            # Project context
│   ├── mod.rs
│   ├── codebase.rs     # Codebase analysis
│   └── memory.rs       # Memory management
├── permissions/        # Permission system
│   ├── mod.rs
│   └── handlers.rs     # Permission handlers
├── query/              # AI query orchestration
│   ├── mod.rs
│   ├── executor.rs     # Query execution
│   └── streaming.rs    # Streaming responses
└── utils/              # Utilities
    ├── mod.rs
    ├── diff.rs         # Diff utilities
    ├── syntax.rs       # Syntax highlighting
    └── fs.rs           # File system utilities
```

## Core Abstractions & Type Mappings

### 1. Tool Trait
**TypeScript**: `Tool<TInput, TOutput>` interface
**Rust**: `Tool` trait with associated types

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    type Input: DeserializeOwned + Send;
    type Output: Serialize + Send;

    fn name(&self) -> &str;
    async fn description(&self) -> String;
    fn input_schema(&self) -> Value; // JSON Schema
    async fn prompt(&self, safe_mode: bool) -> String;
    fn user_facing_name(&self) -> String;
    async fn is_enabled(&self) -> bool;
    fn is_read_only(&self) -> bool;
    fn is_concurrency_safe(&self) -> bool;
    fn needs_permissions(&self, input: &Self::Input) -> bool;
    async fn validate_input(&self, input: &Self::Input, ctx: &ToolContext) -> ValidationResult;
    fn render_result(&self, output: &Self::Output) -> String;
    fn render_tool_use(&self, input: &Self::Input, verbose: bool) -> String;

    async fn call(
        &self,
        input: Self::Input,
        ctx: ToolContext,
    ) -> Result<ToolStream<Self::Output>>;
}

// Tool streaming result
pub enum ToolStreamItem<T> {
    Progress { content: String },
    Result { data: T, result_for_assistant: Option<String> },
}

pub type ToolStream<T> = Pin<Box<dyn Stream<Item = ToolStreamItem<T>> + Send>>;
```

### 2. Configuration System
**TypeScript**: JSON-based config with env vars
**Rust**: Layered config using `serde` + `toml`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub models: HashMap<String, ModelConfig>,
    pub default_model: String,
    pub task_model: String,
    pub reasoning_model: String,
    pub quick_model: String,
    pub agents_dir: Vec<PathBuf>,
    pub memory_dir: PathBuf,
    pub safe_mode: bool,
    pub verbose: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub provider: ModelProvider,
    pub model_name: String,
    pub api_key: Option<String>,
    pub api_base: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelProvider {
    Anthropic,
    OpenAI,
    Bedrock,
    Vertex,
}
```

### 3. Model Adapter
**TypeScript**: Service classes with adapters
**Rust**: Trait-based adapter pattern

```rust
#[async_trait]
pub trait ModelAdapter: Send + Sync {
    async fn complete(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSchema>,
        options: CompletionOptions,
    ) -> Result<CompletionStream>;

    async fn stream_complete(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSchema>,
        options: CompletionOptions,
    ) -> Result<CompletionStream>;
}

pub type CompletionStream = Pin<Box<dyn Stream<Item = Result<CompletionChunk>> + Send>>;
```

### 4. Agent System
**TypeScript**: Markdown files with YAML frontmatter
**Rust**: Same format, using `gray_matter` equivalent

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub description: String,
    pub tools: ToolFilter,
    pub model: Option<String>,
    pub prompt: String,
}

pub enum ToolFilter {
    All,
    Specific(Vec<String>),
}
```

## Library Dependencies Mapping

### Core Dependencies
- **CLI**: `clap` (4.x) with derive macros
- **TUI**: `ratatui` (0.29) + `crossterm` (0.28)
- **Async**: `tokio` (1.x) with full features
- **HTTP**: `reqwest` (0.12) for API clients
- **Serialization**: `serde`, `serde_json`, `toml`, `toml_edit`
- **Errors**: `anyhow`, `thiserror`, `color-eyre`

### Tool-Specific Dependencies
- **Glob**: `wildmatch` (glob patterns)
- **Grep**: `regex-lite` (regex matching)
- **Diff**: `similar` or `diffy` (text diffing)
- **Syntax**: `tree-sitter` + language parsers + `tree-sitter-highlight`
- **Fuzzy**: `nucleo-matcher` (fuzzy matching)
- **HTML**: HTML parsing for URL fetcher
- **Markdown**: Markdown parsing for agents

### Testing Dependencies
- `tokio-test`: Async testing utilities
- `insta`: Snapshot testing
- `assert_cmd`: CLI testing
- `wiremock`: HTTP mocking
- `vt100`: Terminal emulation
- `predicates`: Assertion predicates
- `pretty_assertions`: Better assertion output

## Porting Phases

### Phase 1: Foundation (Priority 1)
1. **Project Setup**
   - Initialize Cargo.toml with all dependencies
   - Set up workspace structure
   - Configure CI/CD (GitHub Actions)
   - Set up pre-commit hooks

2. **Core Types & Traits**
   - Port Tool trait
   - Port Config types
   - Port Message types
   - Port Permission types
   - Port Error types

3. **Configuration System**
   - Layered config loading (global, project, env)
   - Model configuration
   - Agent directory configuration
   - Settings persistence

### Phase 2: Services Layer (Priority 1)
1. **Model Adapters**
   - Anthropic SDK integration
   - OpenAI SDK integration
   - Streaming support
   - Token counting

2. **MCP Client**
   - MCP protocol implementation
   - Server discovery
   - Tool schema parsing

### Phase 3: Core Tools (Priority 1)
Port tools in order of dependency:

1. **FileReadTool** - Most fundamental
2. **FileWriteTool** - File creation
3. **FileEditTool** - File modification with diff
4. **BashTool** - Command execution
5. **GlobTool** - File pattern matching
6. **GrepTool** - Content search
7. **lsTool** - Directory listing

### Phase 4: Advanced Tools (Priority 2)
8. **MemoryReadTool** - Session memory
9. **MemoryWriteTool** - Memory persistence
10. **ThinkTool** - Internal reasoning
11. **TodoWriteTool** - Task tracking
12. **MultiEditTool** - Batch edits

### Phase 5: Integration Tools (Priority 2)
13. **URLFetcherTool** - Web content fetching
14. **WebSearchTool** - Web search integration
15. **NotebookReadTool** - Jupyter notebooks
16. **NotebookEditTool** - Notebook editing
17. **MCPTool** - MCP server integration

### Phase 6: Meta Tools (Priority 2)
18. **TaskTool** - Agent orchestration
19. **ArchitectTool** - Architecture planning
20. **AskExpertModelTool** - Multi-model queries

### Phase 7: TUI Layer (Priority 1)
1. **Basic REPL**
   - Input handling
   - Message display
   - Syntax highlighting
   - Scrolling

2. **Permission System**
   - Permission requests
   - User approval UI
   - Permission caching

3. **Advanced UI**
   - Tool use visualization
   - Progress indicators
   - Error display
   - Multi-line input

### Phase 8: Agent System (Priority 2)
1. **Agent Loader**
   - Directory scanning
   - YAML parsing
   - Agent caching
   - Hot reload

2. **Agent Registry**
   - Agent lookup
   - Tool filtering
   - Model selection

### Phase 9: Context & Memory (Priority 2)
1. **Project Context**
   - Codebase analysis
   - File relationships
   - Git integration

2. **Memory System**
   - Persistent memory
   - Session state
   - Context window management

### Phase 10: Testing (Continuous)
1. **Unit Tests**
   - Tool implementations
   - Config parsing
   - Agent loading
   - Permission system

2. **Integration Tests**
   - End-to-end CLI tests
   - API integration tests
   - MCP integration tests

3. **Snapshot Tests**
   - Tool output verification
   - UI rendering tests
   - Error message tests

### Phase 11: Polish & Documentation (Priority 3)
1. **Performance Optimization**
   - Profile hot paths
   - Optimize allocations
   - Parallel processing where possible

2. **Documentation**
   - API documentation
   - User guide
   - Architecture documentation
   - Migration guide

3. **Packaging**
   - Binary releases (GitHub Actions)
   - Homebrew formula
   - Cargo publish

## Key Technical Challenges

### 1. Async Generators → Streams
**TypeScript**: AsyncGenerator with yield
**Rust**: `Stream` trait from `futures` crate

```rust
// TypeScript
async function* call(input, ctx) {
    yield { type: 'progress', content: 'Processing...' };
    yield { type: 'result', data: result };
}

// Rust
fn call(&self, input: Input, ctx: Context) -> ToolStream<Output> {
    Box::pin(async_stream::stream! {
        yield ToolStreamItem::Progress { content: "Processing...".into() };
        yield ToolStreamItem::Result { data: result, result_for_assistant: None };
    })
}
```

### 2. React UI → Ratatui
**TypeScript**: Ink (React for terminal)
**Rust**: Ratatui (immediate mode)

Major differences:
- React: Declarative, component-based
- Ratatui: Imperative, frame-based rendering

Solution: Build stateful components that render to ratatui widgets

### 3. Dynamic Tool Loading
**TypeScript**: Dynamic imports, easy reflection
**Rust**: Static compilation

Solution: Macro-based tool registration:
```rust
register_tools! {
    FileReadTool,
    FileWriteTool,
    BashTool,
    // ... etc
}
```

### 4. JSON Schema Generation
**TypeScript**: `zod-to-json-schema`
**Rust**: `schemars` or manual schema generation

### 5. Permission System
Maintain interactive permission approval during tool execution:
- Pause tool execution
- Show permission request in UI
- Wait for user approval
- Resume or cancel execution

## Testing Strategy

### Unit Tests (80% of testing time)
- Each tool has unit tests for core logic
- Config parsing and validation
- Agent loading and parsing
- Permission system

### Integration Tests (15% of testing time)
- CLI command tests using `assert_cmd`
- End-to-end tool execution
- API integration with mocks (`wiremock`)

### Snapshot Tests (5% of testing time)
- Tool output format verification
- Error message consistency
- UI rendering verification (using `vt100`)

## Git Workflow

**IMPORTANT**: Commit and push after every single file edit.

Format:
```
git add <file>
git commit -m "port: <brief description of what was ported>"
git push
```

Example:
```
git add src/tools/mod.rs
git commit -m "port: add Tool trait definition and core types"
git push

git add src/tools/file_read.rs
git commit -m "port: implement FileReadTool with line range support"
git push
```

## Progress Tracking

Track progress in `agent/TODO.md` with:
- [x] Completed items
- [ ] Pending items
- Current blockers
- Next steps

## Success Criteria

1. **Feature Parity**: All 20 tools implemented
2. **TUI Functional**: Interactive REPL works
3. **Multi-Model**: Support for Anthropic and OpenAI
4. **Agent System**: Dynamic agent loading works
5. **Tests Pass**: >80% code coverage
6. **Performance**: Faster startup than TypeScript version
7. **Documentation**: README and architecture docs complete

## Timeline Estimate

- Phase 1-2: Foundation & Services (3-4 days)
- Phase 3-4: Core & Advanced Tools (5-7 days)
- Phase 5-6: Integration & Meta Tools (4-5 days)
- Phase 7: TUI Layer (3-4 days)
- Phase 8-9: Agent System & Context (2-3 days)
- Phase 10: Testing (continuous, 20% of time)
- Phase 11: Polish (2-3 days)

**Total**: ~20-30 days of focused work

## Next Steps

1. Read and understand key TypeScript files
2. Set up Rust project with dependencies
3. Port core types (Tool trait, Config, etc.)
4. Implement first tool (FileReadTool)
5. Set up basic TUI scaffold
6. Iterate on remaining tools
