# Starbot Architecture Specification

Starbot consists of three independent clients:

1. Starbot_API (Source of Truth)
2. Starbot_TUI (Thin local client)
3. Starbot_WebGUI (Integration client)

The API defines all business logic, memory injection rules, and streaming semantics.

---

## Core Concepts

Project
  A logical container of threads and workspaces.
  Has one PMEMORY.md.

Workspace
  A local folder, GitHub repo, or cloud environment.
  Has one MEMORY.md.

Chat (Thread)
  A conversation within either:
    - A workspace scope
    - A project scope

Message
  A single message in a chat.

---

## Memory Hierarchy

Workspace Thread:
  Inject:
    - Workspace MEMORY.md
  Retrieve:
    - Messages from same workspace

Project Thread:
  Inject:
    - PMEMORY.md
  Retrieve:
    - All project chats
    - Workspace MEMORY.md documents via retrieval

Workspace memory is never blindly injected into project threads.

---

## Streaming Model

All generation occurs via:

POST /v1/chats/:chatId/run

SSE events streamed over the same response.

Clients must use fetch + ReadableStream.
EventSource is not used.

---

## Auth Model

WebGUI:
  Standard login (future expansion).
  Approves device linking.

TUI:
  Device flow only.
  Receives refresh + access tokens.

Tokens:
  - Short-lived access token
  - Long-lived refresh token
  - Revocable per device

---

## Integration Model

Integrations are server-side.
Clients never call Notion or GitHub directly.

GitHub:
  Implemented via GitHub App (preferred over OAuth App).

Notion:
  OAuth flow.
  Token stored encrypted in DB.

---

## Client Responsibilities

API:
  - Persistence
  - Retrieval
  - Memory injection
  - Streaming
  - Tool execution

TUI:
  - Local file reading
  - Chat UI
  - Device auth

WebGUI:
  - Project management
  - Integration management
  - Streaming UI
  - Memory editing UI

---

## Design Rules

1. API is authoritative.
2. Clients never invent endpoints.
3. Streaming protocol is uniform.
4. Memory rules enforced server-side.
5. No cross-project retrieval.
