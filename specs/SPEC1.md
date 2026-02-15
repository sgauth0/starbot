
Deep Code Review and Improvement Plan for sgauth0/starbot
Executive summary

I reviewed the current state of sgauth0/starbot using the only enabled connector (github) and focused on the main sources of truth: the TypeScript/Fastify API (“Starbot_API”), the Next.js Web UI (“Starbot_WebGUI”), and the Rust terminal client (“Starbot_TUI”). The recent commit history shows rapid iteration over only a few days, with “monorepo split” and “memory improvements” landing on February 12–14, 2026, which matches what I see in the code: the system is functional in parts (API + SSE streaming + memory chunking/embeddings/retrieval), but the clients are not consistently aligned with the API contract and several endpoints are stubbed or mismatched.

The highest-risk issues are contract incompatibilities (WebGUI and TUI calling endpoints with wrong signatures/paths/field casing), and runtime correctness/build integrity (API imports the OpenAI SDK for embeddings but does not declare the openai dependency, which will break builds).

My prioritized plan is:

Critical (same-day to 1–2 days)

    Fix API build break: add missing openai dependency; align .env.example and env validation.
    Fix WebGUI compile/runtime blockers: correct sidebar.tsx and chat-view.tsx to call chatsApi.list/create and messagesApi.send with the actual signatures (or refactor the APIs to match intended request objects).
    Fix TUI device auth flow: change camelCase ↔ snake_case JSON fields and request bodies to match API routes.

High (3–7 days)

    Consolidate Prisma usage across the API to reuse a single Prisma client (Prisma warns that multiple PrismaClient instances can exhaust DB connection pools and degrade reliability).
    Add CI workflows (Node + Rust), plus security scanning (npm audit, CodeQL, cargo audit) and enforce Node versions required by dependencies: Fastify v5 requires Node 20+ and Next.js 16 requires Node 20.9+.
    Add contract tests for SSE framing and event payload consistency (MDN documents that SSE messages are separated by a blank line and may include multiple data: lines).

Medium (2–4 weeks)

    Improve memory indexing performance and bulk DB writes (Prisma recommends bulk operations for large writes).
    Implement authentication enforcement and rate-limiting to prevent runaway model usage costs.
    Make docs match reality: IMPLEMENTATION_ROADMAP.md appears to lag behind the code (it says memory is “not started” while the code includes memory endpoints and embeddings).

Repository scope and evidence base
Enabled connectors

I used the only enabled connector: github.
Notable commits referenced

From newest to oldest (UTC timestamps from the connector output):

    0fd7fb7681863c21ca193ca4d4c14e6e1a05f6a1 — “separated everything ig and improved memory” (2026-02-14T00:32:27Z).
    0ad582c6f081953b7183632f662dce57bef1f7ee — “split monorepo into 3, reqired the LLMs…” (2026-02-13T05:59:13Z).
    84209f5a8a803deb568997980f2d5916196b8d3c — “Add Starbot_WebGUI source files…” (2026-02-12T17:18:10Z).
    fb009dfecbac3ea549e47c6a23cd4756638fa4ea — initial multi-component commit (2026-02-12T17:10:41Z).

These commits are relevant because most issues I found are consistent with a repository freshly split into components and then rapidly reworked, with clients lagging behind the API contract.
Main files reviewed

I did not receive uploaded files in the conversation, so I treated “provided files” as the repository’s main source/config/docs:

    Specs/docs
        IMPLEMENTATION_ROADMAP.md
        specs/ARCHITECTURE.md
        specs/DR_APICONTRACT.md
        specs/MEMORY_DESIGN.md
        specs/DEEPRESEARCH.md

    Starbot_API (Node/TypeScript/Fastify/Prisma)
        Starbot_API/package.json
        Starbot_API/src/index.ts
        Starbot_API/src/env.ts
        Starbot_API/src/db.ts
        Routes: projects.ts, chats.ts, messages.ts, generation.ts, models.ts, workspaces.ts, memory.ts, auth.ts
        Services: retrieval.ts, embeddings.ts
        Lockfiles: pnpm-lock.yaml, package-lock.json

    Starbot_WebGUI (Next.js)
        Starbot_WebGUI/package.json
        Core client libs: src/lib/config.ts, src/lib/api.ts, src/lib/types.ts
        API wrappers: src/lib/api/projects.ts, src/lib/api/chats.ts, src/lib/api/messages.ts
        SSE stream hook: src/hooks/use-chat-stream.ts
        UI callsites that currently mismatch: src/components/sidebar.tsx, src/components/chat/chat-view.tsx

    Starbot_TUI (Rust)
        Starbot_TUI/Cargo.toml
        Commands: src/commands/auth.rs, src/commands/chat.rs

Findings by component and file
Starbot_API
Starbot_API/package.json

Purpose: Declares the API runtime, scripts, and dependency versions.

Key functions/classes: n/a (configuration).

Coding style issues: n/a.

Bugs:

    Critical: The API code imports the OpenAI SDK (import OpenAI from 'openai') in src/services/embeddings.ts, but openai is not listed in dependencies. This will fail at runtime or during build depending on resolution.

Security vulnerabilities:

    Dependency hygiene risk: The API directory contains both a pnpm-lock.yaml and package-lock.json, which makes installs non-deterministic across environments and increases supply-chain risk surface.

Performance bottlenecks: n/a.

Dependency/version concerns:

    Fastify v5 is used. Fastify’s v5 migration guide states v5 supports Node.js v20+ only, which should drive a repo-wide Node baseline and CI enforcement.

Test coverage gaps:

    No test scripts are present; only dev/build/start and Prisma scripts. This implies tests are either missing or not wired into package scripts.

Documentation gaps:

    No stated Node version policy in package.json (e.g., engines) while dependencies imply Node 20+.

Maintainability concerns:

    With multiple providers and memory features, the API needs basic CI gates now to prevent drift.

Starbot_API/src/index.ts

Purpose: Fastify bootstrap; registers CORS, websocket plugin, routes, and health endpoints.

Key functions/classes:

    Fastify server instance creation, route registration, /v1/health + /health redirect.

Issues by category:

    Bugs: none obvious in the bootstrap itself.
    Security: CORS is restricted to local dev origins and sets credentials: true. If you later move to cookie-based auth, I would add explicit CSRF protections and ensure origin allowlists remain strict.
    Dependency/version: Since @fastify/websocket is registered, but streaming is SSE-based in generation.ts, I would remove the websocket plugin unless it’s planned. Unused plugins expand the audit surface.
    Test gaps: no boot-level smoke test to verify route registration.
    Maintainability: route modules have mixed export styles (some are plain functions, some are FastifyPluginAsync), which adds friction.

Starbot_API/src/env.ts

Purpose: Centralizes environment variables for providers and feature flags and logs a redacted configuration summary.

Key functions:

    isProviderConfigured, listConfiguredProviders, logConfiguration.

Issues:

    Bugs: none obvious.
    Security: Secrets are not printed, which is correct. I would still ensure logs never accidentally include provider request headers or API keys.
    Dependency concerns: The embeddings service depends on OPENAI_API_KEY, but env.ts does not list it—this is a documentation/operational mismatch.
    Maintainability: Add a small validation layer (Zod or custom) to fail fast if required vars are missing for enabled providers.

Starbot_API/src/db.ts

Purpose: Shared Prisma client initialization and graceful shutdown handlers.

Key functions/classes:

    Singleton prisma export, SIGINT/SIGTERM disconnect.

Issues:

    Maintainability/convention: This is the right approach and aligns with Prisma’s guidance to reuse a single PrismaClient in traditional servers.
    Bug/consistency: Several files still instantiate their own new PrismaClient() instead of importing from db.ts (notably workspaces.ts, memory.ts, retrieval.ts). This undermines the intended pattern and can create connection pooling problems.

Starbot_API/src/routes/projects.ts

Purpose: CRUD for projects: list/create/get/delete.

Key functions:

    projectRoutes(server) defines GET /projects, POST /projects, GET /projects/:id, DELETE /projects/:id.

Issues:

    Bugs: no obvious runtime errors.
    Contract: The list endpoint includes _count (extra keys), which is okay for JS clients but should be documented as part of the contract if strongly validated in clients.
    Tests: Needs integration tests for 404 behavior, create validation, and delete idempotency.
    Maintainability: Zod schema validation is used, but there is no Fastify schema integration for OpenAPI generation.

Starbot_API/src/routes/chats.ts

Purpose: Chat CRUD bound to project: list/create chats per project, get/delete chat.

Key functions:

    GET /projects/:projectId/chats, POST /projects/:projectId/chats, GET /chats/:id, DELETE /chats/:id.

Issues:

    Bugs: none obvious in server logic.
    Client contract: WebGUI currently calls chatsApi.list() without a projectId, which will break compilation/runtime and makes the UI unusable as written.
    Tests: should cover “project does not exist” and chat ordering.

Starbot_API/src/routes/messages.ts

Purpose: List/add messages per chat.

Key functions:

    GET /chats/:chatId/messages, POST /chats/:chatId/messages.

Issues:

    Security: Allowing public creation of messages with role tool expands risk and complicates provider-message integrity. I would restrict the public message endpoint to role=user (and possibly system) and keep tool internal unless you have a concrete tool execution pipeline.
    Tests: ensure messages are ordered, chat updatedAt updated, and role constraints.

Starbot_API/src/routes/generation.ts

Purpose: SSE streaming generation at POST /chats/:chatId/run with triage, model selection, memory retrieval injection, streaming tokens, persistence, and final event emission.

Key functions/classes:

    parseModelPrefs, resolveRequestedModel, generationRoutes(server).
    SSE event writing: event: status, event: token.delta, event: message.final, event: chat.updated.

Issues:

    Bug (client-visible): After updating the chat title, the emitted chat.updated event uses title: chat.title (the pre-update value). This makes the event misleading for clients. A quick fix is to compute the new title string once and use it both in the DB update and the event.
    Correctness: When converting DB messages to provider messages, the code casts roles into 'user' | 'assistant' | 'system' without filtering out tool. If tool messages ever exist, this is unsafe and could break provider adapters or produce incorrect prompts.
    SSE robustness: The server emits standard SSE framing (event: + data: + blank line). MDN notes that events are separated by a pair of newlines and can contain multiple consecutive data: lines which are concatenated by clients. While your current payloads are single-line JSON, improving the client parser to tolerate multi-line data will prevent future breakage.
    Security: No auth enforcement; a public /run endpoint is a cost and abuse risk if exposed beyond localhost.
    Tests: Needs contract tests that assert event order and that the response is valid SSE.

Starbot_API/src/routes/models.ts

Purpose: List configured models for clients to populate pickers.

Issues:

    Contract: The API returns { defaultProvider, providers } with entries like ${provider}:${deploymentName}. This must be documented and kept stable if UIs store provider keys.
    Tests: Basic unit test: output sorting and presence of “auto” option.

Starbot_API/src/routes/workspaces.ts

Purpose: Workspace CRUD linked to projects; includes list/create/get/delete.

Issues:

    Critical maintainability/perf: It instantiates new PrismaClient() locally rather than using db.ts. Prisma explicitly recommends reusing PrismaClient to avoid connection pool exhaustion.
    Contract: API returns { workspaces } and { workspace }; clients currently do not appear to consume these endpoints.
    Tests: Add project scoping tests and ensure deletes cascade as expected (depending on Prisma schema).

Starbot_API/src/routes/memory.ts

Purpose: Create/read/update memory documents for projects and workspaces; process memory into chunks, generate embeddings, and support memory search.

Key functions:

    GET/PUT memory documents
    POST /memory/process to regenerate chunks and embeddings
    POST /memory/search

Issues:

    Performance bottleneck: Memory process writes chunks one-by-one in a loop. Prisma recommends bulk operations (createMany) for large writes.
    Dependency correctness: Embeddings call out to OpenAI embeddings via openai SDK but the dependency is not declared in Starbot_API/package.json.
    Maintainability/concurrency: this file also instantiates its own PrismaClient instead of reusing db.ts.
    Security: Memory contents are user-managed. If you render memory as markdown/HTML in WebGUI later, sanitize or render as plain text to avoid XSS.
    Tests: Must cover the “auto-create default PMEMORY/MEMORY” behavior and processing pipeline idempotency.

Starbot_API/src/routes/auth.ts

Purpose: Stub device auth flow: start/poll/confirm/pending endpoints stored in an in-memory Map.

Issues:

    Security: In-memory auth is explicitly non-production; there is no token verification middleware to actually protect the API.
    Resource leak: the cleanup interval marks requests expired but does not remove them from the Map; over time this can leak memory.
    Client mismatch: The API uses snake_case fields (e.g., device_code, user_code, verification_url), while the Rust client expects camelCase (deviceCode, userCode, verificationUrl).
    Tests: Add unit tests for expiry handling and response codes (404, 410, 403).

Starbot_API/src/services/retrieval.ts

Purpose: Semantic retrieval against memory chunks via cosine similarity; produces “Relevant Memory” context injected into prompts.

Issues:

    Performance: It scans all memory chunks and parses JSON embeddings for each query; this will not scale beyond small projects. Consider caching parsed vectors, storing normalized embeddings, and/or migrating to a vector index.
    Maintainability: Creates its own PrismaClient rather than reusing db.ts, contrary to Prisma guidance.
    Tests: Add deterministic tests for cosine similarity, thresholding, topK ordering, and “no embedding available”.

Starbot_API/src/services/embeddings.ts

Purpose: Generate embeddings using OpenAI text-embedding-3-large; supports single and batched embedding generation.

Issues:

    Critical: Missing dependency declaration (openai package not in API dependencies).
    Operational: The model and dimension (3072) are hardcoded; make model name configurable via env for future migration and cost management.
    Tests: If you keep this direct SDK integration, add a testable abstraction and mock it.

Starbot_WebGUI
Starbot_WebGUI/package.json

Purpose: Declares Web GUI dependencies (Next.js 16.1.6, React 19, React Query, Zod v4, etc.).

Issues:

    Runtime constraint: Next.js 16 requires Node.js 20.9+. You should enforce this in tooling and CI to prevent developer drift.

Starbot_WebGUI/src/lib/types.ts

Purpose: Zod schemas and inferred TS types for Project/Chat/Message and settings.

Issues:

    Contract mismatch: SettingsSchema uses speed: 'fast'|'quality', while the API expects speed: boolean and auto: boolean in /chats/:chatId/run. This will cause confusion and/or incorrect requests.
    Maintainability: The file defines request interfaces CreateChatRequest and SendMessageRequest but the rest of the code does not consistently use them.

Starbot_WebGUI/src/lib/api.ts

Purpose: Fetch wrapper that attaches token header, handles JSON parsing, optionally validates with Zod.

Issues:

    Correctness: Strict Zod validation is good, but only if the API contract is stable. Right now, the UI contains endpoints that do not exist on the server, so validation will fail for those flows.
    Security: Token stored in localStorage (via config). If you ever render untrusted HTML, XSS becomes a token exfiltration risk.

Starbot_WebGUI/src/lib/config.ts

Purpose: Defines API base URL and token header.

Issues:

    Contract: Uses X-API-Token, but the API currently does not enforce any authorization; this is fine for localhost but may mislead future hardening.

Starbot_WebGUI/src/lib/api/projects.ts

Purpose: Project API client wrapper.

Bugs:

    Calls PUT /projects/:id for update, but the API does not implement a PUT /projects/:id route. This will 404.

Starbot_WebGUI/src/lib/api/chats.ts

Purpose: Chats API wrapper (list, get, create, update, delete, getMessages).

Bugs:

    Calls PUT /chats/:id, but the API does not implement that route; only GET /chats/:id and DELETE /chats/:id exist.

Starbot_WebGUI/src/lib/api/messages.ts

Purpose: Messages API wrapper.

Bugs:

    Calls PUT /messages/:id, but the API does not implement any /messages/:id route.

Starbot_WebGUI/src/hooks/use-chat-stream.ts

Purpose: Implements fetch-based SSE parsing for POST /chats/:chatId/run and updates React Query caches in response to events.

Issues:

    SSE parsing correctness: The parser resets event type on blank lines and parses each data: line as a standalone JSON. MDN’s SSE rules allow multiple data: lines per event; your server currently emits one JSON line per event, but this is brittle if you ever split JSON across lines.
    Cache update correctness: For token.delta and message.final, when old is undefined it returns [], which can drop the stream output if messages are not preloaded. Consider treating “missing cache” as “start from empty but append,” not “return empty and drop event.”
    Contract mismatch: The server sends token.delta payload as { text }, and message.final as { id, content, usage, ... }. The client tries data.message_id || data.id, which is okay, but it sets metadata: { final: true, ...data.usage } which flattens usage fields into metadata and may confuse message rendering.

Starbot_WebGUI/src/components/sidebar.tsx

Purpose: Sidebar UI listing chats and creating a new chat.

Bugs (blockers):

    chatsApi.list() is called with no projectId, but chatsApi.list requires (projectId: string).
    useMutation({ mutationFn: chatsApi.create }) and then mutate({ title: 'New Chat' }) is incompatible because chatsApi.create requires (projectId, data). This should not compile under TypeScript strictness and will not work at runtime.

Maintainability:

    There is no “active project” selection or state, but the API structure is project-scoped. The UI must pick a project first or create a default project.

Starbot_WebGUI/src/components/chat/chat-view.tsx

Purpose: Chat main panel; sends message and shows message list; uses useChatStream.

Bugs (blockers):

    Mutation uses mutationFn: messagesApi.send, but the call uses sendMutation.mutate({ chatId, content, settings }) which does not match messagesApi.send(chatId, content, role?). This is a hard signature mismatch and likely breaks builds.
    Even if fixed, sending settings to the messages endpoint makes little sense; settings belong to /run, not to message creation.

Starbot_TUI
Starbot_TUI/Cargo.toml

Purpose: Rust crate metadata and dependencies; uses Rust edition 2024.

Concerns:

    Compatibility: edition 2024 requires an up-to-date toolchain; enforce a minimum Rust version in docs/CI.
    Security: Add routine cargo audit. RustSec describes cargo-audit and related tooling for vulnerability scanning.

Starbot_TUI/src/commands/auth.rs

Purpose: Device code authentication flow and token storage.

Bugs (blockers):

    Expects camelCase response fields (deviceCode, userCode, verificationUrl) but API returns snake_case (device_code, user_code, verification_url).
    Poll body uses deviceCode, but API expects device_code.
    Expects poll response accessToken/refreshToken but API returns access_token only (no refresh token in the current stub).

Security:

    Uses config file token persistence. Ensure file permissions are restricted (0600) on Unix-like systems.

Test gaps:

    No unit tests for device flow parsing and error branches.

Starbot_TUI/src/commands/chat.rs

Purpose: A CLI “chat” command posting to /v1/inference/chat.

Bugs (blockers):

    The API does not implement /v1/inference/chat. The API’s generation endpoint is /v1/chats/:chatId/run.
    Therefore, the CLI chat command cannot work against this API without either a compatibility endpoint or a CLI refactor.

Maintainability:

    The CLI mentions streaming but explicitly says it is not implemented; meanwhile the API is SSE-first. Align this.

Prioritized actionable fix plan with effort and patches
Critical fixes
Add missing OpenAI dependency in Starbot_API

Why it matters: Starbot_API/src/services/embeddings.ts imports openai. Without declaring it, your API build is broken or will break as soon as embeddings are reached.

Effort: 0.5–1 hour.

Patch sketch:

bash

cd Starbot_API
npm install openai

Also update Starbot_API/.env.example to include OPENAI_API_KEY and add it to env.ts for consistency (the code currently reads it directly from process.env).
Fix WebGUI compile-breakers by aligning signatures and project scoping

Why it matters: The current WebGUI calls the chat APIs without a projectId and calls messagesApi.send with the wrong argument type, which prevents the app from working.

Effort: 6–10 hours.

Concrete approach (recommended):

    Add “selectedProjectId” to useUIStore and require a project selection before listing chats.
    Update Sidebar to call chatsApi.list(selectedProjectId) and chatsApi.create(selectedProjectId, {title}).
    Update ChatView to call messagesApi.send(selectedChatId, content) (positional args), then call startStream(settings.mode) to generate assistant reply.

Patch sketch for sidebar.tsx:

ts

// before: chatsApi.list(), chatsApi.create with wrong args
const projectId = selectedProjectId;
const { data: chats } = useQuery({
  queryKey: ['chats', projectId],
  queryFn: () => projectId ? chatsApi.list(projectId) : Promise.resolve([]),
  enabled: !!projectId,
});

const createChatMutation = useMutation({
  mutationFn: (title: string) => chatsApi.create(projectId!, { title }),
  onSuccess: ...
});

const handleCreateChat = () => {
  if (!projectId) return;
  createChatMutation.mutate('New Chat');
};

Patch sketch for chat-view.tsx:

ts

const handleSend = async (content: string) => {
  if (!selectedChatId) return;
  await messagesApi.send(selectedChatId, content, 'user');
  await startStream(settings.mode); // extend startStream to accept full settings later
};

Fix TUI device auth casing and payloads

Why it matters: The TUI auth flow cannot succeed because it expects camelCase fields while the API returns snake_case.

Effort: 3–6 hours.

Patch sketch (TUI):

    Replace deviceCode → device_code
    Replace userCode → user_code
    Replace verificationUrl → verification_url
    Poll body should be { "device_code": "..." }
    Poll response should read access_token instead of accessToken

High priority fixes
Consolidate PrismaClient usage across API

Why it matters: You currently have both a shared Prisma client (db.ts) and several modules that create their own PrismaClient (workspaces.ts, memory.ts, retrieval.ts). Prisma warns that multiple PrismaClient instances can exhaust DB pools and reduce reliability; in traditional servers you should instantiate once and reuse.

Effort: 4–8 hours.

Patch sketch:

ts

// workspaces.ts / memory.ts / retrieval.ts
import { prisma } from '../db.js'; // adjust relative path as needed
// remove new PrismaClient() instances

Fix chat.updated correctness in generation route

Why it matters: The server updates the chat title in the DB but emits the old title in chat.updated. This breaks reactive UIs.

Effort: 1–2 hours.

Patch sketch:

    Compute newTitle before the update.
    Emit chat.updated using newTitle.

Remove or implement client “update” endpoints

Why it matters: WebGUI calls PUT /projects/:id, PUT /chats/:id, PUT /messages/:id, but the API does not implement them. These calls will always fail.

Effort: 4–10 hours depending on choice.

Recommended choice:

    Remove update calls from WebGUI until needed, or
    Implement minimal server endpoints:
        PUT /v1/projects/:id updates project name
        PUT /v1/chats/:id updates title
        Avoid message updates unless you have a strong need.

Medium priority fixes
Memory indexing performance and bulk writes

Why it matters: Memory processing currently deletes chunks and recreates them, writing each chunk in a loop. Prisma recommends bulk/batched operations for large writes.

Effort: 8–16 hours.

Implementation idea:

    Use createMany to insert chunks.
    Generate embeddings in batches (you already do), then update embeddings with updateMany or build full rows and createMany if you generate embeddings first.
    Consider storing embeddings in a vector DB or pgvector once scale matters.

Harden SSE parsing and add contract tests

Why it matters: Your server uses correct SSE framing, but your client parser assumes one JSON per data: line and doesn’t accumulate multi-line data: blocks. MDN notes multi-line semantics explicitly.

Effort: 6–12 hours including tests.
Low priority fixes

    Remove unused websocket plugin if you are not using it (0.5–1 hour).
    Improve docs alignment: IMPLEMENTATION_ROADMAP.md and specs should reflect what is actually implemented to avoid misguiding contributors (2–6 hours).

Tooling, tests, CI/CD, and deployment improvements
Start with static analysis and linting (exact commands)

I would start by running these exactly as written, because they will surface the current breakpoints immediately:

Starbot_API (Node/TS)

bash

cd Starbot_API
npm ci
npm run build
npx eslint .
npx prettier -c .
npm audit --audit-level=high

Starbot_WebGUI (Next.js)

bash

cd Starbot_WebGUI
npm ci
npm run build
npm run lint
tsc --noEmit

Starbot_TUI (Rust)

bash

cd Starbot_TUI
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit

Security scan rationale:

    RustSec documents cargo audit as the standard approach to audit Cargo.lock for known vulnerabilities.

Recommended linters/formatters/static analysis and configs

Starbot_API (TypeScript)

    ESLint with @typescript-eslint + rules:
        @typescript-eslint/no-floating-promises
        @typescript-eslint/consistent-type-imports
        @typescript-eslint/no-misused-promises
        no-console (warn; allow in CLI/dev)
    Prettier with consistent import order and trailing commas.
    Add engines in package.json or a root .nvmrc to enforce Node 20+ (Fastify v5 requires Node 20+).

Starbot_WebGUI

    Keep eslint-config-next, but add:
        eslint-plugin-jsx-a11y for accessible icon buttons
        eslint-plugin-react-hooks (if not already via Next config)

Also: Next.js 16 requires Node 20.9+. Enforce Node version in CI.

Starbot_TUI

    clippy with -D warnings already good.
    Add cargo deny (optional) for license/source policy (RustSec ecosystem).

Tests to add with example cases and targets

Targets (initial, pragmatic):

    API: 70% statements on routes/services
    WebGUI: 50% on stream reducer + API wrappers
    TUI: Focus on parsing/auth correctness

API tests (Vitest + Fastify inject or node:test)

    POST /v1/projects rejects empty name; creates valid project.
    POST /v1/projects/:projectId/chats returns 404 if project missing.
    POST /v1/chats/:chatId/run SSE test: verifies content-type text/event-stream, verifies events exist in order and end with message.final.
    Memory pipeline:
        PUT /v1/projects/:id/memory then POST /memory/process yields chunks count ≥ 1; no embeddings if OPENAI_API_KEY absent.

WebGUI tests (React Testing Library + MSW)

    Sidebar requires selected project before listing chats; verifies correct endpoint usage.
    Stream hook test: feed a simulated SSE response with token.delta and message.final and assert cache contains one assistant message with full concatenated content.

TUI tests

    Unit test JSON parsing for auth start and poll responses (snake_case).
    Endpoint selection test: ensure chat command uses existing API endpoints (after refactor).

CI/CD and deployment

I did not find any evidence of GitHub Actions workflows in the repository search results (no matches for typical workflow markers like actions/checkout). Given how quickly contract drift happened, CI is now a necessity.

I would add CI workflows that:

    Run Node builds for API and WebGUI with Node 20.9+ (Next.js 16) and Node 20+ (Fastify v5).
    Run Rust fmt/clippy/test and cargo audit.
    Run npm audit --audit-level=high for both Node projects.

Docker:

    There is no Dockerfile in the codebase results I reviewed (the word “Dockerfile” only appears in docs/spec discussions).
    I would add:
    A minimal API Dockerfile (Node 20 base, non-root user).
    A docker-compose file for local orchestration (API + WebGUI + persistent volume for SQLite, or migrate to Postgres for production).

Risk assessment, rollback plan, and metrics
Risk assessment

High risk changes:

    API contract changes (adding/removing endpoints, renaming SSE events, changing JSON casing). This is already your biggest failure mode (clients are broken today). The safest plan is to establish one canonical contract (OpenAPI) and generate client types from it.
    Auth enforcement: When you start enforcing tokens, you can lock out the WebGUI/TUI if they are not updated.

Medium risk changes:

    Memory retrieval refactor (bulk writes, vector DB migration). This can change result ordering and relevance and can introduce migration issues.

Low risk changes:

    Prisma client consolidation: should improve reliability if done carefully.
    CI introduction: high benefit, low operational risk.

Rollback plan for major changes

I would implement rollback in layers:

    Feature flags:
        ENABLE_AUTH=false to allow reverting auth enforcement quickly.
        ENABLE_VECTOR_SEARCH=false to keep current scan-based retrieval as fallback.
    DB migration safety:
        Before Prisma migrations, snapshot the SQLite DB file.
        Use migration naming and “down” strategy or keep rollout reversible by feature flag.
    Git rollback:
        Maintain a “known good” tag (e.g., v0.1.0-local-dev) for quick reversion.
        For contract changes, release versioned API (/v1 stable, /v2 experimental) so rollback is path-based.

Current vs recommended key metrics table
Metric	Current state (observed)	Recommended state	Measurement method
Build integrity	WebGUI and TUI have signature/endpoint mismatches; API embeds OpenAI SDK without dependency	All three components build in CI on clean install	CI “build” jobs + npm ci && npm run build, cargo test
Security posture	Auth is stub; no enforcement; no CI scanning	Auth enforcement (flagged), rate limits, CodeQL, npm audit, cargo audit	CI security jobs; periodic scheduled scans
Test coverage	No wired test suite in Node projects; minimal/no Rust tests	API ≥70%; WebGUI ≥50%; TUI targeted tests	Coverage reports (c8/istanbul, jest/vitest, cargo coverage tooling optional)
Performance	Memory search is O(n) scan + JSON parse; chunk inserts are per-row	Bulk DB operations + scalable retrieval strategy	Benchmarks and p95 latency of /run + memory endpoints
Dependency hygiene	Multiple lockfiles in API; Node versions not enforced	Single package manager per workspace; Node/Rust min versions pinned	CI check + repo policy
Architecture diagram

mermaid

flowchart LR
  subgraph Clients
    W[Starbot_WebGUI<br/>Next.js 16] -->|REST + SSE| A
    T[Starbot_TUI<br/>Rust] -->|REST (auth, chat)| A
  end

  subgraph API["Starbot_API (Fastify v5)"]
    A[HTTP API /v1] --> G[POST /chats/:chatId/run<br/>SSE stream]
    A --> P[Projects / Chats / Messages]
    A --> M[Memory docs + chunks<br/>Embeddings + retrieval]
    A --> Auth[Device auth (stub)]
    P --> DB[(SQLite via Prisma)]
    M --> DB
    G --> Providers[Provider adapters<br/>(Azure, Vertex, Bedrock, etc.)]
  end
