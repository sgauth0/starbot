# Starbot API Contract (v1 Implementation Snapshot)

Snapshot date: 2026-02-16

This document reflects the current implemented API behavior in `Starbot_API/src/routes/*`.
It is intentionally implementation-first so clients do not drift.

## Base URL

- Dev: `http://localhost:3737/v1`
- Production: `https://starbot.cloud/v1` (when deployed behind nginx)

## Auth and Headers

- `Content-Type: application/json` is required for JSON request bodies.
- Streaming endpoint uses `Accept: text/event-stream`.
- CORS is configured server-side for localhost and `starbot.cloud` origins.
- API token headers are not currently enforced on v1 business routes.
  - WebGUI currently sends `X-API-Token`.
  - TUI currently sends `Authorization: Bearer <token>`.
- Device auth endpoints exist and return access tokens, but those tokens are not yet enforced for route authorization.
- Optional feature flags exist for enforcement and throttling:
  - `AUTH_ENFORCEMENT_ENABLED=true` enforces token presence on expensive routes.
  - `RATE_LIMITING_ENABLED=true` enables in-memory per-client limits.

## Health

### `GET /health`

- Redirects to `/v1/health` with HTTP 301.

### `GET /v1/health`

Response:

```json
{
  "status": "ok",
  "timestamp": "2026-02-16T00:00:00.000Z",
  "version": "1.0.0"
}
```

## Authentication (Device Flow, In-Memory Stub)

### `POST /auth/device/start`

Request body: `{}` (unused)

Response:

```json
{
  "device_code": "string",
  "user_code": "ABC-123",
  "verification_url": "http://localhost:3000/auth/device",
  "expires_in": 900,
  "interval": 5
}
```

### `POST /auth/device/poll`

Request:

```json
{
  "device_code": "string"
}
```

Pending response:

```json
{
  "status": "pending",
  "message": "User has not yet authorized this device"
}
```

Authorized response:

```json
{
  "status": "authorized",
  "access_token": "string"
}
```

Possible errors:

- `404`: `{ "error": "Device code not found" }`
- `403`: `{ "error": "authorization_denied", "message": "..." }`
- `410`: `{ "error": "expired_token", "message": "..." }`

### `POST /auth/device/confirm`

Request:

```json
{
  "user_code": "ABC-123",
  "action": "approve"
}
```

`action` is optional. If `action` is `"deny"`, request is denied. Any other value (or omitted) authorizes.

Responses:

- Authorized:

```json
{
  "status": "authorized",
  "message": "Device authorized successfully"
}
```

- Denied:

```json
{
  "status": "denied"
}
```

### `GET /auth/device/pending/:user_code`

Response:

```json
{
  "user_code": "ABC-123",
  "status": "pending",
  "expires_in": 123
}
```

Possible errors:

- `404`: `{ "error": "User code not found" }`
- `410`: `{ "error": "Code has expired" }`

## Projects

Project model (current):

```json
{
  "id": "string",
  "name": "string",
  "createdAt": "2026-02-16T00:00:00.000Z"
}
```

### `GET /projects`

Response:

```json
{
  "projects": [
    {
      "id": "string",
      "name": "string",
      "createdAt": "2026-02-16T00:00:00.000Z",
      "_count": { "chats": 0 }
    }
  ]
}
```

### `POST /projects`

Request:

```json
{
  "name": "My Project"
}
```

Response: `201` with `{ "project": Project }`

### `GET /projects/:id`

Response:

```json
{
  "project": {
    "id": "string",
    "name": "string",
    "createdAt": "2026-02-16T00:00:00.000Z",
    "chats": []
  }
}
```

`chats` is included, ordered by `updatedAt` desc, limited to 10.

### `PUT /projects/:id`

Request:

```json
{
  "name": "Renamed Project"
}
```

Response: `{ "project": Project }`

### `DELETE /projects/:id`

Response:

```json
{
  "ok": true
}
```

## Workspaces

Workspace model (current):

```json
{
  "id": "string",
  "projectId": "string",
  "type": "repo",
  "identifier": "string",
  "createdAt": "2026-02-16T00:00:00.000Z"
}
```

### `GET /projects/:projectId/workspaces`

Response: `{ "workspaces": Workspace[] }`

### `POST /projects/:projectId/workspaces`

Request:

```json
{
  "type": "repo",
  "identifier": "https://github.com/org/repo"
}
```

Response: `{ "workspace": Workspace }`

### `GET /workspaces/:id`

Response:

```json
{
  "workspace": {
    "id": "string",
    "projectId": "string",
    "type": "repo",
    "identifier": "string",
    "createdAt": "2026-02-16T00:00:00.000Z",
    "chats": []
  }
}
```

### `DELETE /workspaces/:id`

Response:

```json
{
  "success": true
}
```

## Memory

Memory endpoints return a `memory` envelope:

```json
{
  "memory": {
    "id": "string",
    "content": "markdown...",
    "updatedAt": "2026-02-16T00:00:00.000Z"
  }
}
```

### `GET /projects/:projectId/memory`

- Auto-creates project memory document if missing.

### `PUT /projects/:projectId/memory`

Request:

```json
{
  "content": "markdown..."
}
```

### `POST /projects/:projectId/memory/process`

Generates chunks and embeddings (if embedding provider configured).

Response:

```json
{
  "status": "success",
  "chunks": 12,
  "embeddings": 12
}
```

### `POST /projects/:projectId/memory/search`

Request:

```json
{
  "query": "search text",
  "workspaceId": "optional-string",
  "topK": 5
}
```

Response:

```json
{
  "results": [
    {
      "chunkId": "string",
      "text": "string",
      "similarity": 0.82,
      "memoryId": "string",
      "scope": "project"
    }
  ]
}
```

### `GET /workspaces/:workspaceId/memory`

- Auto-creates workspace memory document if missing.

### `PUT /workspaces/:workspaceId/memory`

Request:

```json
{
  "content": "markdown..."
}
```

### `POST /workspaces/:workspaceId/memory/process`

Response shape matches project `/memory/process`.

## Chats

Chat model (current):

```json
{
  "id": "string",
  "projectId": "string",
  "workspaceId": "string|null",
  "title": "string",
  "createdAt": "2026-02-16T00:00:00.000Z",
  "updatedAt": "2026-02-16T00:00:00.000Z"
}
```

### `GET /projects/:projectId/chats`

Response:

```json
{
  "chats": [
    {
      "id": "string",
      "projectId": "string",
      "workspaceId": null,
      "title": "string",
      "createdAt": "2026-02-16T00:00:00.000Z",
      "updatedAt": "2026-02-16T00:00:00.000Z",
      "_count": { "messages": 0 }
    }
  ]
}
```

### `POST /projects/:projectId/chats`

Request:

```json
{
  "title": "optional"
}
```

If omitted, title defaults to `"New Chat"`.

Response: `201` with `{ "chat": Chat }`

### `GET /chats/:id`

Response includes `project` and `messages`:

```json
{
  "chat": {
    "id": "string",
    "projectId": "string",
    "workspaceId": null,
    "title": "string",
    "createdAt": "2026-02-16T00:00:00.000Z",
    "updatedAt": "2026-02-16T00:00:00.000Z",
    "project": {},
    "messages": []
  }
}
```

### `PUT /chats/:id`

Request:

```json
{
  "title": "New Title"
}
```

Response: `{ "chat": Chat }`

### `DELETE /chats/:id`

Response:

```json
{
  "ok": true
}
```

## Messages

Message model (current):

```json
{
  "id": "string",
  "chatId": "string",
  "role": "user|assistant|tool|system",
  "content": "string",
  "createdAt": "2026-02-16T00:00:00.000Z"
}
```

### `GET /chats/:chatId/messages`

Response: `{ "messages": Message[] }` ordered ascending by `createdAt`.

### `POST /chats/:chatId/messages`

Request:

```json
{
  "role": "user",
  "content": "hello"
}
```

Response: `201` with `{ "message": Message }`

### `PUT /messages/:id`

Request:

```json
{
  "content": "updated text"
}
```

Response: `{ "message": Message }`

### `DELETE /messages/:id`

Response:

```json
{
  "ok": true
}
```

### `DELETE /chats/:chatId/messages/after/:messageId`

Deletes target message and all later messages in that chat.

Response:

```json
{
  "ok": true,
  "deleted": 3
}
```

## Models

### `GET /models`

Response:

```json
{
  "defaultProvider": "auto",
  "providers": [
    { "id": "auto", "label": "Auto" },
    {
      "id": "azure:gpt-5.2-chat",
      "provider": "azure",
      "model": "gpt-5.2-chat",
      "label": "Azure GPT-5.2 Chat",
      "tier": 2,
      "capabilities": ["text"]
    }
  ]
}
```

## Generation (Streaming)

### `POST /chats/:chatId/run`

Request:

```json
{
  "mode": "quick",
  "model_prefs": "azure:gpt-5.2-chat",
  "speed": false,
  "auto": true
}
```

All fields are optional:

- `mode` defaults to `"standard"`.
- `auto` defaults to `true`:
  - `true`: routing tier follows triage lane (`quick|standard|deep`).
  - `false`: routing tier follows requested `mode`.
- `speed` defaults to `false`:
  - `true`: routing prefers a faster tier (one tier lower, floor at tier 1) and caps generation max tokens at 1024.
  - `false`: uses full selected model output token limit.

Response content type: `text/event-stream`

When feature flags are enabled:

- Returns `401` for missing token if `AUTH_ENFORCEMENT_ENABLED=true`.
- Returns `429` for over-limit callers if `RATE_LIMITING_ENABLED=true`.

SSE event sequence (typical):

1. `event: status`
   - `data: { "message": "..." }`
2. `event: token.delta`
   - `data: { "text": "..." }`
3. `event: message.final`
   - `data` shape:

```json
{
  "id": "assistant-message-id",
  "role": "assistant",
  "content": "full response",
  "provider": "azure",
  "model": "gpt-5.2-chat",
  "modelDisplayName": "Azure GPT-5.2 Chat",
  "usage": {
    "promptTokens": 1,
    "completionTokens": 2,
    "totalTokens": 3
  },
  "triage": {
    "category": "string",
    "lane": "quick|standard|deep",
    "complexity": 1,
    "elapsed_ms": 12
  }
}
```

4. `event: chat.updated`
   - `data: { "id": "chat-id", "title": "title", "updatedAt": "..." }`

Error event:

- `event: error`
- `data: { "message": "string", "fatal": true }`

### `POST /chats/:chatId/cancel`

Current behavior:

```json
{
  "ok": true,
  "message": "Cancellation not yet implemented"
}
```

## Inference (Legacy JSON Endpoint for TUI)

### `POST /inference/chat`

Request:

```json
{
  "messages": [
    { "role": "user", "content": "hello" }
  ],
  "client": "cli",
  "provider": "auto",
  "model": "optional",
  "max_tokens": 512,
  "conversationId": "optional"
}
```

Current behavior notes:

- `provider` and `model` are used for model selection when provided.
- If a specific requested provider/model cannot be resolved to an enabled configured model, endpoint returns `400` with:

```json
{
  "error": "Requested provider/model is not available"
}
```

- Endpoint is non-streaming JSON.
- When feature flags are enabled:
  - Returns `401` for missing token if `AUTH_ENFORCEMENT_ENABLED=true`.
  - Returns `429` for over-limit callers if `RATE_LIMITING_ENABLED=true`.

Response:

```json
{
  "reply": "assistant text",
  "conversation_id": "chat-id",
  "provider": "azure",
  "model": "gpt-5.2-chat",
  "usage": {
    "prompt_tokens": 1,
    "completion_tokens": 2,
    "total_tokens": 3
  }
}
```

## Error Envelope

Most route-level not-found paths use:

```json
{
  "error": "..."
}
```

Some errors include `message`.
Validation errors are not fully normalized yet across all routes.
