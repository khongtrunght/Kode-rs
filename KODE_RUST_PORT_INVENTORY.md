# Kode to Rust: Comprehensive Component Inventory

## Project Overview

**Kode** is an AI-powered terminal assistant written in TypeScript/React with ~266 source files. This document provides a structured inventory of all components for porting to Rust, organized by priority and complexity.

---

## SECTION 1: CRITICAL CORE SYSTEMS (Phase 1)

These components are essential for basic functionality and should be implemented first.

### 1.1 REPL (Read-Eval-Print Loop) / Terminal UI
**Current Location**: `src/screens/REPL.tsx`
**Complexity**: HIGH
**Estimated Lines of Rust**: 2,000-3,000
**Dependencies**: Terminal I/O, async message handling, React-like UI rendering

**Key Responsibilities**:
- Interactive terminal interface (built on Ink/React in JS)
- User input handling (3 modes: prompt, bash, koding)
- Real-time message rendering (static vs transient)
- Streaming response handling
- Permission dialog display

**Rust Considerations**:
- Replace Ink/React with TUI crate (ratatui, crossterm)
- Implement async event loop with tokio
- State management without React hooks
- Streaming message rendering

**Components to Implement**:
- `PromptInput` - text input handling
- `MessageRenderer` - message display logic
- `PermissionDialog` - user confirmation UI
- `ProgressIndicator` - loading/progress display
- `ThemeEngine` - terminal color/style management

---

### 1.2 Query System (Orchestration Layer)
**Current Location**: `src/query.ts`
**Complexity**: HIGH
**Estimated Lines of Rust**: 1,500-2,500
**Dependencies**: LLM service integration, tool execution, message handling

**Key Responsibilities**:
- Message orchestration and flow control
- Tool use detection and execution
- Streaming response handling
- Context management and compaction
- Progress message generation

**Rust Considerations**:
- Async generator equivalent (channels or async streams)
- Complex state machine for tool use workflows
- Message normalization and formatting
- Error handling with proper recovery

**Key Algorithms**:
- Auto-compaction for context windows
- Concurrent vs serial tool execution logic
- Binary feedback response handling
- Message streaming reconstruction

---

### 1.3 Tool System Core
**Current Location**: `src/Tool.ts` + `src/tools.ts`
**Complexity**: MEDIUM-HIGH
**Estimated Lines of Rust**: 1,000-1,500
**Dependencies**: Zod schemas (replace with serde), async execution

**Key Interfaces**:
```rust
pub trait Tool {
    fn name(&self) -> &str;
    async fn description(&self) -> String;
    async fn prompt(&self, safe_mode: bool) -> String;
    fn input_schema(&self) -> JsonSchema;
    async fn is_enabled(&self) -> bool;
    fn is_read_only(&self) -> bool;
    fn needs_permissions(&self, input: &serde_json::Value) -> bool;
    async fn call(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> impl Stream<Item = ToolResult>;
    fn render_for_assistant(&self, output: &ToolResult) -> String;
}
```

**Components to Port**:
1. Tool registry and discovery
2. Tool execution context
3. Tool result handling
4. Input validation framework

---

### 1.4 Message Types and Normalization
**Current Location**: `src/messages.ts`, `src/types/conversation.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 800-1,200
**Dependencies**: serde for JSON serialization

**Key Types**:
- `UserMessage` - user input with multimodal support
- `AssistantMessage` - model response with tool calls
- `ProgressMessage` - streaming updates
- `ToolResultMessage` - tool execution results
- Message content blocks (text, images, tool uses)

**Rust Approach**:
- Use `serde` for serialization/deserialization
- `enum` for content blocks (tagged unions)
- Validation logic for message structure
- Helper functions for message creation/manipulation

---

## SECTION 2: ESSENTIAL SERVICES (Phase 1-2)

Critical backend services required for core operation.

### 2.1 LLM Service Integration
**Current Location**: `src/services/claude.ts`, `src/services/openai.ts`
**Complexity**: HIGH
**Estimated Lines of Rust**: 2,500-3,500
**Dependencies**: HTTP client, streaming, JSON handling

**Providers to Support**:
1. Anthropic (native SDK equivalent)
2. OpenAI (ChatGPT, GPT-5 Responses API)
3. OpenAI-compatible (Mistral, DeepSeek, Groq, etc.)

**Key Features**:
- Streaming message handling
- Retry logic with exponential backoff
- Request timeout management
- Prompt caching (Anthropic-specific)
- Cost tracking
- Error recovery

**Rust Libraries**:
- `reqwest` - HTTP client
- `tokio` - async runtime
- `serde_json` - JSON parsing
- `async-stream` - for streaming responses

---

### 2.2 Model Manager
**Current Location**: `src/utils/model.ts`, `src/utils/config.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 1,200-1,800
**Dependencies**: Configuration system, model resolution logic

**Key Responsibilities**:
- Model profile management
- Model pointer system (main/task/reasoning/quick)
- Model switching logic
- Context compatibility analysis
- Model validation and repair

**Rust Approach**:
- Use `serde` for config serialization
- Singleton pattern for ModelManager
- Builder pattern for model configuration
- Validation using custom types

---

### 2.3 Configuration System
**Current Location**: `src/utils/config.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 1,000-1,500
**Dependencies**: File I/O, JSON parsing, environment variables

**Configuration Levels** (in priority order):
1. Environment variables
2. Global config (`~/.kode.json`)
3. Project config (`./.kode.json`)
4. CLI parameters

**Features**:
- Hierarchical configuration merging
- Validation and defaults
- Hot reloading (file watchers)
- MCP server configuration

---

### 2.4 Permission System
**Current Location**: `src/permissions.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 1,000-1,500
**Dependencies**: Tool system, configuration

**Key Features**:
- Safe mode enforcement
- Command injection prevention
- File path validation
- Permission caching
- Permission UI integration

**Security Considerations**:
- Sandbox file access within project directory
- Command chaining detection
- Symlink escape prevention
- Sensitive file protection

---

### 2.5 Agent System
**Current Location**: `src/utils/agentLoader.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 1,200-1,800
**Dependencies**: YAML/frontmatter parsing, file watching, caching

**Key Features**:
- 5-tier agent loading priority
- Dynamic agent configuration
- Agent caching and hot reload
- Agent validation
- Tool filtering

**Agent Configuration** (Markdown + YAML frontmatter):
```yaml
name: agent-name
description: "When to use this agent"
tools: ["Tool1", "Tool2"] | "*"
model_name: optional-model-override
color: optional-ui-color
```

---

## SECTION 3: FILE I/O TOOLS (Phase 2)

Essential file manipulation capabilities.

### 3.1 FileReadTool
**Current Location**: `src/tools/FileReadTool/FileReadTool.tsx`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 800-1,200

**Features**:
- Line-based pagination (offset/limit)
- Image support (PNG, JPG, etc.)
- PDF parsing (page-by-page)
- Jupyter notebook parsing
- File change detection (diffs on re-read)
- Line truncation (2000 chars)

**Rust Libraries**:
- `std::fs` - file reading
- `image` crate - image parsing
- `pdf` crate - PDF extraction
- `serde_json` - notebook parsing

---

### 3.2 FileEditTool
**Current Location**: `src/tools/FileEditTool/FileEditTool.tsx`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 800-1,200

**Critical Features**:
- String replacement (exact match)
- Indentation preservation
- Unified diff output
- Replace-all mode (for renaming)
- Line number format handling

**Rust Approach**:
- Careful string manipulation with offset tracking
- Diff generation using `similar` crate
- Validation before file write

---

### 3.3 FileWriteTool
**Current Location**: `src/tools/FileWriteTool/FileWriteTool.tsx`
**Complexity**: LOW-MEDIUM
**Estimated Lines of Rust**: 500-800

**Features**:
- Create/overwrite files
- Parent directory creation
- Diff display for overwrites
- Permission enforcement

---

### 3.4 GlobTool (File Pattern Matching)
**Current Location**: `src/tools/GlobTool/GlobTool.tsx`
**Complexity**: LOW-MEDIUM
**Estimated Lines of Rust**: 400-600

**Features**:
- Glob pattern matching
- Results sorted by modification time
- Recursive directory traversal

**Rust Libraries**:
- `globwalk` crate - glob pattern matching

---

### 3.5 GrepTool (Code Search)
**Current Location**: `src/tools/GrepTool/GrepTool.tsx`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 600-1,000

**Features**:
- Regex pattern matching (ripgrep equivalent)
- File type filtering (language-aware)
- Context lines (-B, -C, -A)
- Output modes (content/files/count)
- Multiline pattern support

**Rust Libraries**:
- `regex` crate - regex matching
- `ignore` crate - ripgrep-compatible file traversal

---

### 3.6 LSTool (Directory Listing)
**Current Location**: `src/tools/lsTool/lsTool.tsx`
**Complexity**: LOW-MEDIUM
**Estimated Lines of Rust**: 400-700

**Features**:
- Directory listing with metadata
- Recursive depth control
- File type detection
- Human-readable sizes
- Timestamps

---

### 3.7 MultiEditTool
**Current Location**: `src/tools/MultiEditTool/MultiEditTool.tsx`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 600-900

**Features**:
- Batch file edits
- Transaction-like semantics (all-or-nothing)
- Multiple files in single operation

---

## SECTION 4: COMMAND EXECUTION TOOLS (Phase 2)

### 4.1 BashTool
**Current Location**: `src/tools/BashTool/BashTool.tsx`
**Complexity**: MEDIUM-HIGH
**Estimated Lines of Rust**: 1,200-1,800

**Key Features**:
- Shell command execution
- Permission system (safe commands, prefixes)
- Timeout handling
- Background execution support
- Output streaming
- Command chaining validation
- Command injection prevention

**Rust Libraries**:
- `tokio::process` - subprocess management
- `shell-quote` - command parsing

**Safe Commands**:
- `git` (status, diff, log, branch, etc.)
- `pwd`, `tree`, `date`, `which`
- Directory/file inspection commands

---

### 4.2 PersistentShell
**Current Location**: `src/utils/PersistentShell.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 800-1,200

**Features**:
- Long-lived shell session
- Working directory tracking
- Command history
- Background process management
- Shell type detection (bash, zsh, fish)

---

## SECTION 5: ADVANCED TOOLS (Phase 3)

### 5.1 TaskTool (Agent Delegation)
**Current Location**: `src/tools/TaskTool/TaskTool.tsx`
**Complexity**: HIGH
**Estimated Lines of Rust**: 1,200-1,800

**Features**:
- Sub-agent launching
- Tool filtering per agent
- Model override per agent
- Message passing
- Progress tracking
- Recursive task handling

---

### 5.2 WebSearchTool
**Current Location**: `src/tools/WebSearchTool/WebSearchTool.tsx`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 600-1,000

**Features**:
- Web search (US only)
- Domain filtering
- Result formatting

**Rust Libraries**:
- `reqwest` - HTTP requests

---

### 5.3 URLFetcherTool
**Current Location**: `src/tools/URLFetcherTool/URLFetcherTool.tsx`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 800-1,200

**Features**:
- URL fetching
- HTML to Markdown conversion
- 15-minute caching
- Redirect handling
- robots.txt compliance

**Rust Libraries**:
- `reqwest` - HTTP
- `html2md` - HTML to Markdown
- `moka` - caching

---

### 5.4 NotebookTools (Jupyter)
**Current Location**: `src/tools/NotebookReadTool/`, `src/tools/NotebookEditTool/`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 1,000-1,500

**Features**:
- Notebook reading (cell extraction)
- Notebook editing (cell manipulation)
- Cell type handling (code/markdown)
- Output preservation

---

### 5.5 MemoryTools (Anthropic-specific)
**Current Location**: `src/tools/MemoryReadTool/`, `src/tools/MemoryWriteTool/`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 600-900

**Features**:
- Persistent memory across sessions
- TTL support
- Key-value storage with Anthropic API

---

### 5.6 AskExpertModelTool
**Current Location**: `src/tools/AskExpertModelTool/`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 600-900

**Features**:
- Model switching for specific queries
- Pointer-based model selection
- Isolated query context

---

## SECTION 6: UTILITIES & HELPERS (Phase 2-3)

### 6.1 Context Management
**Current Location**: `src/utils/messageContextManager.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 1,000-1,500

**Features**:
- Auto-compaction for context windows
- Message retention strategy
- Context limit analysis
- Token counting

---

### 6.2 Cost Tracking
**Current Location**: `src/cost-tracker.ts`
**Complexity**: LOW
**Estimated Lines of Rust**: 300-500

**Features**:
- Cost calculation per provider
- Per-model cost tracking
- Total cost aggregation

---

### 6.3 Debug Logger
**Current Location**: `src/utils/debugLogger.ts`
**Complexity**: LOW
**Estimated Lines of Rust**: 400-600

**Features**:
- Structured logging
- Event categorization
- Request tracking
- Performance timing

---

### 6.4 Token Counting
**Current Location**: `src/utils/tokens.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 600-900

**Features**:
- Anthropic token counting
- OpenAI token counting (tiktoken)
- Context usage estimation

**Rust Libraries**:
- `js-tiktoken` wrapper or native implementation

---

### 6.5 Markdown Rendering
**Current Location**: `src/utils/markdown.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 600-900

**Features**:
- Markdown to terminal display
- Code highlighting
- Syntax highlighting
- Terminal-safe formatting

**Rust Libraries**:
- `markdown` crate
- `syntect` - syntax highlighting

---

### 6.6 Diff Generation
**Current Location**: `src/utils/diff.ts`
**Complexity**: LOW-MEDIUM
**Estimated Lines of Rust**: 400-600

**Features**:
- Unified diff output
- Terminal-friendly formatting

**Rust Libraries**:
- `similar` crate - diff algorithms

---

## SECTION 7: MISSING/LESS CRITICAL COMPONENTS

### 7.1 Commands System
**Current Location**: `src/commands.ts`
**Complexity**: LOW-MEDIUM
**Estimated Lines of Rust**: 800-1,200

**Built-in Commands**:
- `/agents` - list available agents
- `/cost` - show cost tracking
- `/model` - model management
- `/help` - help system
- `/exit` - graceful exit
- `/clear` - clear conversation

---

### 7.2 MCP (Model Context Protocol) Integration
**Current Location**: `src/services/mcpClient.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 1,000-1,500

**Features**:
- MCP server connection
- Tool discovery
- Tool invocation
- Error handling

---

### 7.3 Context/Documentation System
**Current Location**: `src/context.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 600-900

**Features**:
- Project documentation loading
- Context injection into prompts
- Documentation file discovery

---

## SECTION 8: CROSS-CUTTING CONCERNS

### 8.1 Error Handling
**Current Location**: Multiple files
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 1,000-1,500

**Key Error Types**:
- API errors
- File I/O errors
- Permission errors
- Validation errors
- Timeout errors
- Command execution errors

**Rust Approach**:
- Custom error types using `thiserror`
- Result<T, E> pattern throughout
- Error context preservation

---

### 8.2 Async/Await Pattern Conversion
**Current Consideration**: JavaScript async/await → Rust async/await
**Complexity**: HIGH (pervasive throughout)

**Key Conversions**:
- Promise → Future
- async function → async fn
- .then() chains → .await
- Error propagation → ?

---

### 8.3 File System Operations
**Centralized Utilities**: `src/utils/file.ts`
**Complexity**: MEDIUM
**Estimated Lines of Rust**: 600-1,000

**Features**:
- Safe file reading
- Atomic writes
- Path normalization
- Directory traversal

**Rust Libraries**:
- `tokio::fs` - async file I/O
- `camino` - cross-platform paths

---

### 8.4 Environment & Auth
**Current Location**: `src/utils/auth.ts`, `src/utils/env.ts`
**Complexity**: LOW-MEDIUM
**Estimated Lines of Rust**: 600-900

**Features**:
- API key management
- Environment variable loading
- Credential storage
- User authentication flow

---

## SECTION 9: REACT/INK REPLACEMENT STRATEGY

### Challenge: Terminal UI Framework
**Current Stack**: Ink (React for CLIs)
**Rust Alternatives**:

1. **ratatui** (Recommended)
   - Popularity and maturity
   - Good component ecosystem
   - Active maintenance
   - Similar declarative API

2. **crossterm**
   - Lower-level terminal control
   - Good for custom implementations
   - Combine with UI framework

3. **tui-rs** (legacy, use ratatui instead)

### Components to Reimplement

#### PromptInput
- Text input with real-time rendering
- History support
- Autocomplete

#### MessageRenderer
- Static vs transient message distinction
- Content block rendering (text, images, tool uses)
- Syntax highlighting

#### ProgressIndicator
- Spinner animation
- Progress percentage
- Status messages

#### PermissionDialog
- Modal-like dialog
- Keyboard navigation
- Yes/No/Always options

---

## SECTION 10: DEPENDENCY MAPPING

### Current TypeScript Dependencies to Replace

| TypeScript Package | Rust Equivalent | Notes |
|---|---|---|
| @anthropic-ai/sdk | reqwest + custom | Need to implement API client |
| openai | reqwest + custom | OpenAI/GPT-5 API client |
| ink | ratatui | Terminal UI framework |
| react | ratatui components | UI logic |
| zod | serde + custom validation | Schema validation |
| glob | globwalk | File globbing |
| markdown | markdown crate | Markdown parsing |
| commander | clap | CLI argument parsing |
| dotenv | dotenv crate | Environment loading |
| diff | similar | Diff algorithms |
| lodash-es | standard library + custom | Utilities |
| gray-matter | yaml crate | YAML/frontmatter parsing |

---

## PORTING ROADMAP

### Phase 1: Core Infrastructure (Weeks 1-3)
Priority: CRITICAL
- Message types and normalization
- Configuration system
- REPL / Terminal UI (with ratatui)
- Query system (orchestration)
- Tool system interfaces

**Deliverable**: Basic REPL that accepts input (not yet connected to LLM)

### Phase 1B: LLM Integration (Weeks 2-3, parallel)
- LLM service client (Anthropic)
- Model manager
- Basic API communication
- Streaming response handling

**Deliverable**: REPL can query Claude API

### Phase 2: Core Tools (Weeks 4-6)
Priority: HIGH
- File I/O tools (Read/Edit/Write/Glob)
- Bash tool
- Permission system
- Agent system

**Deliverable**: File editing and command execution working

### Phase 2B: Search & Web (Weeks 5-6, parallel)
- GrepTool
- WebSearch / URLFetcher
- Notebook tools

### Phase 3: Advanced Features (Weeks 7-10)
Priority: MEDIUM
- TaskTool (agent delegation)
- MCP integration
- Memory tools
- Cost tracking
- Extended thinking support

### Phase 4: Polish & Testing (Weeks 11-12)
Priority: MEDIUM
- Error handling improvements
- Performance optimization
- Test coverage
- Documentation

---

## ESTIMATED TOTAL EFFORT

**Total Rust Code**: ~20,000-30,000 lines
**Estimated Time**: 12 weeks (with team of 2-3 developers)
**Critical Path**: Phases 1 + 1B + 2 (6 weeks minimum for MVP)

---

## KEY ARCHITECTURAL DECISIONS FOR RUST

### 1. Async Runtime
**Decision**: Use `tokio`
- Industry standard
- Good performance
- Rich ecosystem
- Works well with terminal UIs

### 2. Terminal UI Framework
**Decision**: Use `ratatui`
- Closest equivalent to Ink
- Good abstraction layer
- Active community
- Good documentation

### 3. Error Handling
**Decision**: Use `thiserror` for custom errors
- Clear error messages
- Good error context
- Integrates with ? operator

### 4. Configuration
**Decision**: Use `serde` + `serde_json` + `toml`
- Standard approach
- Type-safe deserialization
- Validation support

### 5. HTTP Client
**Decision**: Use `reqwest` with `tokio`
- Async by default
- Streaming support
- Good for API clients

### 6. API Client Design
**Decision**: Custom client, not third-party SDKs
- SDKs often lag API updates
- Full control over retry logic
- Easier to support multiple providers

---

## RISK MITIGATION

### 1. Terminal UI Complexity
**Risk**: Replicating Ink's abstractions in Rust
**Mitigation**: 
- Start with simple components
- Use ratatui's existing patterns
- Component library approach

### 2. Async/Await Learning Curve
**Risk**: Complexity of Rust's async model
**Mitigation**:
- Use tokio's high-level abstractions
- Avoid complex Future combinators
- Heavy testing and documentation

### 3. API Rate Limits & Retries
**Risk**: Complex retry logic needed
**Mitigation**:
- Use `backoff` crate for retry logic
- Comprehensive testing with VCR cassettes
- Rate limit tracking

### 4. Performance
**Risk**: Rust startup time vs Node.js
**Mitigation**:
- Profile early and often
- Use release builds for distribution
- Lazy loading for heavy dependencies

---

## DEPENDENCIES SUMMARY

### Core (Essential)
- tokio - async runtime
- ratatui - terminal UI
- reqwest - HTTP client
- serde/serde_json - serialization
- clap - CLI parsing

### File Operations
- tokio::fs - async file I/O
- globwalk - glob patterns
- regex - regex matching
- ignore - ripgrep-compatible traversal

### Utilities
- thiserror - error types
- anyhow - error handling
- tracing - structured logging
- uuid - unique identifiers

### Optional (Later phases)
- yaml-rust - YAML parsing
- markdown - markdown parsing
- syntect - syntax highlighting
- moka - caching

---

## SUCCESS CRITERIA

1. Basic REPL functionality matches TypeScript version
2. All file I/O tools working
3. Bash tool with permission system
4. Multi-provider LLM support
5. Agent system functional
6. Performance matches or exceeds Node.js version
7. Memory usage < Node.js version
8. Startup time < 100ms

