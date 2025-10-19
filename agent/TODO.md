# Kode-rs Porting TODO

## Phase 1: Foundation ✅ COMPLETED

### Project Setup
- [x] Create porting plan
- [x] Initialize Cargo.toml with dependencies
- [x] Set up workspace structure
- [x] Configure .gitignore
- [x] Set up rustfmt.toml and clippy.toml
- [x] Verify project compiles

### Core Types & Traits
- [x] Define Tool trait
- [x] Define ToolContext
- [x] Define ValidationResult
- [x] Define ToolStreamItem enum
- [x] Define Message types (Message, UserMessage, AssistantMessage, etc.)
- [x] Define Config types (Config, GlobalConfig, ProjectConfig)
- [x] Define Model types (ModelProfile, ModelPointer, ProviderType)
- [x] Define Error types (using thiserror)

### Configuration System
- [x] Implement Config struct
- [x] Implement ModelProfile struct
- [x] Implement ModelPointer struct
- [x] Implement config loading (JSON for now, matching TypeScript)
- [x] Implement config merging (global + project + env)
- [x] Implement config validation
- [x] Add tests for config loading/saving

### CLI
- [x] Implement basic CLI structure with clap
- [x] Add commands: repl, query, config, models, agents, version
- [x] Verify CLI works (--help, version)

## Phase 2: Services Layer ⏸️ NEXT

### Model Adapters
- [ ] Define ModelAdapter trait
- [ ] Implement AnthropicAdapter
- [ ] Implement OpenAIAdapter
- [ ] Implement streaming support
- [ ] Implement token counting

### MCP Client
- [ ] Port MCP client basics
- [ ] Implement server discovery
- [ ] Implement tool schema parsing

## Phase 3: Core Tools (Priority 1)

- [ ] FileReadTool (with line range support)
- [ ] FileWriteTool (create files)
- [ ] FileEditTool (diff-based editing)
- [ ] BashTool (command execution with streaming)
- [ ] GlobTool (file pattern matching)
- [ ] GrepTool (content search with ripgrep-style)
- [ ] lsTool (directory listing)

## Phase 4: Advanced Tools (Priority 2)

- [ ] MemoryReadTool
- [ ] MemoryWriteTool
- [ ] ThinkTool
- [ ] TodoWriteTool
- [ ] MultiEditTool

## Phase 5: Integration Tools (Priority 2)

- [ ] URLFetcherTool
- [ ] WebSearchTool
- [ ] NotebookReadTool
- [ ] NotebookEditTool
- [ ] MCPTool

## Phase 6: Meta Tools (Priority 2)

- [ ] TaskTool (agent orchestration)
- [ ] ArchitectTool
- [ ] AskExpertModelTool

## Phase 7: TUI Layer

### Basic REPL
- [ ] Set up ratatui + crossterm
- [ ] Implement basic input handling
- [ ] Implement message display
- [ ] Implement syntax highlighting (tree-sitter)
- [ ] Implement scrolling

### Permission System
- [ ] Permission request UI
- [ ] User approval flow
- [ ] Permission caching

### Advanced UI
- [ ] Tool use visualization
- [ ] Progress indicators
- [ ] Error display
- [ ] Multi-line input

## Phase 8: Agent System

- [ ] Implement agent loading from markdown
- [ ] Parse YAML frontmatter
- [ ] Implement agent registry
- [ ] Implement tool filtering
- [ ] Implement agent caching
- [ ] Implement hot reload

## Phase 9: Context & Memory

- [ ] Implement project context
- [ ] Implement codebase analysis
- [ ] Implement Git integration
- [ ] Implement memory persistence
- [ ] Implement context window management

## Phase 10: Testing (Continuous)

### Unit Tests
- [ ] Tool trait tests
- [ ] Config parsing tests
- [ ] Agent loading tests
- [ ] Permission system tests

### Integration Tests
- [ ] CLI command tests (assert_cmd)
- [ ] API integration tests (wiremock)
- [ ] MCP integration tests

### Snapshot Tests
- [ ] Tool output verification (insta)
- [ ] Error message tests
- [ ] UI rendering tests (vt100)

## Phase 11: Polish & Documentation

- [ ] Performance profiling
- [ ] Optimize hot paths
- [ ] Write API documentation
- [ ] Write user guide
- [ ] Write architecture documentation
- [ ] Set up GitHub Actions for CI/CD
- [ ] Create binary releases
- [ ] Publish to crates.io

## Current Progress Summary

### ✅ Completed (Session 1)
1. **Project Foundation**
   - Set up Cargo.toml with all required dependencies
   - Configured rustfmt, clippy, .gitignore
   - Created module structure: cli, config, error, messages, tools

2. **Core Type System**
   - Ported Tool trait with async support
   - Implemented ToolContext, ValidationResult, ToolStreamItem
   - Created message types (Message, ContentBlock, etc.)
   - Defined error types with thiserror

3. **Configuration System**
   - Ported GlobalConfig and ProjectConfig
   - Implemented ModelProfile and ModelPointer
   - Added ProviderType enum with 15+ providers
   - Config loading/saving with tests

4. **CLI**
   - Implemented argument parsing with clap
   - Added commands: repl, query, config, models, agents, version
   - Verified working with --help and version

5. **Build System**
   - Project compiles successfully
   - Release build works
   - Tests included for core modules

### Files Created
```
src/
├── lib.rs              # Library root
├── main.rs             # Binary entry point
├── error.rs            # Error types (92 lines)
├── messages.rs         # Message types (232 lines)
├── cli/
│   └── mod.rs          # CLI parsing (98 lines)
├── config/
│   ├── mod.rs          # Config module (150 lines)
│   ├── models.rs       # Model profiles (322 lines)
│   └── settings.rs     # Global/project settings (327 lines)
└── tools/
    └── mod.rs          # Tool trait (262 lines)

agent/
├── PORTING_PLAN.md     # Comprehensive porting plan (514 lines)
└── TODO.md             # This file
```

### Statistics
- **Total Rust code**: ~1,500 lines
- **Commits**: 13 commits
- **Compilation**: ✅ Clean (0 errors, 0 warnings)

## Current Blockers

None.

## Next Steps

1. **Implement services layer**
   - Define ModelAdapter trait
   - Implement AnthropicAdapter (using reqwest)
   - Implement OpenAIAdapter
   - Add streaming support

2. **Implement first tool: FileReadTool**
   - Input/output types
   - Line range support
   - File validation
   - Tests

3. **Implement core tools**
   - FileWriteTool
   - FileEditTool (with diff generation)
   - BashTool (with streaming output)
   - GlobTool (using wildmatch)
   - GrepTool (using regex)
