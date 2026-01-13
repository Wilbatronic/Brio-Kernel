# Brio: The Lean Component Kernel

**Brio** is a strictly headless micro-kernel designed to orchestrate artificial intelligence agents using the **WebAssembly Component Model (WASI Preview 2)**.

It synthesizes the safety of a database kernel with the speed of a service mesh. Brio operates as a high-security host that orchestrates loosely coupled Components (Agents, Tools, Supervisors) via internal memory channels and atomic filesystem isolation.

## Core Philosophy

- **WASI Preview 2 Native**: Plugins are strict "Components" bound by WIT (Wasm Interface Type). This creates a hard security boundary with zero runtime ambiguity.
- **Zero-Copy Mesh**: Agent-to-Tool communication happens via host-managed Rust `mpsc` channels. No HTTP overhead, no serialization where possible.
- **Directory-Based Isolation**: Brio replaces complex syscall interception with simple VFS mounting. Sessions run in temporary copies; commits are atomic directory moves.
- **Active Orchestration**: Control flow is dictated by a "Supervisor" component that queries a local SQLite state, rather than a passive event bus.

## Architecture

### 1. The Service Mesh (Host IPC)

Brio removes external message brokers (like NATS or Redis). The Host acts as the switchboard using direct memory channels.

- **Routing**: `mesh_call("tool_grep", "search", { pattern: "error" })`
- **Mechanism**: Direct Rust/Tokio channels.
- **Latency**: Near-native.

### 2. Workspace Isolation (The VFS Strategy)

To prevent agents from hallucinating destructive file changes, Brio implements a strict "Checkout, Sandbox, Commit" lifecycle.

1. **Start**: Host performs a `cp -r` (or reflink) of the project to `/tmp/brio/session_id`.
2. **Work**: The Agent is sandboxed inside this temporary directory. It has no write access to the real project.
3. **Commit**: The Host diffs the session folder and atomically applies changes to the base project.

### 3. Relational State

State is not a loose collection of JSON files; it is strictly relational, stored in an embedded `brio.db` (SQLite).

```sql
-- The Supervisor queries tasks deterministically
SELECT * FROM tasks WHERE status = 'pending' ORDER BY priority DESC

```

## Technology Stack

| Component        | Tech                    | Role                                         |
| ---------------- | ----------------------- | -------------------------------------------- |
| **Kernel**       | Rust (Tokio + Wasmtime) | Lifecycle management & WASI Host.            |
| **Runtime**      | Wasmtime                | Component Model execution.                   |
| **Interface**    | WIT                     | Contract definition between Host and Guests. |
| **State**        | SQLite                  | Embedded relational storage.                 |
| **UI Transport** | Native WebSocket        | JSON Patches for UI broadcasting.            |

## Interface Definitions (WIT)

The contract between the Brio Kernel and the Components is defined strictly in WIT:

```wit
package brio:core;

// 1. Service Mesh: Direct component-to-component calls
interface service-mesh {
    variant payload {
        json(string),
        binary(list<u8>)
    }
    call: func(target: string, method: string, args: payload) -> result<payload, string>;
}

// 2. State: SQL capabilities for the Supervisor
interface sql-state {
    record row {
        columns: list<string>,
        values: list<string>
    }
    query: func(sql: string, params: list<string>) -> result<list<row>, string>;
    execute: func(sql: string, params: list<string>) -> result<u32, string>;
}

// 3. Workspace: Atomic FS operations
interface session-fs {
    // Creates a sandboxed copy of the target directory
    begin-session: func(base-path: string) -> result<string, string>;

    // Applies changes back to the original directory
    commit-session: func(session-id: string) -> result<_, string>;
}

world brio-host {
    import service-mesh;
    import sql-state;
    import session-fs;
    import wasi:logging/logging;
}

```

## Directory Structure

```text
brio-core/
├── wit/                 # The Law
│   ├── host.wit         # Core capabilities
│   └── mesh.wit         # Mesh interfaces
├── kernel/              # The Enforcer (Rust)
│   ├── src/
│   │   ├── mesh/        # Internal Channel Router
│   │   ├── store/       # SQLite Logic
│   │   └── vfs/         # Workspace Copy/Reflink Logic
│   └── src/main.rs
└── components/          # The Logic (Wasm)
    ├── supervisor/      # The "Brain" (Policy Engine)
    ├── agents/          # Stateful Workers
    └── tools/           # Stateless Functions

```

## Operational Flow

**Scenario**: User requests a bug fix via WebSocket.

1. **Trigger**: User sends `{ "type": "task", "content": "Fix bug" }`.
2. **Supervisor**:

- Receives event.
- Executes `INSERT INTO tasks...` via `sql-state`.
- Selects `agent_coder` for the job.

3. **Isolation**:

- Host triggers `session-fs.begin-session("./src")`.
- Project is copied to `/tmp/brio/sess-123`.
- Host mounts `/tmp` as the Agent's root.

4. **Action**: Agent runs logic, writing files only to the temp directory.
5. **Commit**: Agent requests `commit-session`. Host atomically moves modified files back to real source.
