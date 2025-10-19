# Kode-rs Porting TODO

## Phase 1: Foundation ✅ STARTED

### Project Setup
- [x] Create porting plan
- [ ] Initialize Cargo.toml with dependencies
- [ ] Set up workspace structure
- [ ] Configure .gitignore
- [ ] Set up rustfmt.toml and clippy.toml

### Core Types & Traits
- [ ] Define Tool trait
- [ ] Define ToolContext and ExtendedToolContext
- [ ] Define ValidationResult
- [ ] Define ToolStreamItem enum
- [ ] Define Message types
- [ ] Define Config types
- [ ] Define Permission types
- [ ] Define Error types (using thiserror)

### Configuration System
- [ ] Implement Config struct
- [ ] Implement ModelConfig struct
- [ ] Implement config loading (TOML)
- [ ] Implement config merging (global + project + env)
- [ ] Implement config validation

## Phase 2: Services Layer

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

## Current Blockers

None yet.

## Next Steps

1. Set up Cargo.toml with all dependencies
2. Create initial module structure
3. Define core traits (Tool, ModelAdapter, etc.)
4. Implement Config system
5. Implement FileReadTool as first tool
