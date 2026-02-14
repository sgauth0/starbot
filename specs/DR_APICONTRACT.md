# Starbot API Contract (v1)

Base URL (dev):
http://localhost:3737/v1

All responses are JSON unless otherwise specified.
All authenticated requests require:

Authorization: Bearer <access_token>

---

## Authentication

### Device Flow

POST /auth/device/start
Body: {}
Response:
{
  "device_code": string,
  "user_code": string,
  "verification_url": string,
  "expires_in": number,
  "interval": number
}

POST /auth/device/poll
Body:
{
  "device_code": string
}
Response (pending):
{
  "status": "pending"
}
Response (authorized):
{
  "status": "authorized",
  "access_token": string,
  "refresh_token": string,
  "expires_in": number
}

POST /auth/device/confirm  (WebGUI only)
Body:
{
  "user_code": string
}
Response:
{
  "status": "confirmed"
}

---

## Projects

GET /projects
Response:
{
  "projects": Project[]
}

POST /projects
Body:
{
  "name": string
}
Response:
{
  "project": Project
}

Project:
{
  "id": string,
  "name": string,
  "created_at": string
}

---

## Workspaces

GET /projects/:projectId/workspaces
POST /projects/:projectId/workspaces

Workspace:
{
  "id": string,
  "project_id": string,
  "type": "repo" | "folder" | "cloud",
  "identifier": string,
  "created_at": string
}

---

## Memory

GET /projects/:projectId/memory
PUT /projects/:projectId/memory

Response:
{
  "content": string,
  "updated_at": string
}

GET /workspaces/:workspaceId/memory
PUT /workspaces/:workspaceId/memory

---

## Chats (Threads)

GET /projects/:projectId/chats
POST /projects/:projectId/chats

Chat:
{
  "id": string,
  "project_id": string | null,
  "workspace_id": string | null,
  "title": string,
  "created_at": string
}

---

## Messages

GET /chats/:chatId/messages

Response:
{
  "messages": Message[]
}

POST /chats/:chatId/messages

Body:
{
  "role": "user" | "assistant" | "system",
  "content": string
}

Message:
{
  "id": string,
  "chat_id": string,
  "role": string,
  "content": string,
  "created_at": string
}

---

## Generation (Streaming)

POST /chats/:chatId/run

Headers:
Content-Type: application/json
Accept: text/event-stream

Body:
{
  "mode": "quick" | "standard" | "deep",
  "include_project_memory": boolean,
  "workspace_scope": "workspace" | "project"
}

Response:
Content-Type: text/event-stream

Events:

event: status
data: { "message": string }

event: token.delta
data: {
  "message_id": string,
  "delta": string
}

event: message.final
data: {
  "message_id": string,
  "content": string,
  "usage": {
    "input_tokens": number,
    "output_tokens": number
  }
}

event: chat.updated
data: {
  "chat_id": string,
  "title": string
}

event: run.error
data: {
  "code": string,
  "message": string
}

---

## Integrations

GET /integrations

POST /integrations/notion/connect
POST /integrations/github/connect

POST /tools/notion/search
POST /tools/notion/update

POST /tools/github/search
POST /tools/github/pr
