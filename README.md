# Brio: The Lean Component Kernel

**Brio** is a strictly Headless Micro-Kernel designed to orchestrate artificial intelligences using the **WebAssembly Component Model (WASI Preview 2)**. It operates as a high-security host that composes loosely coupled Components into a unified system, synthesizing the safety of a database kernel with the speed of a service mesh.

## Vision

- **WASI Preview 2 Native**: Plugins are strict "Components" described by WIT, creating a hard security boundary.
- **Active Orchestration**: A central Supervisor component dictates control flow via SQL-based state queries.
- **Zero-Copy Mesh**: High-frequency Agent-to-Tool communication uses internal memory channels.
- **Workspace Isolation**: Sessions operate in isolated temporary directories using atomic file system moves.
- **Battery-Included**: Standard providers for Memory (Vector Store) and State (SQLite).

## Technology Stack

- **Kernel**: Rust (Tokio + Wasmtime)
- **Interface**: WIT (Wasm Interface Type)
- **Runtime**: Wasmtime (Component Model)
- **Mesh**: Rust/Tokio MPSC Channels
- **State**: SQLite (Embedded)
- **UI**: Native WebSocket (JSON Patches)

## Architecture

### 1. The Service Mesh

Direct host-managed routing between components.
`mesh_call("tool_grep", "search", { pattern: "error" })`

### 2. Workspace Isolation

Brio uses directory isolation instead of syscall interception.

- **Start**: Copy/Reflink project to `/tmp/brio/session_id`.
- **Work**: Agent executes in sandbox.
- **Commit**: Atomic diff and move back to base.

### 3. Relational State

State is stored in `brio.db` (SQLite).
`SELECT * FROM tasks WHERE status = 'pending' ORDER BY priority DESC`

## Operational Flow

1. **Trigger**: User sends task via WebSocket.
2. **Supervisor**: Inserts task into SQLite, calls Agent.
3. **Isolation**: Host isolates workspace for Agent.
4. **Action**: Agent modifies files in sandbox.
5. **Commit**: Host atomically applies changes to real project.
