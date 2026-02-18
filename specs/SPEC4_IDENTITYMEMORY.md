# SPEC4: Identity + Chat Memory

Date: 2026-02-16  
Status: Proposed (ready for implementation)

## 1. Goal

Define and implement the memory model you described:

- `IDENTITY.md` is global and shared across all chats.
- `MEMORY.md` is chat-specific.

This spec introduces that behavior without breaking existing project/workspace memory routes.

## 2. Scope

In scope:

- API data model changes for identity + chat memory.
- New API endpoints to read/write/process identity and chat memory.
- Generation pipeline changes to inject memory in the correct order.
- Backward-compatible migration path from current project/workspace memory.
- WebGUI integration points.

Out of scope:

- Full auth redesign.
- Vector DB migration (stay on current SQLite + embedding JSON for now).
- Large TUI UX redesign.

## 3. Current State (as of 2026-02-16)

- `MemoryDocument.scope` supports only `project | workspace`.
- Memory endpoints:
  - `GET/PUT /projects/:projectId/memory`
  - `GET/PUT /workspaces/:workspaceId/memory`
  - process/search endpoints for project/workspace memory
- Generation currently injects retrieved context from project/workspace memory via `getRelevantContext(...)`.
- No first-class `IDENTITY.md` and no chat-level `MEMORY.md`.

## 4. Target Behavior

### 4.1 Identity

- There is one global identity document (`IDENTITY.md`) for the whole deployment.
- It is always considered during generation.
- It is stable and does not vary by project/chat.

### 4.2 Chat memory

- Each chat has its own `MEMORY.md`.
- It stores durable thread-specific facts, decisions, and preferences.
- Retrieval for a chat should prioritize its own memory chunks first.

### 4.3 Injection order (authoritative)

For `POST /v1/chats/:chatId/run`, build prompt context in this order:

1. System base prompt
2. Global `IDENTITY.md`
3. Chat `MEMORY.md` retrieved chunks
4. Existing chat messages
5. Current user message

Optional compatibility layer during migration:

- include project/workspace memory retrieval behind a feature flag.

## 5. Data Model Changes

## 5.1 Prisma schema updates

### `MemoryDocument`

- Extend `scope` values to include:
  - `identity`
  - `chat`
  - (keep `project`, `workspace` for compatibility)
- Add optional `chatId` relation.

### `Chat`

- Add relation:
  - `memories MemoryDocument[]`

### Constraints

- Keep existing uniqueness for old scopes.
- Add uniqueness behavior for new scopes:
  - exactly one `identity` row (enforced in app logic + unique index strategy)
  - one `chat` memory per `chatId`

Suggested practical uniqueness:

- `@@unique([scope, chatId])` for chat scope
- app-level guard for singleton identity (or dedicated table if preferred)

## 5.2 Migration strategy

Phase-safe migration:

1. Add nullable `chatId` column and relation.
2. Deploy code that can read both old and new scopes.
3. Introduce new endpoints.
4. Enable new generation path.
5. Later, optionally deprecate workspace memory usage in generation.

No destructive backfill required.

## 6. API Contract Additions

All routes under `/v1`.

### 6.1 Identity routes

- `GET /identity`
  - returns global identity doc
  - auto-creates default if missing
- `PUT /identity`
  - updates global identity content
- `POST /identity/process`
  - regenerates chunks/embeddings

### 6.2 Chat memory routes

- `GET /chats/:chatId/memory`
  - returns chat memory
  - auto-creates default `MEMORY.md` if missing
- `PUT /chats/:chatId/memory`
  - updates chat memory content
- `POST /chats/:chatId/memory/process`
  - regenerates chunks/embeddings for this chat memory
- `POST /chats/:chatId/memory/search`
  - semantic search within chat memory

Response envelope matches existing memory routes:

```json
{
  "memory": {
    "id": "string",
    "content": "string",
    "updatedAt": "ISO date"
  }
}
```

## 7. Generation Pipeline Changes

Update `src/routes/generation.ts` + retrieval service:

1. Load identity memory (scope `identity`) and include it as system context.
2. Retrieve top-K chunks from chat memory (scope `chat`, `chatId = current`).
3. Build final provider messages with deterministic ordering.
4. Keep model failover logic unchanged.

Recommended defaults:

- identity chunks: top 3 (or full doc if short)
- chat memory chunks: top 5
- min similarity: 0.5 (same as current default)

## 8. Backward Compatibility

Keep existing endpoints and data:

- project/workspace memory routes continue working.
- existing docs/chunks remain valid.

Introduce feature flag:

- `MEMORY_V2_ENABLED=true`:
  - enables identity + chat memory injection path.
- if off:
  - current project/workspace retrieval path remains active.

This allows safe rollout on production without breaking current chats.

## 9. WebGUI Changes

Minimum required UI work:

- Settings menu:
  - add `Identity` editor (global)
  - add `Chat Memory` editor in chat context
- Chat composer flow:
  - no UX changes required for send/run path
- Optional:
  - show status badges: `Identity active`, `Chat memory active`

## 10. Security + Guardrails

- Treat `IDENTITY.md` as privileged content:
  - only authenticated/admin users can edit.
- Input size limits for identity/chat memory updates.
- Keep markdown rendering sanitized in WebGUI.
- Never execute memory content; memory is data only.

## 11. Acceptance Criteria

API:

- `GET /v1/identity` auto-creates and returns singleton identity doc.
- `GET /v1/chats/:chatId/memory` auto-creates chat memory doc.
- processing endpoints regenerate chunks without errors.

Generation:

- run endpoint injects identity + chat memory in defined order.
- if embeddings unavailable, generation still works (no hard failure).

Compatibility:

- existing project/workspace memory endpoints still pass current tests.

UI:

- user can edit identity and chat memory via WebGUI.
- edits affect subsequent generations.

## 12. Implementation Plan

1. Prisma schema + migration for `chatId` and scope extensions.
2. Add identity/chat memory routes in `src/routes/memory.ts`.
3. Extend retrieval service with:
   - `searchIdentityMemory(...)`
   - `searchChatMemory(...)`
4. Update generation injection logic.
5. Add integration tests for:
   - identity auto-create
   - chat memory auto-create
   - generation context ordering
6. Add WebGUI editors and wire to new endpoints.
7. Roll out behind `MEMORY_V2_ENABLED`, then default on.

## 13. Open Decisions

- Should identity be a special singleton table instead of `MemoryDocument(scope='identity')`?
- Should chat memory auto-process on every save, or explicit process endpoint only?
- Should project/workspace memory remain retrieval sources after V2 stabilizes?

