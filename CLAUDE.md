# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## Project Overview

Starbot is a multi-client AI assistant system with distributed architecture:

- **Starbot_API** (Fastify/TypeScript) - Central backend handling auth, persistence, streaming, and memory injection
- **Starbot_TUI** (Rust/CLI) - Terminal client with device-based auth and TUI mode
- **Starbot_WebGUI** (Next.js/React) - Web client for project/workspace/memory management

All three clients communicate via a unified REST + SSE API. The API is the source of truth for all business logic.

---

## Development Quick Start

### Starbot_API (TypeScript/Fastify)

**Install & Run:**
```bash
cd Starbot_API
npm install
npm run dev              # Watch mode with TSX (recommended for development)
npm run build            # Compile to dist/
npm start                # Run compiled code
```

**Database:**
```bash
npm run db:push          # Push schema changes to SQLite
npm run db:migrate       # Create migration and apply (interactive)
npm run db:studio        # Open Prisma Studio UI
```

**Testing & Linting:**
```bash
npm test                 # Run all tests (Vitest)
npm run test:watch       # Watch mode for tests
npm run test:coverage    # Coverage report
npm lint                 # ESLint checks
```

**Environment:**
- API runs on port **3737** (localhost only)
- Database is **SQLite** at `./prisma/dev.db`
- Copy `.env.example` to `.env` and fill in provider credentials (OpenAI, Vertex, Azure, Bedrock, etc.)
- Key env vars: `OPENAI_API_KEY`, `VERTEX_PROJECT_ID`, `AWS_ACCESS_KEY_ID`, `DATABASE_URL`

### Starbot_TUI (Rust/Cargo)

**Install & Run:**
```bash
cd Starbot_TUI
cargo build --release    # Build binary to target/release/starbott
./target/release/starbott --help
```

**Development:**
```bash
cargo build              # Debug build
cargo run -- --help      # Run with args
cargo test               # Run tests
```

**Quick Test:**
```bash
./scripts/starbott-dev.sh tui  # Wrapper script for local testing
```

### Starbot_WebGUI (Next.js)

**Install & Run:**
```bash
cd Starbot_WebGUI
npm install
npm run dev              # Dev server on port 3000
npm run build            # Build for production
npm start                # Run production build
npm lint                 # ESLint checks
```

**Environment:**
- Expects API at `http://localhost:3737/v1`
- Proxy configured in `src/proxy.ts` for streaming API calls

---

## Architecture & Core Concepts

### Data Model

**Project**
- Top-level container for organizing chats and workspaces
- Has associated PMEMORY.md (project-wide semantic memory)
- Schema: `id`, `name`, `createdAt`

**Workspace**
- Represents a code repository, folder, or cloud resource
- Belongs to a project
- Types: `"repo"` (GitHub), `"folder"` (local), `"cloud"` (other)
- Has MEMORY.md (workspace-specific semantic memory)
- Schema: `id`, `projectId`, `type`, `identifier`, `createdAt`

**Chat (Thread)**
- Conversation within a project or workspace scope
- Can be scoped to a workspace (`workspaceId`) or project-wide (`workspaceId = null`)
- Schema: `id`, `projectId`, `workspaceId`, `title`, `createdAt`, `updatedAt`

**Message**
- Single message in a chat
- Schema: `id`, `chatId`, `role` ("user" | "assistant"), `content`, `createdAt`

**MemoryDocument**
- Raw memory content (PMEMORY.md or MEMORY.md files)
- Schema: `id`, `scope` ("project" | "workspace"), `scope_id`, `content`, `updatedAt`

**MemoryChunk**
- Semantic chunks of memory documents with embeddings
- Used for retrieval via cosine similarity
- Schema: `id`, `memoryDocumentId`, `chunk_index`, `content`, `tokens`, `embedding`

### Memory Hierarchy

**Workspace Thread** (`workspaceId` is set):
- Injects workspace MEMORY.md automatically
- Retrieves messages from same workspace only
- Cannot access other workspace memories

**Project Thread** (`workspaceId` is null):
- Injects PMEMORY.md automatically
- Retrieves all messages from entire project
- Can retrieve workspace MEMORY.md documents via semantic search (not auto-injected)

Memory injection happens in the `/v1/chats/:id/run` endpoint via the retrieval service.

### Streaming Protocol

All generation occurs via:
```
POST /v1/chats/:chatId/run
```

Clients use fetch + ReadableStream to consume SSE events. Response is streamed as:
- `message.start`
- `message.delta` (content chunks)
- `message.stop`
- `tool.call` (if tools used)
- `tool.result`
- `inference.complete`

EventSource is NOT used; clients must handle raw HTTP streaming.

### API Route Structure

**Projects**
- `GET /v1/projects` - List all projects
- `POST /v1/projects` - Create project
- `GET /v1/projects/:id` - Get project details
- `PUT /v1/projects/:id` - Update project

**Chats**
- `GET /v1/projects/:projectId/chats` - List chats in project
- `POST /v1/projects/:projectId/chats` - Create chat
- `GET /v1/chats/:chatId` - Get chat details
- `PUT /v1/chats/:chatId` - Update chat

**Messages**
- `GET /v1/chats/:chatId/messages` - List messages in chat
- `POST /v1/chats/:chatId/messages` - Add message

**Generation (Streaming)**
- `POST /v1/chats/:chatId/run` - Stream generation with SSE (requires auth token, injects memory, executes tools)

**Workspaces**
- `GET /v1/projects/:projectId/workspaces` - List workspaces in project
- `POST /v1/projects/:projectId/workspaces` - Create workspace
- `GET /v1/projects/:projectId/workspaces/:workspaceId` - Get workspace
- `PUT /v1/projects/:projectId/workspaces/:workspaceId` - Update workspace

**Memory**
- `GET /v1/projects/:projectId/pmemory` - Get PMEMORY.md content
- `PUT /v1/projects/:projectId/pmemory` - Update PMEMORY.md (triggers re-chunking)
- `GET /v1/projects/:projectId/workspaces/:workspaceId/memory` - Get MEMORY.md
- `PUT /v1/projects/:projectId/workspaces/:workspaceId/memory` - Update MEMORY.md

**Models**
- `GET /v1/models` - List available models across all providers

**Inference**
- Legacy endpoints for backward compatibility with WebGUI

---

## Code Organization

### Starbot_API (`src/`)

**`routes/`** - Fastify route handlers
- `projects.ts` - Project CRUD
- `chats.ts` - Chat CRUD
- `messages.ts` - Message CRUD
- `generation.ts` - Chat run (streaming generation)
- `workspaces.ts` - Workspace CRUD
- `memory.ts` - Memory document GET/PUT
- `models.ts` - List available models
- `auth.ts` - Auth flows (device, etc.)
- `inference.ts` - Legacy inference endpoints

**`services/`** - Business logic
- `retrieval.ts` - Semantic search for memory documents (cosine similarity)
- `chunking.ts` - Split memory documents into semantic chunks (by markdown headings, ~800 tokens each)
- `embeddings.ts` - Generate embeddings via OpenAI (text-embedding-3-large)
- `interpreter.ts` - Code execution on Cloudflare Workers AI
- `filesystem-router.ts` - Local file system navigation
- `web-search.ts` - Brave Search integration (optional)

**`providers/`** - LLM provider clients
- `openai.ts` - OpenAI + Azure OpenAI
- `vertex.ts` - Google Vertex AI (Gemini)
- `bedrock.ts` - AWS Bedrock (Claude, Llama, etc.)
- `cloudflare.ts` - Cloudflare Workers AI
- `kimi.ts` - Moonshot/Kimi
- `types.ts` - Provider interface and streaming types

**`security/`**
- `route-guards.ts` - Auth checks and request guards

**`db.ts`** - Prisma client instance

**`env.ts`** - Environment variable validation and defaults

**`index.ts`** - Server entry point (registers routes, CORS, health check)

### Starbot_TUI (`src/`)

**`main.rs`** - CLI entry point using Clap

**`commands/`** - Subcommand implementations
- `chat.rs` - Send chat messages
- `config.rs` - Configuration management
- `auth.rs` - Device authentication
- `workspaces.rs` - Workspace operations
- `tools.rs` - Tool proposal/execution flows
- `usage.rs` - Billing/usage queries
- `whoami.rs` - Current user info

**`tui/`** - Terminal UI for interactive mode
- `mod.rs` - TUI state machine
- `handlers/key.rs` - Keyboard input handling
- `handlers/message.rs` - Message composition
- `handlers/async_ops.rs` - Async operations (API calls)
- `types.rs` - TUI state structures

**`api.rs`** - HTTP client for API calls with streaming support

**`config.rs`** - Config file loading/saving

**`errors.rs`** - Error types

### Starbot_WebGUI (`src/`)

**`app/`** - Next.js pages/layouts

**`components/`** - Reusable React components

**`lib/`** - Utility functions and API client

**`hooks/`** - Custom React hooks

**`store/`** - State management

**`proxy.ts`** - API proxy configuration for streaming

---

## Working with Memory System

### Memory Injection Flow

1. Client calls `POST /v1/chats/:chatId/run` with message
2. API determines scope: workspace or project
3. Retrieval service fetches relevant memory chunks via semantic search
4. Memory chunks injected into system prompt before inference
5. Model generates response with memory context
6. Response streamed back via SSE

### Adding/Updating Memory

**Project Memory:**
```
PUT /v1/projects/:projectId/pmemory
Body: { content: "# Project Context\n..." }
```

**Workspace Memory:**
```
PUT /v1/projects/:projectId/workspaces/:workspaceId/memory
Body: { content: "# Workspace Context\n..." }
```

Memory is automatically:
- Chunked by markdown headings (max ~800 tokens per chunk)
- Embedded using OpenAI text-embedding-3-large
- Indexed for retrieval

### Retrieval & Search

The `retrieval.ts` service:
1. Queries all relevant MemoryChunk entries
2. Uses cosine similarity between user message embedding and chunk embeddings
3. Returns top K chunks (typically 5-10) above a similarity threshold
4. Injects chunks into system prompt for context

---

## Testing

### API Tests

Located in `Starbot_API/src/routes/__tests__/` and `Starbot_API/src/services/__tests__/`

Run tests:
```bash
cd Starbot_API
npm test                 # Single run
npm run test:watch       # Watch mode
npm run test:coverage    # Coverage report
```

Tests use Vitest and test Fastify routes directly with mock data.

### Integration Testing

For manual testing of full flow:
1. Start API: `npm run dev` in Starbot_API
2. Start TUI: `./scripts/starbott-dev.sh tui`
3. Create project, add memory, and test chat streaming

---

## Common Development Tasks

### Adding a New API Endpoint

1. Create route handler in `routes/*.ts`
2. Use Zod schemas for request validation
3. Return Fastify `reply.code(status).send(payload)`
4. Register route in `index.ts` with appropriate prefix
5. Write tests in `routes/__tests__/*.test.ts`

### Adding a New LLM Provider

1. Implement provider interface in `providers/*.ts`:
   - `streamChat()` method for streaming
   - Model availability checking
2. Add provider check in route handlers
3. Add environment variables to `env.ts`
4. Update `/v1/models` endpoint to include new models

### Debugging Streaming

- TUI: Set `RUST_LOG=debug` environment variable
- API: Check `console.log` output from route handler (shown in dev watch output)
- WebGUI: Use browser DevTools Network tab to inspect fetch response stream

### Database Changes

After modifying `prisma/schema.prisma`:
```bash
cd Starbot_API
npm run db:migrate       # Create and apply migration
npm run db:push          # Or just push to SQLite (for development)
npm run db:generate      # Regenerate Prisma client if needed
```

---

## Important Notes

### Authentication & Authorization

- API requires `Authorization: Bearer <token>` header on protected routes
- Tokens are short-lived access tokens or long-lived refresh tokens
- Device auth flow is primary method for TUI
- WebGUI uses browser-based authentication (not yet fully implemented)

### CORS & Networking

- API runs on localhost:3737 and is not directly internet-accessible
- Production deployment proxies through starbot.cloud
- WebGUI proxies API calls through `src/proxy.ts`
- TUI makes direct HTTP requests to configured API URL

### Provider Credentials

- Multiple providers supported: OpenAI, Vertex, Azure, Bedrock, Cloudflare, Kimi
- Credentials loaded from environment variables at startup
- No provider is required; system routes to available models
- Bedrock and Vertex can be challenging to set up (requires GCP/AWS account setup)

### SQLite Database

- Local file-based (`./prisma/dev.db`)
- Not production-ready for multi-user scenarios
- Schema includes all necessary relations and indexes
- Migrations stored in `prisma/migrations/`

---

## Key Files Reference

| File | Purpose |
|------|---------|
| `Starbot_API/prisma/schema.prisma` | Complete database schema |
| `Starbot_API/src/index.ts` | Server initialization and route registration |
| `Starbot_API/src/env.ts` | Environment variable validation |
| `Starbot_API/src/routes/generation.ts` | Core streaming chat endpoint |
| `Starbot_API/src/services/retrieval.ts` | Memory retrieval & injection logic |
| `Starbot_TUI/src/main.rs` | CLI entry point |
| `Starbot_WebGUI/src/proxy.ts` | API proxy configuration |
| `specs/ARCHITECTURE.md` | High-level system design |
| `specs/DR_APICONTRACT.md` | Complete API specification |
| `DEPLOYMENT_STATUS.md` | Current production state |

