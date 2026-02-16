# SPEC3 Status Snapshot (2026-02-15)

This document records the current repo state relative to `specs/SPEC3_CODE.md` as of 2026-02-15.

## Scope Reviewed

- `Starbot_API`
- `Starbot_WebGUI`
- `Starbot_TUI`
- `deploy/*`
- `.github/workflows/*`
- `specs/SPEC3_CODE.md`

## Repo Shape

- The workspace is a single git repo at `/home/stella/projects/starbot`.
- The three app folders are siblings in that repo: `Starbot_API`, `Starbot_WebGUI`, `Starbot_TUI`.

## SPEC3 Findings: Still Accurate

- WebGUI/API contract drift exists for `Project.updatedAt`.
  - WebGUI expects it in `ProjectSchema`.
  - API/Prisma `Project` model does not have `updatedAt`.
- Inference endpoint accepts `provider` and `model` but does not use them for model selection.
- Generation route accepts `speed` and `auto` but does not currently apply them in selection or generation behavior.
- Prisma datasource URL is hard-coded in schema instead of `env("DATABASE_URL")`.
- SSE parsing in WebGUI is still line-based and does not assemble multi-line `data:` payloads per SSE framing rules.
- Expensive generation endpoints still do not enforce authentication.

## SPEC3 Findings: Outdated (Already Improved)

- WebGUI now triggers generation after sending a user message.
- WebGUI now forwards `mode`, `auto`, `speed`, and `model_prefs` to `/v1/chats/:chatId/run`.
- API and TUI GitHub Actions no longer mask lint/test failures with `|| echo`.
- systemd service files are configured to run as `User=starbot` and `Group=starbot` instead of `root`.
- nginx config no longer injects permissive wildcard CORS headers for `/v1/`.

## New Critical Drift Not Explicitly Called Out in SPEC3

- WebGUI calls message endpoints that API does not implement:
  - WebGUI calls `PUT /messages/:id`
  - WebGUI calls `DELETE /messages/:id`
  - WebGUI calls `DELETE /chats/:chatId/messages/after/:messageId`
  - API currently implements only:
    - `GET /chats/:chatId/messages`
    - `POST /chats/:chatId/messages`

## Quick Check Results

### Starbot_API

- `npm run build`: pass
- `npm run lint`: fail (`eslint: not found`)
- `npm test`: fail
  - project route test expecting 400 currently gets 500 on empty project name
  - chunking max token expectation failing

### Starbot_WebGUI

- `npx tsc --noEmit`: pass
- `npm run lint`: fail (multiple errors, including `no-explicit-any`, React hooks lint violations, and UI type issues)
- `npm run build`: timed out at 120s while still at "Creating an optimized production build ..."

### Starbot_TUI

- `cargo check`: pass (with warnings)
- `cargo clippy --all-targets --all-features -- -D warnings`: fail (unused imports/vars, dead code, clippy style violations)

## Immediate Priority Fixes

1. Resolve message API mismatch between WebGUI and API (either implement missing endpoints in API or remove unsupported client calls).
2. Fix WebGUI `ProjectSchema` contract (`updatedAt` handling) to match API output.
3. Stabilize WebGUI build/lint so production build is deterministic.
4. Fix API test failures and ensure API lint toolchain is installed and runnable.

