# Starbot Memory & Retrieval Design

Starbot uses a hierarchical memory system.

There are two persistent memory files:

- MEMORY.md (workspace-level)
- PMEMORY.md (project-level)

These are canonical markdown documents.

---

## Storage Model

MemoryDocument:
{
  id,
  scope: "workspace" | "project",
  scope_id,
  content,
  updated_at
}

Memory content is also indexed into vector embeddings.

---

## Embeddings Strategy

Embedding model:
  - Use single model across system (e.g. text-embedding-3-large)

Chunking rules:
  - Split by markdown headings
  - Max 800 tokens per chunk
  - Overlap 100 tokens

Each chunk stores:
{
  chunk_id,
  parent_memory_id,
  text,
  embedding_vector
}

---

## Retrieval Rules

Workspace thread:
  Retrieve:
    - Top 5 similar chat chunks (workspace only)
    - Top 3 relevant MEMORY.md chunks

Project thread:
  Retrieve:
    - Top 8 similar chat chunks (project-wide)
    - Top 5 relevant PMEMORY.md chunks
    - Top 3 relevant workspace MEMORY chunks (across workspaces)

Never retrieve from other projects.

---

## Injection Order

Final prompt context:

1. System base prompt
2. PMEMORY.md chunks (if project thread)
3. MEMORY.md chunks (if workspace thread)
4. Retrieved chat history snippets
5. Current user message

---

## Token Budgeting

Total model context budget:
  ~16k tokens (configurable)

Allocation:
  - 15% memory
  - 35% retrieval
  - 40% conversation
  - 10% buffer

If overflow:
  - Drop lowest similarity retrieval chunks first
  - Never drop current message
  - Never drop PMEMORY in project thread

---

## Conflict Resolution

If PMEMORY and MEMORY conflict:
  PMEMORY wins in project scope.
  MEMORY wins in workspace scope.

Conflict resolution strategy:
  - Add structured "Decisions" section in memory files.
  - Timestamp entries.
  - Retrieval prioritizes newest chunk.

---

## Editing Memory

Model cannot directly overwrite memory.

Workflow:

1. Model proposes diff.
2. User approves.
3. Server applies patch.
4. Memory re-indexed.

---

## Future Enhancements

- Semantic versioning of memory files
- Change history log
- Auto-summary of memory growth
- Memory compression routine
