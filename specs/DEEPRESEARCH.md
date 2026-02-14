
Deep research on sgauth0/starbot
Executive summary

Enabled connectors used (as requested): github.

This repo is a multi-component system with a TypeScript/Fastify API (Starbot_API), a Next.js Web UI (Starbot_WebGUI), and a Rust TUI/CLI (Starbot_TUI).

The API is internally coherent around /v1 routes and Server-Sent Events (SSE) generation via POST /v1/chats/:chatId/run, and it even ships a local harness exercising that streaming contract.

However, the WebGUI and parts of the Rust CLI are currently contract-incompatible with the API: wrong base URL and route paths, wrong response shapes, wrong streaming method (native EventSource GET) and event names, and mismatched “mode/speed/auto” settings.

The TUI supports a QR/device-code login flow and a legacy chat endpoint (/v1/inference/chat) that the API does not implement, so TUI “dev scripts” and certain CLI commands will fail against the current API.

Your intended product semantics (workspace↔repo mapping; shared project memory via PMEMORY.md; repo/thread memory via MEMORY.md; cross-thread project search; QR linking TUI↔WebGUI↔API) are not implemented end-to-end yet. The API schema only models Project/Chat/Message/Event, and the TUI contains placeholder “memory/tools/workspaces” compatibility stubs.
Repository architecture and local toolchains
What’s in the repo

From inspected paths and code-search hits, the core layout is:

    Starbot_API: Fastify 5 + TypeScript + Prisma (SQLite), SSE generation, model catalog/provider routing.
    Starbot_WebGUI: Next.js + React Query + Zustand + Zod.
    Starbot_TUI: Rust (edition 2024) TUI/CLI using reqwest streaming and an SSE parser.

Clone/build commands to run locally

Network/exec may be unavailable in some environments, so these commands are intended for your local machine.

Repo:

bash

git clone https://github.com/sgauth0/starbot.git
cd starbot
git checkout main

API (TypeScript):

bash

cd Starbot_API
npm ci
npm run db:generate
npm run build
npm run dev
./test-api.sh   # requires curl + jq

The presence of test-api.sh is important: it asserts the “canonical” SSE contract (token.delta, message.final, etc.).

WebGUI (Next.js):

bash

cd Starbot_WebGUI
npm ci
npm run lint
npm run build
npm run dev

WebGUI includes eslint and a lint script.

Rust TUI/CLI:

bash

cd Starbot_TUI
cargo fmt --check
cargo clippy -- -D warnings
cargo test
./scripts/starbott-dev.sh tui   # expects API reachable + auth behavior

The crate is starbot-tui and ships Cargo.lock, improving reproducibility.
WebGUI audit and LLM-refactor risk assessment

This is the most urgent area: the WebGUI appears to have been refactored with incomplete contract grounding, leaving multiple “LLM tells” (commentary about assumptions, temporary IDs, fallback token-in-query, and missing invariants).
High-confidence contract mismatches

Wrong default API base URL
Starbot_WebGUI/src/lib/config.ts defaults to http://localhost:3000/api (line 1), which points at the Next dev server—not the Fastify API on 3737 with /v1.
Impact: nothing works by default.

Wrong route shapes and missing endpoints
The WebGUI client calls endpoints that don’t exist in the API:

    projectsApi.list() calls GET /projects (projects.ts line 6), expects a Project[].
    The API provides GET /v1/projects returning { projects: [...] } (and no update endpoint).

    chatsApi.list() calls GET /chats?projectId=... (chats.ts line 8).
    The API provides GET /v1/projects/:projectId/chats returning { chats: [...] }.

    messagesApi.send() calls POST /messages (messages.ts line 5).
    The API provides POST /v1/chats/:chatId/messages returning { message: ... }.

Streaming implementation is incompatible with the API
WebGUI uses native EventSource and attempts GET ${API_BASE_URL}/chats/${chatId}/stream (use-chat-stream.ts line 21) and listens for assistant.delta / assistant.final (lines 34 and 58).
The API streams via POST /v1/chats/:chatId/run and emits events like token.delta and message.final.

This is a fundamental mismatch because SSE in browsers via EventSource is a unidirectional, long-lived connection to a URL that delivers text/event-stream data. Native EventSource also has no facility to send a POST body or custom headers (its constructor accepts only a URL and limited options such as withCredentials).
LLM-introduced code smells and UI-state bugs

Token in query string fallback (security + caching hazard)
The streaming hook appends token to query params as a fallback (use-chat-stream.ts line 23), explicitly acknowledging header limitations.
Query-string tokens commonly leak via logs, browser history, and referrer headers; this should never ship beyond local dev.

Incorrect and brittle optimistic streaming merge
The stream hook treats missing cache as “return []” (use-chat-stream.ts line 37), which can drop the first assistant delta entirely when no prior messages are present. This is consistent with LLM output that optimizes for “compile” rather than correctness.

Mismatch between Settings UI and API schema
WebGUI’s Settings schema uses mode: standard|thorough|deep and speed: fast|quality (types.ts line 29), and SettingsPanel renders thorough (settings-panel.tsx line 51).
API expects mode: quick|standard|deep and speed: boolean (fast mode).
Result: even if routing and streaming were fixed, “thorough” requests would fail validation.

Type contract mismatch guarantees Zod failures
ProjectSchema requires updatedAt (types.ts line 8), but the API’s Project model has no updatedAt.
WebGUI’s client throws “Response validation failed” when schemas don’t match.
Accessibility issues

Several icon-only buttons have no explicit accessible name:

    Menu toggle in page.tsx uses an icon button without aria-label (line ~29).
    Send button in chat-input.tsx is icon-only (line ~40).
    Close button in settings uses only an X icon (settings-panel.tsx line ~28).

These are easy fixes but matter for keyboard/screen-reader usability.
API audit: routes, SSE contract, security posture, data model gaps
Route surface and SSE

The API registers route modules under /v1 and listens on 127.0.0.1:3737.

The generation route implements SSE by writing event: / data: blocks and setting Content-Type: text/event-stream. This matches the SSE framing described by MDN (messages separated by blank lines; event and data fields).

The test harness demonstrates the expected stream event types and payload fields (token.delta uses .text).
Runtime correctness issues in the API

Stale chat.updated title sent to clients
In generation.ts, after updating the chat title in the DB, the server emits chat.updated with title: chat.title (pre-update value). This occurs at approximately lines 216 and 243 in the fetched file.
Impact: UIs relying on chat.updated will never see the new title.

tool messages can break provider role assumptions
messages.ts accepts role: tool (line ~7).
generation.ts maps DB messages into provider messages by casting role to 'user'|'assistant'|'system' (lines ~178–179), without filtering/remapping tool.
Impact: If any tool message exists in the last 50 messages, providers may receive an invalid role, causing failures or undefined behavior.

Cancellation endpoint is a stub
The API exposes POST /v1/chats/:chatId/cancel but returns “Cancellation not yet implemented” (near the end of generation.ts).
Impact: TUI’s cancel button will appear to work but won’t stop token generation.
Security-relevant defaults

CORS blocks WebGUI by default
API allows only origins on port 8080 (http://localhost:8080, http://127.0.0.1:8080).
If WebGUI runs on the usual 3000, browser requests will fail preflight unless you update CORS.

Credentials loaded via env (good), but .env.example content couldn’t be inspected
The API reads provider credentials from environment variables and logs configuration with “redact secrets” intent.
An .env.example file exists but was blocked from direct fetch in this environment; you should still ensure it contains no real secrets.
Data model vs intended “workspace + memory files” design

Prisma schema includes only: Project, Chat, Message, Event.
There is no Workspace, no repo link, no thread grouping independent of projects, and no memory artifacts (MEMORY.md / PMEMORY.md) represented.

This is the largest conceptual gap between repo code and the product design you described.
Rust TUI/CLI audit and QR device-link flow consistency
What aligns well with the API today

The TUI’s async handlers call the API’s real endpoints (/v1/projects, /v1/projects/:id/chats, /v1/chats/:id/messages, /v1/chats/:id/run) and parse the same stream events (status, token.delta, message.final, error).

The Rust SSE parser correctly implements “event/data + blank line terminator” framing (including multi-line data: accumulation), consistent with MDN’s specification.
What is incompatible or incomplete

Device QR auth flow expects endpoints the API does not implement
The CLI implements a device-code flow (POST /v1/auth/device/start, POST /v1/auth/device/poll) and displays a QR.
starbott-dev.sh will automatically attempt this device login if whoami fails.
The current API registers only projects/chats/messages/generation/models routes; there is no /v1/auth/*.
Impact: “happy path” dev script onboarding fails.

Legacy CLI chat command calls /v1/inference/chat
The CLI chat command posts to /v1/inference/chat (chat.rs line ~60), which does not exist in the API.
smoke.sh invokes starbott chat "hi", so your smoke test is currently expected to fail unless another backend exists.

Inconsistent “canonical port”

    TUI config default is http://localhost:3737.
    Dev script defaults to http://127.0.0.1:3003.
    Impact: contributors will constantly point at the wrong backend.

Naming / repo hygiene signals
The Cargo package is starbot-tui, but Clap declares the executable name as starbott, and config directory is ~/.config/starbott.
There is also a tui.rs.backup file in-tree, which is a common refactor artifact and should be removed or moved to git history.
QR device-link flow diagram

Your intended “TUI links to WebGUI + API via QR” fits well structurally with a device-code pattern:

Starbot_WebGUIStarbot_APIStarbot_TUIStarbot_WebGUIStarbot_APIStarbot_TUIRender QR for verificationUrl?code=userCodeloop[poll until authorized/expired]POST /v1/auth/device/startdeviceCode,userCode,verificationUrl,intervalPOST /v1/auth/device/confirm { userCode }200 OKPOST /v1/auth/device/poll { deviceCode }pending|authorized (+accessToken,+refreshToken)

If you implement this in the API, strongly prefer standard Authorization: Bearer (Rust already uses it) rather than custom headers or query-string tokens.
CI/CD, tests, Docker/dev scripts, and reproducibility
Tests and harnesses present

    API has a pragmatic end-to-end bash harness (test-api.sh) that exercises health/projects/chats/messages and the POST-SSE generation stream.
    TUI has scripts/smoke.sh and a dev wrapper starbott-dev.sh, but they assume legacy endpoints and a different default port.

CI/CD and Docker

I did not encounter any GitHub Actions workflows (.github/workflows/*) or Dockerfiles during the connector scan. If they exist under unusual names/paths, they were not surfaced by the repo search. This means there is currently no automated guardrail preventing the kind of API-contract drift visible between API/WebGUI/TUI.
Dependency locking

    WebGUI has a package-lock.json.
    TUI has a Cargo.lock.
    These improve reproducibility, but won’t help if component contracts remain inconsistent.

Prioritized bug list and improvements
Bug severity table with counts and files affected
Severity	Count	Most affected files
Critical	8	WebGUI config.ts, use-chat-stream.ts, lib/api/*.ts, types.ts; API index.ts, routes/generation.ts; TUI commands/auth.rs, commands/chat.rs, scripts
Major	7	API routes/generation.ts, routes/messages.ts, Prisma schema; WebGUI settings + optimistic updates; TUI port/name inconsistencies
Minor	6	WebGUI a11y + lint (sidebar/chat-input/page/settings); TUI backup file + naming; docs staleness
Critical bugs (highest priority)
Bug	How to reproduce	File/line refs	Suggested fix
WebGUI default API base points to Next server (3000/api)	Start API on 3737; start WebGUI; all API calls fail	Starbot_WebGUI/src/lib/config.ts:1 	Default NEXT_PUBLIC_API_URL to http://localhost:3737/v1 (or add Next proxy)
WebGUI routes don’t match API (/projects, /chats, /messages)	Load sidebar; create chat; send message → 404s	projects.ts:6–10, chats.ts:6–14, messages.ts:5–6 	Rewrite WebGUI API client to match /v1 routes and wrapper responses ({projects}, {chats}, {message})
WebGUI streaming uses native EventSource GET /chats/:id/stream and wrong events	Send a message; UI waits forever	use-chat-stream.ts:21, 34, 58 	Replace with fetch-based POST stream parsing to /v1/chats/:id/run, listen for token.delta / message.final (MDN SSE format)
CORS blocks WebGUI origin	Point WebGUI at 3737; browser requests fail preflight	Starbot_API/src/index.ts:34–36 	Add http://localhost:3000 to allowed origins or env-configure CORS
WebGUI settings mismatch (thorough, `fast	quality, autoRun`)	Choose “thorough”; run fails validation	types.ts:29, settings-panel.tsx:51
API forwards tool messages as provider roles	POST a tool role message; run generation; provider may error	messages.ts:7, generation.ts:178–179 	Filter/remap tool messages before provider call (e.g., remap tool→system)
TUI device QR login expects /v1/auth/device/* missing in API	Run scripts/starbott-dev.sh without token; it triggers device login and fails	commands/auth.rs + script 	Implement device endpoints in API or disable device login until available
Rust CLI chat calls /v1/inference/chat missing	Run starbott chat "hi" or smoke.sh	commands/chat.rs + smoke 	Replace with Project/Chat/Message/Run flow or implement compatibility endpoint
Major bugs and reliability issues

    chat.updated emits stale title after DB update (generation.ts around lines ~216 and ~243).
    /cancel exists but is unimplemented (generation.ts end).
    API does not set x-request-id, but Rust client reads it (ApiClient extracts x-request-id).
    SQLite URL is relative (file:../starbot.db), which is fragile across working directories and packaging.
    TUI defaults disagree with scripts/README on port/back-end (3737 vs 3003).
    WebGUI stream merge drops deltas when cache is empty (use-chat-stream.ts line ~37).
    WebGUI optimistic IDs temp-user/temp-assistant can collide with real IDs and confuse diffing.

Minor issues: lint, accessibility, repo hygiene

    Unused imports/vars likely fail eslint (Sidebar imports Folder, useRouter, loads projects, sets router unused).
    chat-input.tsx imports useState but doesn’t use it (likely lint error).
    Icon-only buttons without aria-label (page/chat-input/settings).
    tui.rs.backup should not be committed.
    Branding drift (starbot vs starbott) in CLI name/config directories and docs.

Recommended fixes table with effort
Fix	Effort	Notes
Make API contract canonical (document SSE + endpoints; optionally OpenAPI/types)	Medium	Prevents recurring drift; use test-api.sh as baseline
Align WebGUI base URL + routes + response parsing to /v1	Medium	WebGUI becomes functional with current API
Replace WebGUI native EventSource with POST stream parser (ReadableStream SSE)	Medium	Required because native EventSource cannot POST/body; MDN SSE framing applies
Fix API chat.updated to emit updated title	Low	Use Prisma update result before emit
Filter/remap tool messages before provider call	Low	Prevents provider runtime errors
Implement real cancel via AbortController / run registry	Medium	Unblocks UX, reduces cost runaways
Add x-request-id middleware in API	Low	Rust already reads it; huge debugging win
Implement device-code auth endpoints in API, or disable TUI device auth temporarily	High	Enables QR linking; requires DB + security model
Add CI: WebGUI lint/build, API typecheck + harness, Rust fmt/clippy/test	Medium	Reproducibility; stops regressions
Implement Workspace + Memory docs (MEMORY.md, PMEMORY.md) model + retrieval rules	High	Brings repo to your intended product semantics
Mermaid diagram: intended entities for workspace↔repo↔project↔MEMORY.md

The current API schema is Project→Chat→Message only, so this diagram reflects the target you described (and what the code should evolve toward):

optional

github repo

contains

hosts

MEMORY.md

PMEMORY.md

Workspace

RepoLink

Repository

Project

Thread

MemoryDoc

ProjectMemoryDoc

Code snippets illustrating high-impact fixes

WebGUI: fix base URL + use /v1 by default

ts

// Starbot_WebGUI/src/lib/config.ts
export const API_BASE_URL =
  process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3737/v1';

WebGUI: stream /v1/chats/:id/run with fetch + SSE parsing (instead of EventSource)
(Uses MDN’s event: / data: / blank-line framing.)

ts

export async function runChatStream(chatId: string, body: any, onEvent: (evt: {event: string, data: any}) => void) {
  const res = await fetch(`${API_BASE_URL}/chats/${chatId}/run`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });

  if (!res.ok || !res.body) throw new Error(`run failed: ${res.status}`);

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = '';

  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });

    let idx;
    while ((idx = buf.indexOf('\n\n')) !== -1) {
      const block = buf.slice(0, idx);
      buf = buf.slice(idx + 2);

      let event = 'message';
      const dataLines: string[] = [];
      for (const line of block.split('\n')) {
        if (line.startsWith('event:')) event = line.slice(6).trim();
        if (line.startsWith('data:')) dataLines.push(line.slice(5).trimStart());
      }
      const raw = dataLines.join('\n');
      onEvent({ event, data: raw ? JSON.parse(raw) : null });
    }
  }
}

API: fix stale chat.updated emission

ts

const updated = await prisma.chat.update({ /* ... */ });
sendEvent('chat.updated', { id: updated.id, title: updated.title, updatedAt: updated.updatedAt.toISOString() });

API: filter/remap tool roles before provider call

ts

const providerMessages = chat.messages
  .filter(m => m.role !== 'tool')
  .map(m => ({ role: m.role as 'user'|'assistant'|'system', content: m.content }));

External integration guidance for GitHub and Notion

When you implement “workspace = GitHub repo” and “Notion search/edit”:

    Prefer a GitHub App over an OAuth App for repo-scoped access: GitHub Apps have fine-grained permissions, repository-scoped installations, and short-lived tokens (reduces blast radius).
    Implement Notion OAuth via the official flow: exchange code for tokens at the Notion token endpoint (/v1/oauth/token) using HTTP Basic auth, then use Authorization: Bearer for API calls.
    For SSE ergonomics in Fastify, consider adopting @fastify/sse once you add session/replay/heartbeat needs; it provides first-class SSE semantics and lifecycle integration.

