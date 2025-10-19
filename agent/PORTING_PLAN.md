# Kode TypeScript → Rust Porting Plan

**Date:** 2025-10-19
**Repository:** kode-rs
**Status:** ~60% Complete (Foundation Built)

## Executive Summary

This document provides a comprehensive plan for completing the port of Kode from TypeScript/Bun to Rust. The project has a solid foundation with core infrastructure complete, but needs the REPL/TUI layer and streaming support to become functional.

### Current State
- ✅ **Core infrastructure:** Message types, error handling, configuration system
- ✅ **Service layer:** Anthropic and OpenAI adapters (non-streaming)
- ✅ **Tool system:** 8 tools implemented (FileRead, FileWrite, FileEdit, Bash, Glob, Grep, MemoryRead, MemoryWrite)
- ✅ **Agent system:** Full agent loader with 5-tier priority, hot reload
- ✅ **Tests:** 51 tests passing
- ❌ **REPL/TUI:** Not implemented (critical gap)
- ❌ **Streaming:** SSE parsing and real-time updates missing
- ❌ **Advanced tools:** ThinkTool, TodoWriteTool, TaskTool, etc.

### Estimated Completion
- **MVP (basic REPL + streaming):** 2-3 weeks
- **Feature Complete:** 6-8 weeks
- **Production Ready:** 10-12 weeks

---

## Phase 1: Critical Foundation (Week 1-2) 🔥 HIGH PRIORITY

### 1.1 Fix Remaining Issues
- [x] Fix memory tools compilation
- [x] Fix all test errors
- [ ] Remove duplicate `tool_trait.rs` file
- [ ] Consolidate Tool trait definition in `mod.rs`

### 1.2 Implement Streaming Support (40-60 hours)
**Priority: CRITICAL** - Required for basic functionality

#### Anthropic Streaming
- Implement SSE (Server-Sent Events) parser
- Handle `message_start`, `content_block_start`, `content_block_delta`, `message_delta`, `message_stop`
- Implement incremental content assembly
- Add error handling for stream interruptions

**Files to create/modify:**
- `src/services/streaming/sse_parser.rs` (new)
- `src/services/streaming/mod.rs` (new)
- `src/services/anthropic.rs` (add streaming methods)

**Reference:**
```typescript
// Original TypeScript: src/services/claude.ts
export async function* createStreamingMessage(...)
```

#### OpenAI Streaming
- Similar SSE parser (slightly different format)
- Handle `data: [DONE]` marker
- Delta content assembly

**Files:**
- `src/services/openai.rs` (add streaming methods)

### 1.3 Basic REPL/TUI Implementation (60-80 hours)
**Priority: CRITICAL** - User-facing interface

Replace Ink/React with ratatui. Create minimal working REPL.

#### Core Components
1. **Terminal Setup** (`src/tui/terminal.rs`)
   - Initialize crossterm backend
   - Setup raw mode
   - Handle cleanup on exit

2. **Message Display** (`src/tui/message_view.rs`)
   - Render user messages
   - Render assistant messages with streaming
   - Syntax highlighting (use tree-sitter)
   - Scroll handling

3. **Input Handler** (`src/tui/input.rs`)
   - Multiline input support
   - Vi/Emacs keybindings
   - Command history
   - Auto-completion

4. **Main Loop** (`src/tui/repl.rs`)
   - Event loop (keyboard, resize, etc.)
   - State management
   - Message orchestration

**Reference:**
```typescript
// Original: src/screens/REPL.tsx
function REPL({ config, ... }) {
  // Uses Ink (React for CLI)
  // Port to ratatui event loop
}
```

**Example Structure:**
```rust
// src/tui/mod.rs
pub struct ReplState {
    messages: Vec<Message>,
    input_buffer: String,
    scroll_offset: usize,
    mode: Mode, // Normal, Insert, Command
}

pub async fn run_repl(config: Config) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut state = ReplState::new();

    loop {
        terminal.draw(|f| {
            render_ui(f, &state);
        })?;

        if let Event::Key(key) = event::read()? {
            handle_key(key, &mut state).await?;
        }
    }
}
```

---

## Phase 2: Essential Tools (Week 3-4)

### 2.1 ThinkTool (Extended Thinking)
**Priority: HIGH** - Required for complex reasoning

- Implement extended thinking for Claude models
- Parse `<thinking>` tags from responses
- Display thinking process in UI

**Files:**
- `src/tools/think/mod.rs` (new)

**Reference:**
```typescript
// src/tools/ThinkTool/ThinkTool.tsx
```

### 2.2 TodoWriteTool (Task Tracking)
**Priority: HIGH** - Used by agents for task management

- Parse todo list format
- Track task status (pending, in_progress, completed)
- Render task list in TUI

**Files:**
- `src/tools/todo_write/mod.rs` (new)
- `src/tui/todo_view.rs` (new)

**Reference:**
```typescript
// src/tools/TodoWriteTool/TodoWriteTool.tsx
```

### 2.3 TaskTool (Agent Orchestration)
**Priority: MEDIUM-HIGH** - Enables sub-agents

- Load agent configurations
- Delegate tasks to sub-agents
- Manage agent context and communication

**Files:**
- `src/tools/task/mod.rs` (new)
- `src/agents/orchestrator.rs` (new)

**Reference:**
```typescript
// src/tools/TaskTool/TaskTool.tsx
// src/utils/agentLoader.ts (already ported!)
```

---

## Phase 3: Additional Tools (Week 5-6)

### 3.1 Web Tools
- **URLFetcherTool** - Fetch web content
- **WebSearchTool** - Search integration

### 3.2 Advanced Editing
- **MultiEditTool** - Batch file edits
- **NotebookEditTool** - Jupyter notebook support

### 3.3 MCP Integration
- **MCPTool** - Model Context Protocol
- Server/client implementation
- Tool discovery

**Files:**
- `src/tools/url_fetcher/mod.rs`
- `src/tools/web_search/mod.rs`
- `src/tools/multi_edit/mod.rs`
- `src/tools/notebook_edit/mod.rs`
- `src/tools/mcp/mod.rs`
- `src/services/mcp_server.rs`

---

## Phase 4: Context Management (Week 7-8)

### 4.1 MessageContextManager
**Priority: MEDIUM** - Intelligent token management

- Implement token counting (tiktoken-rs)
- Smart context truncation
- Message prioritization

**Files:**
- `src/context/message_manager.rs` (new)
- `src/context/token_counter.rs` (new)

**Reference:**
```typescript
// src/utils/messageContextManager.ts
```

### 4.2 Project Context
- Codebase indexing
- File relationship tracking
- Intelligent file suggestions

**Files:**
- `src/context/project.rs` (new)
- `src/context/indexer.rs` (new)

---

## Phase 5: Polish & Production (Week 9-12)

### 5.1 Testing
- [ ] End-to-end tests with `assert_cmd`
- [ ] Integration tests for all tools
- [ ] TUI tests with `vt100` terminal emulation
- [ ] Snapshot tests with `insta`
- [ ] HTTP mocking with `wiremock`

**Target:** 80% code coverage

### 5.2 Documentation
- [ ] API documentation (rustdoc)
- [ ] User guide (book)
- [ ] Architecture documentation
- [ ] Contributing guide

### 5.3 Performance
- [ ] Benchmark critical paths
- [ ] Optimize token counting
- [ ] Memory profiling
- [ ] Async runtime tuning

### 5.4 Distribution
- [ ] Cross-platform builds (Linux, macOS, Windows)
- [ ] GitHub Actions CI/CD
- [ ] Cargo publish preparation
- [ ] Homebrew formula
- [ ] Binary releases

---

## Technical Decisions & Patterns

### Async Patterns
```rust
// Streaming tool pattern
async fn call(&self, input: Self::Input, ctx: ToolContext)
    -> Result<ToolStream<Self::Output>>
{
    Ok(Box::pin(stream! {
        // Yield progress updates
        yield Ok(ToolStreamItem::Progress {
            content: "Processing...".into(),
            normalized_messages: None,
        });

        // Yield final result
        yield Ok(ToolStreamItem::Result {
            data: output,
            result_for_assistant: None,
        });
    }))
}
```

### Error Handling
```rust
// Use thiserror for error types
#[derive(Debug, thiserror::Error)]
pub enum KodeError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("API error: {0}")]
    ApiError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

// Use anyhow for application-level errors
pub type Result<T> = anyhow::Result<T>;
```

### TUI Architecture
```rust
// ratatui event loop pattern
pub struct App {
    state: AppState,
    components: Vec<Box<dyn Component>>,
}

#[async_trait]
pub trait Component: Send + Sync {
    fn render(&self, f: &mut Frame, area: Rect);
    async fn handle_event(&mut self, event: Event) -> Result<EventResult>;
}

pub enum EventResult {
    Handled,
    NotHandled,
    Quit,
}
```

---

## File Organization

```
kode-rs/
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── lib.rs                  # Library root
│   ├── cli/                    # Command-line interface
│   │   └── mod.rs             # ✅ DONE
│   ├── tui/                    # Terminal UI (ratatui)
│   │   ├── mod.rs             # ❌ TODO
│   │   ├── repl.rs            # ❌ TODO - Main REPL loop
│   │   ├── terminal.rs        # ❌ TODO - Terminal setup
│   │   ├── message_view.rs    # ❌ TODO - Message rendering
│   │   ├── input.rs           # ❌ TODO - Input handling
│   │   ├── todo_view.rs       # ❌ TODO - Todo list display
│   │   └── components/        # ❌ TODO - Reusable UI components
│   ├── config/                # Configuration system
│   │   ├── mod.rs             # ✅ DONE
│   │   ├── settings.rs        # ✅ DONE
│   │   └── models.rs          # ✅ DONE
│   ├── services/              # LLM service adapters
│   │   ├── mod.rs             # ✅ DONE
│   │   ├── anthropic.rs       # ✅ DONE (non-streaming)
│   │   ├── openai.rs          # ✅ DONE (non-streaming)
│   │   └── streaming/         # ❌ TODO
│   │       ├── mod.rs         # ❌ TODO
│   │       ├── sse_parser.rs  # ❌ TODO
│   │       └── assembler.rs   # ❌ TODO
│   ├── tools/                 # Tool implementations
│   │   ├── mod.rs             # ✅ DONE
│   │   ├── file_read.rs       # ✅ DONE
│   │   ├── file_write.rs      # ✅ DONE
│   │   ├── file_edit.rs       # ✅ DONE
│   │   ├── bash.rs            # ✅ DONE
│   │   ├── glob.rs            # ✅ DONE
│   │   ├── grep.rs            # ✅ DONE
│   │   ├── memory_read.rs     # ✅ DONE
│   │   ├── memory_write.rs    # ✅ DONE
│   │   ├── think/             # ❌ TODO
│   │   ├── todo_write/        # ❌ TODO
│   │   ├── task/              # ❌ TODO
│   │   ├── url_fetcher/       # ❌ TODO
│   │   ├── web_search/        # ❌ TODO
│   │   ├── multi_edit/        # ❌ TODO
│   │   ├── notebook_edit/     # ❌ TODO
│   │   └── mcp/               # ❌ TODO
│   ├── agents/                # Agent system
│   │   └── mod.rs             # ✅ DONE
│   ├── context/               # Context management
│   │   ├── mod.rs             # ❌ TODO
│   │   ├── message_manager.rs # ❌ TODO
│   │   ├── token_counter.rs   # ❌ TODO
│   │   ├── project.rs         # ❌ TODO
│   │   └── indexer.rs         # ❌ TODO
│   ├── messages.rs            # ✅ DONE
│   └── error.rs               # ✅ DONE
├── tests/                     # Integration tests
│   ├── cli_tests.rs           # ❌ TODO
│   ├── tool_tests.rs          # ❌ TODO
│   └── e2e_tests.rs           # ❌ TODO
├── Cargo.toml                 # ✅ DONE
└── README.md                  # ✅ DONE
```

---

## Dependencies Roadmap

### Already Added ✅
- clap (CLI)
- ratatui, crossterm (TUI)
- tokio, async-trait, futures (async)
- reqwest, axum (HTTP)
- serde, serde_json, toml (serialization)
- anyhow, thiserror (errors)
- tree-sitter (code analysis)
- Many utilities

### To Add 📦
```toml
[dependencies]
# Token counting
tiktoken-rs = "0.5"

# Markdown rendering in TUI
termimad = "0.29"

# HTTP SSE streaming
eventsource-stream = "0.2"

# Better async channels
async-channel = "2"

# Terminal colors
owo-colors = "4"

# Clipboard support
arboard = "3"

[dev-dependencies]
# More test utilities (already have many)
rstest = "0.19"  # Parameterized tests
```

---

## Priority Matrix

### Week 1-2 (Must Have) 🔥
1. **Streaming Support** - Without this, the REPL can't show real-time responses
2. **Basic REPL** - Minimal working interface
3. **Fix tool_trait.rs** - Clean up duplicate code

### Week 3-4 (Should Have) ⚡
4. **ThinkTool** - Extended thinking
5. **TodoWriteTool** - Task tracking
6. **TaskTool** - Agent delegation

### Week 5-6 (Nice to Have) ✨
7. **Web tools** - URL fetching, search
8. **Advanced editing** - MultiEdit, Notebook
9. **MCP** - Plugin system

### Week 7-12 (Polish) 💎
10. **Context management** - Smart truncation
11. **Testing** - Comprehensive coverage
12. **Documentation** - User & dev docs
13. **Distribution** - CI/CD, releases

---

## Risk Assessment

### High Risk ⚠️
1. **Streaming complexity** - SSE parsing can be tricky
   - **Mitigation:** Use well-tested `eventsource-stream` crate
   - **Fallback:** Start with line-by-line parsing

2. **TUI learning curve** - ratatui is different from React
   - **Mitigation:** Study examples, start minimal
   - **Fallback:** Simple line-based interface first

3. **Token counting accuracy** - Must match OpenAI's tiktoken
   - **Mitigation:** Use `tiktoken-rs` (official Rust port)
   - **Fallback:** Approximation with character count

### Medium Risk ⚡
4. **Cross-platform compatibility** - Windows vs Unix
   - **Mitigation:** Test on all platforms early
   - **Fallback:** Platform-specific code paths

5. **Performance** - Rust should be faster, but async can be tricky
   - **Mitigation:** Profile early, optimize hot paths
   - **Fallback:** Simplify async where needed

### Low Risk ✅
6. **Tool implementations** - Patterns are established
7. **Testing** - Good ecosystem support
8. **Documentation** - Rustdoc is excellent

---

## Success Criteria

### MVP (Week 2)
- [ ] REPL starts and shows prompt
- [ ] User can type a message
- [ ] Streaming response displays in real-time
- [ ] Basic file operations work (read/write/edit)
- [ ] Bash commands execute

### Feature Complete (Week 8)
- [ ] All original Kode tools implemented
- [ ] Agent system works (task delegation)
- [ ] MCP plugin support
- [ ] Context management active
- [ ] 80% test coverage

### Production Ready (Week 12)
- [ ] Comprehensive documentation
- [ ] CI/CD pipeline
- [ ] Cross-platform binaries
- [ ] Performance benchmarks published
- [ ] Community onboarding materials

---

## Development Workflow

### Daily
1. Pick highest priority incomplete task
2. Write failing test first (TDD)
3. Implement feature
4. Ensure all tests pass
5. Commit and push after EACH file edit (per instructions)
6. Update this plan

### Weekly
1. Review progress vs. plan
2. Adjust priorities based on blockers
3. Refactor technical debt
4. Performance profiling
5. Documentation updates

---

## Resources

### Documentation
- Original Kode: `/Users/khongtrunght/work/captcha/repomirror/claude-clone/research_resource/Kode/`
- Architecture docs: `ARCHITECTURE_SUMMARY.md`, `KODE_RUST_PORT_INVENTORY.md`
- Ratatui: https://ratatui.rs/
- Anthropic API: https://docs.anthropic.com/

### Key Files to Reference
- Original REPL: `Kode/src/screens/REPL.tsx`
- Streaming: `Kode/src/services/claude.ts`
- Tool pattern: `Kode/src/tools/FileReadTool/FileReadTool.tsx`
- Agent system: `Kode/src/utils/agentLoader.ts` (already ported!)

---

## Next Immediate Steps

1. **Remove duplicate `tool_trait.rs`** (5 min)
   ```bash
   cd src/tools
   rm tool_trait.rs
   # Update any imports to use mod.rs
   ```

2. **Start streaming implementation** (Day 1-2)
   ```bash
   mkdir -p src/services/streaming
   # Create sse_parser.rs
   # Add eventsource-stream dependency
   ```

3. **Basic REPL skeleton** (Day 3-5)
   ```bash
   mkdir -p src/tui
   # Create minimal event loop
   # Test with static content first
   ```

4. **Integrate streaming + REPL** (Day 6-10)
   ```bash
   # Connect the pieces
   # Test end-to-end flow
   ```

---

**Last Updated:** 2025-10-19
**Next Review:** 2025-10-22
**Current Focus:** Streaming implementation
