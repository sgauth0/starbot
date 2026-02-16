
Starbot Code Review and Improvement Plan
Executive summary

I reviewed the current sgauth0/starbot codebase using the only enabled connector (github) and prioritized the production-critical paths: API streaming generation, WebGUI chat UX and SSE parsing, TUI auth/chat correctness, and deployment/CI hardening. The most important finding is that the system’s “core loop” is close to working end-to-end, but it is held back by contract drift (spec vs implementation vs clients) and a few high-impact integration bugs—especially in the WebGUI send→run flow and schema validation mismatches.

The most urgent functional issues I found:

    The WebGUI sends user messages but does not trigger generation (/v1/chats/:chatId/run), so the assistant never replies unless another UI path calls startStream().
    The WebGUI enforces a ProjectSchema that requires updatedAt, but the Prisma Project model has no updatedAt, causing runtime response-validation failures.
    The deployment configs run both services as root and nginx injects wildcard CORS for the API as a “backup,” which is unsafe for internet exposure (and can conflict with credentialed CORS).

The strongest positives:

    The API generation pipeline already has the key “agent platform” primitives: triage-based routing, model catalog selection, memory chunking + embeddings + retrieval, and SSE token streaming with explicit event names.
    A deployment quickstart + systemd/nginx scaffolding exists and is coherent enough to productionize once hardened (non-root users, CORS cleanup, reproducible builds).

Repository snapshot and architecture
Enabled connectors

Enabled connectors used: github.
Notable recent commits I referenced

From the repository’s recent commit history (latest first), the project stabilized quickly via a monorepo split and deployment hardening, culminating in “closer to final prod maybe?????”. These commits matter because they indicate active churn in API contracts and deployment assumptions:

    43a8203 — “closdr to final prod maybe?????”
    0fd7fb7 — “separated everything ig and improved memory”
    0ad582c — “split monorepo into 3…”
    84209f5 — “Add Starbot_WebGUI source files…”
    fb009df — “Initial commit: Starbot multi-component project”

Current architecture

Starbot is structured as three sibling apps:

    Starbot_API: a Fastify v5 TypeScript server with Prisma/SQLite, supporting:
        chat/message/project CRUD
        memory documents → chunking → embeddings and semantic retrieval
        triage-based model selection and SSE streaming generation
    Starbot_WebGUI: a Next.js 16 app using react-query + zustand, acting as the primary user interface.
    Starbot_TUI: a Rust CLI/TUI that currently uses a “legacy inference” endpoint rather than native SSE streaming in the terminal.

Mermaid architecture diagram:

mermaid

flowchart LR
  subgraph Clients
    W[Starbot_WebGUI\nNext.js] -->|REST + fetch(SSE)| A
    T[Starbot_TUI\nRust CLI/TUI] -->|REST| A
  end

  subgraph API["Starbot_API\nFastify + Prisma (SQLite)"]
    A[HTTP /v1] --> CRUD[Projects/Chats/Messages]
    A --> MEM[Memory docs\nchunking + embeddings + retrieval]
    A --> G[POST /chats/:chatId/run\nSSE stream]
    A --> AUTH[Device auth\nstub]
    A --> INF[POST /inference/chat\nlegacy]
    CRUD --> DB[(SQLite)]
    MEM --> DB
    G --> LLM[Provider adapters\nmodel-catalog routing]
  end

  subgraph Edge["Nginx + systemd"]
    EdgeProxy[Nginx]\n--> A
    EdgeProxy --> W
    systemd[systemd services]\n--> A
    systemd --> W
  end

Static analysis and quality gates I run first

I start by running your requested commands (these are the fastest way to surface contract drift, type errors, and dependency issues).
Starbot_API

bash

cd Starbot_API
npm ci
npm run build
npx eslint .
npx prettier -c .
npm audit --audit-level=high

Important: CI currently runs npm ci (good), but the workflow masks lint/test failures with || echo ... (bad), which undermines “green means safe.”
Starbot_WebGUI

bash

cd Starbot_WebGUI
npm ci
npm run build
npm run lint
tsc --noEmit

WebGUI’s Node floor must remain Node 20.9+ for Next.js 16.
Starbot_TUI

bash

cd Starbot_TUI
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit

TUI CI currently “passes” even if cargo audit is missing, because it falls back to || echo and is continue-on-error. That is fine as an informational check, but not as a security gate.
File-by-file review

I focus on the main source files, contracts, CI, and deployment assets. For each file below I summarize: purpose, key constructs, style, bugs, security, performance, dependency concerns, tests, docs, and maintainability.
Starbot_API
Starbot_API/src/index.ts

Purpose: API server bootstrap: env loading, logging, CORS, route wiring, /v1/health.

Key constructs:

    Registers all route modules under /v1 (projects, chats, messages, generation, models, workspaces, memory, auth, inference).

Coding style:

    Mostly consistent, but uses both console.log and Fastify’s logger; I would standardize on structured logging and avoid console.log for production.

Bugs:

    None obvious within bootstrap, but CORS + nginx “backup CORS” can create conflicting behavior.

Security:

    CORS is allowlisted (good), but nginx injects wildcard CORS for /v1/ (bad). These can conflict, especially with credentialed requests.

Tests:

    No “boot smoke test” verifies route registration and health endpoint in CI.

Maintainability:

    Good modular routing structure. I recommend adding OpenAPI emission and deriving client schemas from it (see contract plan).

Starbot_API/package.json

Purpose: defines scripts, Node floor, deps, dev tooling.

Key constructs:

    test:coverage exists (Vitest + coverage).
    Node engine is >=20.0.0 (aligned with Fastify v5).

Issues:

    You still include @fastify/websocket even though the stack is SSE-based and index.ts does not register websockets. This is dependency bloat/attack surface—remove unless planned.

Starbot_API/prisma/schema.prisma

Purpose: defines SQLite schema for Projects, Chats, Messages, Workspaces, Memory docs/chunks.

Key constructs:

    Project has id, name, createdAt—no updatedAt.
    Chat has updatedAt @updatedAt.
    MemoryChunk.embeddingVector is a JSON-string of floats (nullable).

Bugs / contract drift:

    Prisma datasource URL is hard-coded: file:../starbot.db, while deployment docs instruct setting DATABASE_URL=file:../starbot.db. Because the schema does not use env("DATABASE_URL"), the env var is effectively ignored by Prisma runtime configuration.

Performance:

    Storing embeddings as JSON is acceptable initially, but semantic search scales poorly because retrieval must parse JSON for every chunk. This will become noticeable as memory grows (see retrieval service).

Starbot_API/src/routes/generation.ts

Purpose: primary streaming generation endpoint POST /v1/chats/:chatId/run; runs triage, retrieves memory, selects model/provider, streams tokens via SSE, persists final assistant message.

Key constructs:

    RunChatSchema supports mode, model_prefs, speed, auto.
    parseModelPrefs supports provider:model style selection and providers-only selection.
    SSE event types emitted: status, token.delta, message.final, chat.updated, plus error.

Bugs / correctness:

    speed and auto are parsed but not actually used to alter behavior—this is a contract smell (either implement or remove).
    Messages are cast to provider role union (m.role as 'user'|'assistant'|'system'). Your DB allows tool, so this can leak invalid roles into providers if tooling is added later.

Security:

    There is no authentication/authorization check before model calls. With public ingress, this is a high-cost abuse vector.

Performance:

    Retrieval runs before generation and scales with chunk count; currently OK for small data.
    The route uses a single stream loop and writes directly to reply.raw (fine).

Tests:

    Missing contract tests for SSE sequencing and payload shape.

Documentation gap:

    Spec claims Authorization: Bearer … and different SSE payload fields (delta, message_id, etc.). The real implementation uses data: { text } for token deltas and data: { id } for the final message. This mismatch breaks strict clients.

Starbot_API/src/routes/inference.ts

Purpose: legacy /v1/inference/chat endpoint for the TUI; wraps chat persistence and returns a single JSON response.

Key constructs:

    Always runs triage and selects model using getBestModelForTier.
    Accepts optional provider and model fields but does not actually use them.

Bugs / UX mismatch:

    The TUI supports --model and sets provider/model fields, but they’re ignored by this endpoint, so users will believe selection works when it does not.

Security:

    Same as generation: no auth enforcement.

Maintainability:

    This endpoint will keep drifting unless it shares selection logic with generation.

Starbot_API/src/routes/messages.ts

Purpose: list/create messages for a chat.

Key constructs:

    Accepts role user|assistant|tool|system. Sends { message } on create.

Bugs / client mismatch:

    There is no PUT /v1/messages/:id route, but WebGUI calls PUT /messages/:id.

Security:

    Allowing browser clients to create assistant or tool messages without auth/signature enables conversation forgery. I would constrain message creation to role=user for the public WebGUI path.

Tests:

    No tests for message role restrictions or chat-not-found behavior.

Starbot_API/src/routes/memory.ts

Purpose: manages project/workspace memory documents, processes content into chunks, generates embeddings, bulk inserts chunks, and supports memory search.

Key constructs:

    Default memory templates (PMEMORY and MEMORY).
    /memory/process uses createMany (good bulk write).

Bugs:

    Minimal validation of request body shapes (e.g., content is assumed string). This is OK early but should become Zod-validated like other routes.

Performance:

    Bulk inserts are good. Prisma recommends bulk writes for large operations.

Tests:

    Missing tests for processing and search endpoints.

Documentation:

    Memory endpoints in spec use snake_case fields (updated_at) while the implementation returns updatedAt as a Date object (JSON serialized).

Starbot_API/src/services/embeddings.ts

Purpose: generates embeddings using the OpenAI Node SDK; batches requests; includes “availability” checks based on env presence.

Key constructs:

    Hard-codes text-embedding-3-large and uses encoding_format: 'float'.
    Caches the client instance.

Issues:

    Model name is hard-coded; make it configurable via env to allow future migrations without code redeploy.
    Error handling is console-based; prefer Fastify logger wiring.

Starbot_API/src/services/retrieval.ts

Purpose: cosine-similarity semantic search over stored embeddings.

Key constructs:

    searchMemory loads matching memory documents and includes chunks; parses embeddingVector JSON and compares vectors.

Performance bottleneck:

    O(N) scanning of all chunk vectors and repeated JSON parsing will become a hotspot as memory grows.

Maintainability:

    Logs errors to console; prefer structured logging.
    Possible failure mode: if embeddings dimension mismatches ever occur, you throw at cosineSimilarity; consider smoothing by skipping mismatched vectors to avoid breaking all retrieval.

Starbot_WebGUI
Starbot_WebGUI/package.json

Purpose: defines Next.js 16 runtime, scripts, and deps; sets Node engine floor to >=20.9.0.

Dependency/version concerns:

    Next.js 16 requires Node 20.9+; your engine constraints match official Next.js docs.

Starbot_WebGUI/src/lib/types.ts

Purpose: Zod schemas and TS types for API resources + settings.

Key constructs:

    ProjectSchema requires updatedAt.
    ChatSchema.projectId is optional.
    SettingsSchema matches API modes and includes auto, speed, model_prefs.

Bugs:

    ProjectSchema mismatch with DB/API: Prisma Project has no updatedAt. This will break projectsApi.list() validation.
    Likely mismatch: API returns projectId required in chats, but client allows optional.

Maintainability:

    This Zod file is effectively your client contract. It must be derived from a canonical OpenAPI schema, or it will drift again.

Starbot_WebGUI/src/hooks/use-chat-stream.ts

Purpose: fetch-based SSE client for /v1/chats/:chatId/run; updates react-query cache for streaming messages.

Key constructs:

    startStream(mode) posts { mode } and reads text/event-stream.
    Parses event: and data: lines and dispatches to handleSSEEvent().

Bugs:

    It only posts { mode } and does not send auto, speed, model_prefs, so UI settings won’t affect the backend even though the API supports them.
    If old cache is null, token events are dropped (if (!old) return []). This can happen if the SSE starts before message query hydration completes.
    SSE parsing assumes each data: line is a full JSON object; the SSE spec allows multiple consecutive data: lines per event. MDN documents that clients should concatenate them.

Starbot_WebGUI/src/components/chat/chat-view.tsx

Purpose: main chat view: loads messages via useChatStream, sends user message, shows message list and input.

Key constructs:

    Uses messagesApi.send(); includes optimistic “temp-user” message insertion.

Critical bug:

    Does not call startStream() after sending a user message. This is a “no assistant replies” severity issue.

Starbot_WebGUI/src/lib/api/messages.ts

Purpose: WebGUI wrapper for sending and updating messages.

Bug:

    Calls PUT /messages/:id, which is not implemented by the API. Should be removed or API should implement it intentionally.

Starbot_WebGUI/src/lib/api/chats.ts and projects.ts

Purpose: wrappers for list/get/create/update/delete of projects and chats.

Risk:

    If ProjectSchema validation fails, the entire app’s project loading fails, which then cascades to sidebar and chat list.

Starbot_WebGUI/src/components/sidebar.tsx

Purpose: shows chat list, creates “New Chat,” toggles settings.

Bug:

    Assumes projects?.[0] exists but does not actually create a default project. If there are no projects, “New Chat” will throw “No project selected” and the app has no obvious recovery.

Starbot_TUI
Starbot_TUI/src/commands/auth.rs

Purpose: device code flow auth; stores token in profile.

Correctness:

    The TUI expects snake_case fields (device_code, user_code, verification_url) which matches the API auth implementation.

Security:

    Ensure the config file is written with restrictive permissions; the deployment docs do chmod 600 for .env, but TUI’s token file constraints must be validated separately.

Starbot_TUI/src/commands/chat.rs

Purpose: posts a user prompt to /v1/inference/chat.

Bugs / product gap:

    CLI --stream is not implemented (“falls back to non-streaming”), which prevents parity with modern code agent CLIs that stream tokens and apply patches.
    Provides provider/model selectors; API inference ignores them.

CI/CD and deployment assets
.github/workflows/api.yml

Purpose: API CI build/lint/test + CodeQL.

Critical issue:

    Lint and tests are masked via || echo ..., so regressions do not fail CI.

Security improvement:

    Prefer action pinning, least-privilege token permissions, and explicit audit policies.

.github/workflows/webgui.yml

Purpose: WebGUI CI build/lint/typecheck.

Notes:

    Enforces Node 20.9.x in the matrix (good) given Next.js 16 requirements.

.github/workflows/tui.yml

Purpose: multi-OS Rust fmt/clippy/test/build plus an “audit” step.

Issue:

    cargo audit may not exist in the toolchain and is treated as non-blocking; if you want meaningful RustSec scanning, install cargo-audit explicitly and decide what blocks merges.

deploy/nginx-starbot.cloud.conf

Purpose: nginx reverse proxy for /v1/ to API and / to WebGUI.

Security problems:

    Adds Access-Control-Allow-Origin: * for /v1/ as “backup”. This is not acceptable for credentialed APIs and undermines the server’s strict CORS allowlist.
    Contains “WebSocket support for SSE” upgrade headers; SSE does not need WebSocket upgrade semantics.

Correctness:

    proxy_buffering off for streamed endpoints is appropriate; SSE can break with buffering.

deploy/starbot-api.service and deploy/starbot-webgui.service

Purpose: systemd services for API and WebGUI.

Critical security issues:

    Both run as User=root. Switch to a dedicated starbot user and add stronger sandboxing flags.

deploy/deploy.sh and deploy/QUICKSTART.md

Purpose: build + deploy orchestration, including prisma db push and systemd/nginx installation.

Reliability issue:

    Uses npm install instead of npm ci, which can yield non-reproducible builds and drift from lockfiles.

Docs mismatch:

    Quickstart suggests setting DATABASE_URL, but Prisma schema hard-codes db URL. Align these so docs match behavior.

specs/DR_APICONTRACT.md and FIXES_COMPLETED.md

Purpose: defines intended API contract and claims “all issues fixed.”

Major concern:

    DR_APICONTRACT.md does not match current implementation in auth headers, field casing, and SSE shapes. This should be treated as stale until updated.
    FIXES_COMPLETED.md is helpful context, but it also contains claims that do not fully reflect the current repo state (e.g., dependency removals). Use it as a checklist, not as ground truth.

Prioritized actionable fixes with effort estimates
Critical

Fix WebGUI send→run integration (6–10 hours)
Goal: after a user sends a message, immediately start generation streaming.

Patch sketch:

diff

diff --git a/Starbot_WebGUI/src/components/chat/chat-view.tsx b/Starbot_WebGUI/src/components/chat/chat-view.tsx
@@
 export function ChatView() {
   const { selectedChatId, settings } = useUIStore();
-  const { messages, isLoading, status } = useChatStream(selectedChatId);
+  const { messages, isLoading, status, startStream } = useChatStream(selectedChatId);

@@
   const handleSend = (content: string) => {
     if (selectedChatId) {
-      sendMutation.mutate(content);
+      sendMutation.mutate(content, {
+        onSuccess: async () => {
+          // Start generation immediately after user message is accepted
+          await startStream(settings.mode);
+        },
+      });
     }
   };

Also upgrade useChatStream.startStream() to accept full settings and forward auto, speed, model_prefs, not just mode.

Fix WebGUI ProjectSchema and ChatSchema to match API (2–5 hours)
Option A (fastest): make updatedAt optional in ProjectSchema, and require projectId on ChatSchema if API always returns it.

diff

 export const ProjectSchema = z.object({
   id: z.string(),
   name: z.string(),
   description: z.string().optional(),
   createdAt: z.string(),
-  updatedAt: z.string(),
+  updatedAt: z.string().optional(),
 });

This prevents runtime parse failures.

Remove or implement PUT /messages/:id (2–6 hours)
Right now it’s dead code. Either:

    remove messagesApi.update() from WebGUI, or
    implement an API endpoint intentionally and restrict what can be updated (and by whom).

Harden deployment: eliminate root services and wildcard CORS (6–14 hours)

    Create a starbot system user.
    Change systemd services to run as User=starbot, Group=starbot.
    Remove Access-Control-Allow-Origin: * and other proxy-level CORS “backup” headers from nginx; keep CORS in the API only.

High

Make API CI meaningful (2–4 hours)
Change the workflow to fail when lint/tests fail:

diff

- run: npm run lint || echo "No lint script found"
+ run: npm run lint

- run: npm test || echo "No test script found"
+ run: npm test

Also decide whether npm audit --audit-level=high should block merges for prod branches.

Fix SSE parsing robustness in WebGUI (6–12 hours)
Your parsing assumes a single-line JSON after data:; SSE permits multiple data: lines that must be concatenated.
Implement an event buffer that collects data: lines until the blank-line terminator, then parse once.

Align Prisma datasource with deploy docs (2–4 hours)
Change Prisma schema to:

prisma

datasource db {
  provider = "sqlite"
  url      = env("DATABASE_URL")
}

Update deployment docs accordingly.

Honor CLI provider/model selection in inference endpoint (4–10 hours)
Refactor model selection logic so inference.ts can:

    use provider/model prefs when explicitly provided, or
    default to best tier when not.

Medium

Add auth enforcement + rate limiting on expensive endpoints (12–24 hours)
At minimum enforce auth on:

    POST /v1/chats/:chatId/run
    POST /v1/inference/chat
    memory processing endpoints if embeddings are enabled

Your current auth route is explicitly a stub (“use Redis in production”). Treat this as a feature-flagged rollout.

Make /auth/device/* production-capable (12–30 hours)

    Replace in-memory storage with Redis.
    Make verification_url environment-aware (not hard-coded to localhost).
    Add expiry cleanup (delete expired entries, not just mark).

Retrieval performance path (16–40 hours staged)

    Short-term: cache parsed embeddings in memory keyed by chunk id + updatedAt.
    Long-term: migrate to a proper vector index (Postgres + pgvector or dedicated vector DB).

Low

Reproducible deployments (3–8 hours)

    Change deploy script to use npm ci instead of npm install.
    Add an explicit build artifact strategy and/or containerization later.

Remove unused dependencies (1–3 hours)

    Remove @fastify/websocket from API dependencies if truly unused.

Testing strategy, tools, and CI/CD improvements
Current testing reality

    API has a Vitest setup with coverage reporters configured (text/json/html).
    There is route-level testing for projects using Fastify inject(), but it hits the real Prisma client and therefore needs explicit test DB isolation.

Tests I recommend adding

API unit and integration (target: ≥70% statements in 2–4 weeks)

    SSE contract test for /v1/chats/:chatId/run
        Assert event order includes at least: status, token.delta (multiple), message.final, chat.updated
        Assert final message persists in DB
    Negative tests:
        chat not found → 404
        no user message found → structured error event
    Auth tests:
        device start returns required fields
        poll transitions pending → authorized

WebGUI unit and integration (target: ≥50% for critical UI logic in 4–6 weeks)

    Unit test SSE parser on multi-line data: events (required by spec).
    Integration test: send message → stream kicks off → assistant message appears.
    E2E smoke test (Playwright):
        create project (or auto-create)
        create chat
        send message
        observe streaming output

TUI tests

    parsing tests for auth start/poll responses
    chat command request builder tests (selector parsing)
    future: streaming parser tests once implemented

Linters, formatters, static analysis configuration

Node/TypeScript

    Add a root-level Prettier config and pin Prettier in devDependencies instead of relying on npx prettier -c . resolving a potentially newer version.
    Strengthen TypeScript lint rules:
        @typescript-eslint/no-floating-promises
        @typescript-eslint/no-misused-promises
        @typescript-eslint/consistent-type-imports

WebGUI

    Next.js 16 removed automatic next lint behavior in build workflows; you already call ESLint directly, which matches Next’s current guidance.
    Add MSW to test API wrappers and streaming in React Query without hitting the real server.

Rust

    Keep fmt/clippy/-D warnings.
    Make cargo-audit meaningful by explicitly installing it in CI (or using a standard action) and deciding whether it blocks.

GitHub Actions security best practices

    Minimize token permissions for jobs and avoid granting more than needed. GitHub recommends least-privilege usage for GITHUB_TOKEN.

Risk assessment, rollout plan, and rollback strategy
Current vs recommended state metrics
Metric	Current state	Recommended state	Measurement
Assistant reply reliability	WebGUI send does not start generation	Always: send → run stream	E2E: “send message → assistant response in <5s”
Contract stability	Spec, API, and clients drift	Single source of truth (OpenAPI) + generated clients	Contract tests + CI
Auth & abuse protection	No enforced auth on costly endpoints	Auth + rate limits + feature flags	Load tests + abuse simulations
Deployment hardening	systemd runs as root; nginx wildcard CORS	non-root service user; strict CORS; hardened systemd	Security checklist + config review
Test coverage	API tests exist but DB isolation unclear	API ≥70% + WebGUI ≥50% on critical logic	Coverage reports in CI
Rollout timeline

2026-02-15
2026-02-17
2026-02-19
2026-02-21
2026-02-23
2026-02-25
2026-02-27
2026-03-01
2026-03-03
WebGUI send triggers /run + settings forwarding Fix ProjectSchema and remove dead endpoints     Nginx + systemd hardening (no root, no wildcard)CI gates made strict                           SSE parser robustness (multi-line data)        Prisma DATABASE_URL alignment                  Auth enforcement + rate limiting               SSE contract tests + E2E harness               CriticalHighMediumStarbot stabilization roadmap

Rollback plan for major changes

Auth and contract changes are the highest risk because they can brick both clients instantly. My recommended rollback approach:

    Feature flags in API:
        AUTH_REQUIRED=false initially; enable on staging first, then production.
    Release directory structure on server:
        Deploy to /opt/starbot/releases/<git_sha>/...
        systemd points to a stable symlink /opt/starbot/current
        rollback is atomic: repoint symlink + restart services
    DB backup before schema changes:
        SQLite: snapshot starbot.db before prisma migrate or schema alterations.
    Nginx config rollback:
        Always nginx -t before reload; keep last-known-good config available.

This gives you deterministic rollback without needing to “undo” code changes under pressure.
