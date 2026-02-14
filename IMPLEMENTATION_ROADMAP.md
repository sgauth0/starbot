# Starbot Implementation Roadmap

**Date:** 2026-02-13
**Status:** System in development - core architecture defined, clients need contract alignment

---

## Executive Summary

Starbot is a multi-client AI assistant system with three components:
- **Starbot_API** (Fastify/TypeScript) - Source of truth, handles auth, persistence, streaming
- **Starbot_TUI** (Rust) - Terminal client with device-based auth
- **Starbot_WebGUI** (Next.js) - Web client for project/memory management

**Current State:** API implements core SSE streaming contract. WebGUI and TUI have contract mismatches (wrong endpoints, wrong event names, wrong streaming methods). Memory system, workspace mapping, and integrations are not yet implemented.

**Critical Issue:** Deep research reveals the WebGUI was likely LLM-refactored and broke API compatibility. TUI also has stubs for features not yet in API.

---

## Specification Documents Overview

### 1. ARCHITECTURE.md
**Purpose:** Defines the three-client architecture and responsibilities

**Key Concepts:**
- **Project:** Logical container with PMEMORY.md (project-wide memory)
- **Workspace:** Local folder/repo/cloud env with MEMORY.md (workspace memory)
- **Chat (Thread):** Conversation in workspace or project scope
- **Memory Hierarchy:** Workspace threads inject workspace memory; project threads inject PMEMORY + retrieve across workspaces

**Current Implementation:** ✅ Partial
- API has Project/Chat/Message schema
- ❌ Workspace schema missing
- ❌ Memory injection not implemented
- ❌ Memory hierarchy rules not enforced

**What Needs To Be Done:**
1. Add Workspace model to Prisma schema (`type`, `identifier`, `project_id`)
2. Add MemoryDocument model (`scope`, `scope_id`, `content`)
3. Implement memory injection middleware in chat run endpoint
4. Enforce workspace vs project thread retrieval rules

---

### 2. DR_APICONTRACT.md
**Purpose:** Defines the canonical REST + SSE API contract

**Key Endpoints:**
```
POST /v1/auth/device/start      # Device flow for TUI
POST /v1/auth/device/poll
POST /v1/auth/device/confirm    # WebGUI approval

GET  /v1/projects
POST /v1/projects
GET  /v1/projects/:projectId/workspaces
POST /v1/projects/:projectId/workspaces

GET  /v1/projects/:projectId/memory     # PMEMORY.md
PUT  /v1/projects/:projectId/memory
GET  /v1/workspaces/:workspaceId/memory # MEMORY.md
PUT  /v1/workspaces/:workspaceId/memory

GET  /v1/projects/:projectId/chats
POST /v1/projects/:projectId/chats
GET  /v1/chats/:chatId/messages
POST /v1/chats/:chatId/messages

POST /v1/chats/:chatId/run      # SSE streaming generation
```

**SSE Event Contract:**
```
event: status              data: { message }
event: token.delta         data: { message_id, delta }
event: message.final       data: { message_id, content, usage }
event: chat.updated        data: { chat_id, title }
event: run.error           data: { code, message }
```

**Current Implementation:** ⚠️ API implements core, clients broken
- ✅ API has `/v1/chats/:chatId/run` with correct SSE events
- ✅ API has Projects/Chats endpoints (wrapped responses)
- ❌ WebGUI calls wrong endpoints (`/projects` instead of `/v1/projects`)
- ❌ WebGUI expects wrong response shapes (array instead of `{ projects: [...] }`)
- ❌ WebGUI uses EventSource GET instead of fetch POST
- ❌ WebGUI listens for `assistant.delta` instead of `token.delta`
- ❌ TUI has legacy `/v1/inference/chat` endpoint that doesn't exist
- ❌ Workspace, Memory, Integration endpoints not implemented

**What Needs To Be Done:**

**Phase 1: Fix WebGUI Contract (HIGH PRIORITY)**
1. Update `Starbot_WebGUI/src/lib/config.ts`: change `http://localhost:3000/api` → `http://localhost:3737/v1`
2. Fix `projectsApi.list()` to call `/v1/projects` and unwrap `{ projects }`
3. Fix `chatsApi.list()` to call `/v1/projects/:projectId/chats` and unwrap `{ chats }`
4. Fix `messagesApi.send()` to call `/v1/chats/:chatId/messages`
5. **CRITICAL:** Replace EventSource with fetch + ReadableStream in `use-chat-stream.ts`:
   ```typescript
   const response = await fetch(`${API_BASE_URL}/chats/${chatId}/run`, {
     method: 'POST',
     headers: {
       'Authorization': `Bearer ${token}`,
       'Content-Type': 'application/json',
       'Accept': 'text/event-stream'
     },
     body: JSON.stringify({ mode, workspace_scope, include_project_memory })
   });
   const reader = response.body.getReader();
   // Parse SSE manually
   ```
6. Update event names: `assistant.delta` → `token.delta`, `assistant.final` → `message.final`

**Phase 2: Implement Missing API Endpoints**
1. Add device auth flow (`/v1/auth/device/*`)
2. Add Workspace CRUD (`/v1/projects/:projectId/workspaces`)
3. Add Memory endpoints (`/v1/projects/:projectId/memory`, `/v1/workspaces/:workspaceId/memory`)
4. Add Integration stubs (`/v1/integrations/*`)

**Phase 3: Fix TUI Contract**
1. Remove legacy `/v1/inference/chat` calls
2. Implement device flow login (QR code display)
3. Use `/v1/chats/:chatId/run` for generation
4. Parse `token.delta` events correctly

---

### 3. MEMORY_DESIGN.md
**Purpose:** Defines hierarchical memory system with embeddings and retrieval

**Memory Files:**
- `PMEMORY.md` - Project-level canonical memory (stored in DB)
- `MEMORY.md` - Workspace-level memory (stored in DB, mapped to repo/folder)

**Retrieval Strategy:**
- Workspace threads: Top 5 chat chunks (workspace only) + Top 3 MEMORY.md chunks
- Project threads: Top 8 chat chunks (project-wide) + Top 5 PMEMORY chunks + Top 3 workspace MEMORY chunks

**Embeddings:**
- Model: `text-embedding-3-large` (or configurable)
- Chunking: Split by markdown headings, max 800 tokens, 100 token overlap
- Storage: Vector DB or pgvector

**Current Implementation:** ❌ Not started
- No embeddings infrastructure
- No memory chunking
- No retrieval logic
- No token budgeting

**What Needs To Be Done:**

**Phase 1: Storage**
1. Add Prisma models:
   ```prisma
   model MemoryDocument {
     id         String   @id @default(cuid())
     scope      String   // "workspace" | "project"
     scopeId    String   // workspace_id or project_id
     content    String
     updatedAt  DateTime @updatedAt
   }

   model MemoryChunk {
     id              String   @id @default(cuid())
     memoryId        String
     text            String
     embeddingVector Float[]  // pgvector or JSON
     memory          MemoryDocument @relation(fields: [memoryId], references: [id])
   }
   ```
2. Add ChatChunk model for conversation history embeddings

**Phase 2: Embedding Pipeline**
1. Add chunking service (`src/services/chunking.ts`):
   - Parse markdown by headings
   - Split to 800 token chunks with 100 overlap
2. Add embedding service (`src/services/embeddings.ts`):
   - Use OpenAI `text-embedding-3-large` API
   - Batch embedding requests
3. Hook into memory update endpoints:
   - On PUT `/v1/projects/:projectId/memory`, re-chunk and re-embed
   - On PUT `/v1/workspaces/:workspaceId/memory`, re-chunk and re-embed
4. Hook into message creation:
   - Embed new assistant messages for future retrieval

**Phase 3: Retrieval Service**
1. Implement cosine similarity search (`src/services/retrieval.ts`)
2. Add retrieval logic to `/v1/chats/:chatId/run`:
   - Detect if workspace or project thread
   - Retrieve relevant chunks per MEMORY_DESIGN rules
   - Inject into prompt before user message
3. Implement token budgeting (15% memory, 35% retrieval, 40% conversation, 10% buffer)
4. Add conflict resolution (PMEMORY wins in project scope, MEMORY wins in workspace)

**Phase 4: Memory Editing Workflow**
1. Add memory diff proposal endpoint (model suggests changes)
2. Add user approval flow
3. Apply patches server-side
4. Trigger re-indexing

---

### 4. DEEPRESEARCH.md
**Purpose:** Deep dive analysis of current codebase state and contract mismatches

**Key Findings:**
1. WebGUI has "LLM refactor smell" - broken contracts, wrong assumptions
2. TUI has device auth stubs but API doesn't implement device flow yet
3. API's `test-api.sh` is the canonical streaming contract reference
4. Workspace/repo mapping semantics not implemented anywhere
5. Memory system completely missing

**Actionable Takeaways:**
- Use `test-api.sh` as ground truth for SSE contract
- WebGUI needs significant rework to match API
- TUI is closer to correct but has legacy endpoint calls
- Don't trust WebGUI's current API assumptions

---

## Next Steps Forward

### Immediate (Week 1)
**Goal:** Get WebGUI and TUI working against current API

1. ✅ **Fix WebGUI contract mismatches** (Phase 1 from DR_APICONTRACT section)
   - Update base URL
   - Fix endpoint paths
   - Replace EventSource with fetch streaming
   - Fix event names

2. ✅ **Fix TUI contract** (Phase 3 from DR_APICONTRACT section)
   - Remove `/v1/inference/chat` calls
   - Use `/v1/chats/:chatId/run`

3. ✅ **Manual testing**
   - Run API: `cd Starbot_API && npm run dev`
   - Run WebGUI: `cd Starbot_WebGUI && npm run dev`
   - Test project creation, chat creation, streaming generation

### Short-term (Weeks 2-3)
**Goal:** Implement workspace and basic memory storage

4. **Add Workspace model**
   - Update Prisma schema
   - Implement workspace CRUD endpoints
   - Add workspace → repo/folder mapping (GitHub integration stub)

5. **Add Memory storage**
   - Add MemoryDocument model
   - Implement memory GET/PUT endpoints
   - Basic markdown storage (no embeddings yet)

6. **Implement device auth flow**
   - Add device flow endpoints
   - QR code generation for TUI
   - WebGUI approval UI

### Medium-term (Weeks 4-8)
**Goal:** Full memory system with embeddings and retrieval

7. **Embedding infrastructure**
   - Add chunking service
   - Add embedding service (OpenAI API)
   - Embed memory documents on update

8. **Retrieval system**
   - Implement vector similarity search
   - Add retrieval logic to chat run
   - Inject memory context per hierarchy rules
   - Token budgeting

9. **Chat history embeddings**
   - Embed assistant messages
   - Cross-thread retrieval in project scope

### Long-term (Weeks 9-16)
**Goal:** Integrations and advanced features

10. **GitHub integration**
    - GitHub App setup
    - Workspace ↔ repo sync
    - Auto-update MEMORY.md from repo

11. **Notion integration**
    - OAuth flow
    - Tool endpoints for search/update
    - Memory export to Notion

12. **Advanced memory features**
    - Memory diff proposals
    - User approval workflow
    - Memory versioning
    - Auto-summarization

---

## Implementation Checklist

### API (Starbot_API)
- [ ] Add Workspace model to schema
- [ ] Add MemoryDocument model to schema
- [ ] Add MemoryChunk model to schema
- [ ] Implement `/v1/auth/device/*` endpoints
- [ ] Implement `/v1/projects/:projectId/workspaces` endpoints
- [ ] Implement `/v1/projects/:projectId/memory` endpoints
- [ ] Implement `/v1/workspaces/:workspaceId/memory` endpoints
- [ ] Add chunking service
- [ ] Add embedding service
- [ ] Add retrieval service
- [ ] Inject memory context in `/v1/chats/:chatId/run`
- [ ] Implement token budgeting
- [ ] Add GitHub App integration
- [ ] Add Notion OAuth integration

### WebGUI (Starbot_WebGUI)
- [ ] Fix base URL (`config.ts`)
- [ ] Fix `projectsApi.list()` endpoint path and response shape
- [ ] Fix `chatsApi.list()` endpoint path and response shape
- [ ] Fix `messagesApi.send()` endpoint path
- [ ] Replace EventSource with fetch streaming in `use-chat-stream.ts`
- [ ] Update event names (`token.delta`, `message.final`)
- [ ] Add workspace management UI
- [ ] Add memory editing UI (PMEMORY.md, MEMORY.md)
- [ ] Add device approval UI (QR code scanning)
- [ ] Add integration connection UI

### TUI (Starbot_TUI)
- [ ] Remove legacy `/v1/inference/chat` endpoint calls
- [ ] Implement device auth flow (QR display)
- [ ] Use `/v1/chats/:chatId/run` for generation
- [ ] Parse `token.delta` events
- [ ] Add workspace selection UI
- [ ] Add local file reading for workspace context

---

## Testing Strategy

### Unit Tests
- API: Test each endpoint with mock Prisma client
- Chunking: Test markdown parsing and token limits
- Embeddings: Test batch processing
- Retrieval: Test similarity search and ranking

### Integration Tests
- Full device auth flow (TUI → API → WebGUI)
- Memory injection in workspace thread
- Memory injection in project thread
- Cross-thread retrieval in project scope

### E2E Tests
- Create project → create workspace → link repo → chat in workspace scope
- Update MEMORY.md → verify embedding update → verify retrieval
- Update PMEMORY.md → verify project-wide retrieval

### Manual Testing
- Use `test-api.sh` as contract validation
- Test streaming in WebGUI with browser DevTools (Network tab, EventStream)
- Test TUI device auth with phone camera

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                         Starbot System                       │
└─────────────────────────────────────────────────────────────┘

┌──────────────┐         ┌──────────────┐         ┌──────────────┐
│  Starbot_TUI │◄────────┤ Starbot_API  ├────────►│Starbot_WebGUI│
│   (Rust)     │  Device │  (Fastify)   │  OAuth  │  (Next.js)   │
│              │  Auth   │              │         │              │
│ - QR Login   │         │ - Auth       │         │ - Projects   │
│ - Local Chat │         │ - Projects   │         │ - Memory Edit│
│ - File Read  │         │ - Workspaces │         │ - QR Approve │
└──────────────┘         │ - Memory     │         └──────────────┘
                         │ - Embeddings │
                         │ - Retrieval  │
                         │ - Streaming  │
                         └──────┬───────┘
                                │
                    ┌───────────┼───────────┐
                    │           │           │
                ┌───▼───┐   ┌───▼───┐   ┌──▼──────┐
                │Prisma │   │Vector │   │External │
                │  DB   │   │Search │   │  APIs   │
                │       │   │       │   │         │
                │Project│   │Embeddi│   │ GitHub  │
                │Chat   │   │ngs    │   │ Notion  │
                │Message│   │       │   │ OpenAI  │
                │Memory │   │       │   │         │
                └───────┘   └───────┘   └─────────┘

Memory Hierarchy:

  Project
    ├── PMEMORY.md (project-level memory)
    ├── Workspace 1
    │     ├── MEMORY.md (workspace-level memory)
    │     └── Chats (workspace scope)
    ├── Workspace 2
    │     ├── MEMORY.md
    │     └── Chats
    └── Project Chats (project scope - retrieves across workspaces)
```

---

## Risk Assessment

### High Risk
1. **WebGUI contract drift** - Likely caused by LLM refactor. Requires manual review of every API call.
2. **Embedding infrastructure** - No vector DB chosen yet. Decision: pgvector vs Pinecone vs Weaviate?
3. **Token budgeting complexity** - Dynamic chunking based on context budget is non-trivial.

### Medium Risk
1. **Device auth UX** - QR flow needs careful design for security and usability.
2. **Memory conflict resolution** - PMEMORY vs MEMORY priority needs clear rules and UI feedback.
3. **GitHub App permissions** - Need careful scope selection to avoid over-requesting.

### Low Risk
1. **API streaming contract** - Already working, just need clients to align.
2. **Basic CRUD endpoints** - Straightforward implementation.
3. **Notion integration** - Standard OAuth flow.

---

## Decision Log

### 2026-02-13
- **Decision:** Use fetch + ReadableStream instead of EventSource in WebGUI
  - **Rationale:** EventSource can't send POST bodies or custom headers, incompatible with `/v1/chats/:chatId/run` contract

- **Decision:** Store embeddings in Prisma with Float[] (JSON fallback)
  - **Rationale:** Keep simple for now, migrate to pgvector if performance issues arise

- **Decision:** Use OpenAI `text-embedding-3-large` for embeddings
  - **Rationale:** Best quality, already integrated, can swap later if needed

- **Decision:** Fix WebGUI before implementing new features
  - **Rationale:** No point building new API features if WebGUI can't consume existing ones

---

## Resources

- API Contract: `specs/DR_APICONTRACT.md`
- Architecture: `specs/ARCHITECTURE.md`
- Memory Design: `specs/MEMORY_DESIGN.md`
- Deep Research: `specs/DEEPRESEARCH.md`
- API Test Harness: `Starbot_API/test-api.sh`

---

## Questions to Resolve

1. **Vector DB choice:** pgvector (Postgres extension) vs standalone vector DB?
2. **Embedding model:** Stick with `text-embedding-3-large` or use cheaper/faster model?
3. **Workspace type priority:** GitHub repos first, or local folders first?
4. **Memory update permissions:** Who can edit PMEMORY.md? Workspace owners only?
5. **Token budget limits:** What happens when memory + retrieval exceed budget? Drop memory or retrieval?

---

## Contact & Coordination

- **API Owner:** [Define ownership]
- **WebGUI Owner:** [Define ownership]
- **TUI Owner:** [Define ownership]
- **Specs maintained by:** [Define ownership]

**Next sync:** Schedule architecture review after WebGUI contract fixes are complete.

---

_Last updated: 2026-02-13_
