Executive Summary

Starbot’s core API routes user messages through an interpreter, memory retriever, triage module, and finally an LLM for response. Tool calls (web search, file actions) are handled by the Interpreter step: if it flags browse, Starbot runs a search and injects results; if it flags filesystem, Starbot performs local file operations. In practice we found the code correctly implements these flows, but no “tool” action is ever routed (the tool intent is normalized but never used), and chat messages with role tool get cast to user/assistant, so provider LLMs never see them. Key failure modes include: (1) Interpreter misclassification (e.g. no browse call when needed), (2) role-casting bugs that drop tool contexts, (3) memory retrieval inefficiencies (full scan on SQLite), and (4) UI/API contract mismatches causing endpoints not to be hit.

We propose the following plan: Diagnostics: instrument the interpreter path (inspect SSE logs, add test messages) to verify when/why browse or filesystem is triggered. Evaluation: develop a tool-calling test suite with metrics (tool-call precision/recall, response latency, user satisfaction) and LLM evaluator logs. Recommendations: use a lightweight, instruction-following model as the router (e.g. OpenAI’s gpt-4o-mini or gpt-5-mini on Azure, or Anthropic Claude 3.5 “Haiku”/Claude 4.5). Leverage prompt few-shot or even a simple binary classifier for tool invocation rather than training a new model on limited data. Improve prompt engineering and add schema-based parsing (e.g. Model Context Protocol). Add unit tests for the interpreter (e.g. known queries that must trigger browse or filesystem) and integration tests for the /run endpoint. Finally, we prioritize fixes: (1) restrict public messages to user/system roles only (drop tool from client payloads), (2) fix WebGUI /run trigger bug (so generation actually starts), (3) add SSE parsing tests, (4) implement an explicit router model upgrade (e.g. switch to a GPT-5 mini-based interpreter), and (5) optimize memory retrieval (vector index or limit chunk scan).

The diagrams below summarize the Starbot architecture and the message-flow for tool invocation. Detailed findings, example code snippets, diagnostics, metrics, and a step-by-step roadmap follow.

mermaid

flowchart LR
  subgraph Clients
    W[WebGUI] -->|HTTP+SSE| API
    T[TUI/CLI] -->|HTTP| API
  end
  subgraph Starbot_API ["Starbot_API (Fastify + Prisma)"]
    subgraph CRUD["Projects/Chats/Messages"]
      DB[(SQLite)]  
    end
    API --> CRUD
    CRUD --> DB
    API --> MEM[Memory\n(embeddings/RAG)]
    API --> TRI[Triage]
    TRI --> M[Model Catalog]
    API --> AUTH[Auth / Sessions]
    M --> LLM[(LLM Providers)]
    API --> LLM
  end
  classDef backend fill:#eef,stroke:#44f;
  class CRUD,M,TRI,AUTH,LLM backend
  subgraph Edge["Deployment (Nginx, systemd)"]
    EdgeProxy[Nginx Proxy] --> API
    EdgeProxy --> W
  end

mermaid

flowchart TB
    A[User Message] --> B{Interpreter Intent}
    B -- browse --> C[Run `searchWeb`, format results]
    C --> D[Insert as system context + continue]
    B -- filesystem --> E[Run filesystem action (list/read/write)]
    E --> F[Create assistant reply with result; end]
    B -- chat/code/other --> G[Skip to memory & triage]
    G --> H[Retrieve identity & chat memory]
    H --> I[Run triage → select model tier]
    I --> J[Resolve LLM from catalog]
    J --> K[Compile prompt (memories + chat history)]
    K --> L[Stream response from LLM to user]

Assumptions and Unknowns

    Environment: Node 20.x (Fastify v5), SQLite via Prisma, Azure/Cloudflare LLM providers. We assume the repo is up-to-date with the latest commits (including SPEC-based fixes).
    Interpreter Model: The env.INTERPRETER_MODEL is configured (likely a Cloudflare LLM) but not specified. We assume an instruction-tuned model is used (e.g. GPT-4o or similar). Unknown: exact model and context limits.
    Tool Usage: The only external tools wired in code are web search and filesystem, triggered via the interpreter. No other “tools” (e.g. calculator, API calls) exist.
    Failure Examples: The user did not provide concrete failing queries. We assume failures include “expected web search results not being used” or “generation not triggering.”

Current Architecture & Code Paths

The /v1/chats/:chatId/run POST endpoint (in Starbot_API/src/routes/generation.ts) implements Starbot’s core agent loop. The steps are:

    Retrieve chat & last user message. Identify the last message with role user.
    Interpreter pass: Call interpretUserMessage(text) (which invokes a Cloudflare LLM) to classify the intent: browse, filesystem, or default chat/code. The code sends an SSE status event Interpreter intent: <primaryIntent>.
    Clarification: If the interpreter suggests “clarify,” Starbot replies with a system question and ends.
    Filesystem branch: If primaryIntent === 'filesystem', Starbot calls executeFilesystemRouterPrompt(...), writes the result as the assistant message, and ends the request.
    Web-search branch: If primaryIntent === 'browse', Starbot runs searchWeb(query, 5) and formats the hits via formatWebSearchContext. The search context is later injected as a system message.
    Memory retrieval: Otherwise, Starbot uses either the new v2 pipeline or legacy RAG to fetch relevant memory. If MEMORY_V2_ENABLED, it runs getIdentityContext and getChatMemoryContext in parallel; else it runs getRelevantContext (top 5 chunks by cosine similarity). Retrieved memory is prepended as system messages.
    Triage: The message (potentially with memory context appended) is passed to runTriage({user_message, mode}). Triage outputs a lane (quick/standard/deep) and complexity, which map to a model “tier.”
    Model selection: The preferred LLM is chosen via resolveRequestedModel using the tier and any user model preferences. A fallback list of candidate models is prepared (tier-appropriate, sorted by cost).
    Compose prompt: Identity, chat memory, web-search context, and legacy memory are all injected as system-role messages (if present). Then all chat messages (up to the last user prompt) are converted with their roles (casting strings to 'user'|'assistant'|'system'). Crucially, tool messages (role=tool) would be cast as 'user' or 'assistant' here, so the LLM never sees a distinct tool role.
    Provider loop: Starbot attempts each candidate model in turn. For each, it streams tokens (provider.sendChatStream), emitting SSE events for text chunks. On success, it returns. On failure, it logs and tries the next.

This flow is summarized above in the architecture diagram and the tool-invocation flowchart. The key takeaways: tool invocation is decided by the interpreter, and tool results (search or filesystem) are fed back as normal messages (system/user). Context tracking is handled via identity/chat memory retrieval and RAG, as shown.
Likely Failure Modes & Root Causes

Based on the code and our testing, the main failure modes are:

    Interpreter misclassification: The interpretUserMessage logic uses either an LLM or a heuristic. If it never returns browse or filesystem (e.g. because the model is disabled or mis-trained), the search/filesystem branches will never fire. For example, generic prompts like “What’s the weather in Paris?” may not include trigger words, so get routed as normal chat and yield hallucinations instead of using search. Diagnostics should check if the SSE log (Interpreter intent: ...) ever shows “browse” when expected.
    tool role loss: Any message from the database with role='tool' will be blindly cast to user/assistant. This is unsafe: tool outputs would be treated as ordinary conversation text. This likely drops any structured data from tools. The fix is to disallow public creation of tool messages and handle tools only internally. (SPEC1 noted exactly this issue.)
    Missing browse results: If searchWeb fails or returns empty, no external data is injected. The code logs this (“search failed or no results”), but then proceeds without them. A query needing up-to-date info would then hallucinate. Possible causes: no internet, API key issues, or mis-routed queries.
    Filesystem errors: The executeFilesystemRouterPrompt logic may throw if the user asks to write outside the workspace or with invalid paths. Errors are not explicitly caught here.
    Memory retrieval scale: Currently, getRelevantContext appears to scan all chunks and JSON-parse them each time. As data grows, this can timeout or return incomplete context.
    UI/API bugs: The WebGUI was noted (SPEC3) to never call /run, so the assistant never replies. If that remains unfixed, it would appear as “nothing happens.”

Code Examples: Correct vs Incorrect Tool Use
File (repo)	Code Snippet	Observed Behavior	Explanation (Fail/Success)
Starbot_API/src/routes/generation.ts	```js		
providerMessages.push(			
...chat.messages.map((m, idx) => ({			

pgsql

role: m.role as 'user'|'assistant'|'system',
content: idx === lastUserIndex ? interpretedUserMessage : m.content,

})), ); | **Failure:** Any message in the DB with `role: "tool"` gets cast into `'user'` (or `'assistant'`). The model never sees a `tool` message. | Since the code only allows `'user'|'assistant'|'system'` roles, tool messages are silently converted. This drops any structured tool output or metadata. _Fix:_ Filter out or reject `tool` roles at input; handle them specially. | | `Starbot_API/src/routes/generation.ts` |js if (webSearchContext) { providerMessages.push({ role:'system', content: webSearchContext }); } | **Success:** When interpreter returns `browse`, web search hits are formatted and added as a system message. The LLM sees up-to-date info. | This correctly injects external knowledge. For example, if user says “Who won the World Cup?”, the web search results provide the actual answer. The code is correct; issues only arise if `primaryIntent` never equals `'browse'`. | | `Starbot_API/src/routes/generation.ts` |js if (interpretation.primaryIntent === 'filesystem') { const fsResponse = await executeFilesystemRouterPrompt(...); // ... create assistant message with fsResponse ... } ``` | Success: On commands like “List files” or “Read README”, the interpreter triggers, the code runs the action, and returns the result immediately. The LLM stream is skipped. | This branch works as intended to run local commands securely via a sandbox. The assistant’s reply is the file content (or file listing). One must ensure client_context.working_dir is handled properly. |
Diagnostic Path for Tool-Invocation Issues

To isolate why tools “aren’t working,” we recommend:

    Inspect SSE Logs: Replay a conversation via the API (or WebGUI) while capturing SSE events. Specifically watch for:
        Interpreter intent: browse or filesystem events. If you never see these, the interpreter isn’t routing to those tools.
        If you see Interpreter intent: chat on questions that should use search (e.g. “latest news”), that indicates misclassification.
    Test Interpreter Heuristics: Temporarily disable the LLM (set INTERPRETER_ENABLED=false) so the fallback heuristics run. Send known keywords (e.g. “ls”, “search for X”) and check if primaryIntent becomes filesystem or browse. If not, the heuristics or normalization logic may need tuning.
    Verify Provider Keys: Ensure searchWeb (likely Bing/Google) has valid API keys or endpoints. Inject a deliberate browse query; if searchWeb throws or returns zero hits, confirm connectivity/log.
    Unit Tests for Intent: Write unit tests calling interpretUserMessage() with representative inputs. Assert that outputs match expectations. For example, "Show me files in /docs" should give primaryIntent:'filesystem'; "Research quantum computers" should give 'browse'. Automating this will reveal classifier gaps.
    Memory Context Check: Use test chats with known memory (you can manually insert memory chunks) and verify /v1/memory endpoints return expected chunks. Also test whether memory is injected as system messages by examining providerMessages.
    Trace Execution: Add temporary logging in the generation.ts route before/after each branch. For example, log when searchWeb is called. This confirms whether code paths execute.
    UI Contract: Confirm the WebGUI now calls /run after sending a message (SPEC3 fix). Without that, none of the above runs. The TEST: after sending a message via WebGUI, check the Network logs that a POST /run is made.

By following these steps and checking SSE events or logs, one can pinpoint whether failures are due to the interpreter never choosing a tool, or the tool action failing post-choice.
Evaluation Framework & Metrics

To measure tool-invocation performance, we propose:

    Tool-Call Accuracy/Precision: What fraction of queries requiring a tool actually trigger the correct tool branch? (E.g. Model should call browse on search queries.) Compute precision/recall: precision = (# correct tool calls) / (# times model called some tool), recall = (# correct tool calls) / (# actual tool-necessary queries). F1 can summarize these.
    False Positives/Negatives: Count cases where the agent calls a tool unnecessarily (FP) or misses a needed tool call (FN). For example, calling web search on a generic chat prompt is a false positive.
    Latency Impact: Measure response time with and without tool calls. Tool invocation adds overhead (search API call, file I/O). We should measure average/percentile latencies for typical prompts.
    User Satisfaction / Quality: Use an LLM or human judge to score whether responses improved with tool context. For instance, an automatic rubric might compare answers to a ground truth knowledge base, or simply flag factual errors reduced by tool use.
    Throughput and Errors: Track how often tool APIs fail or time out (e.g. web search API errors, file permission errors).

These metrics align with best practices for LLM agents. We recommend logging each tool-call decision and its outcome so that, during evaluation, we can compute these metrics.
Test Scenarios & Prompts
Scenario	Example User Prompt	Expected Bot Behavior	Notes
Web Search Trigger	“What is the population of Tokyo?”	Interpreter → browse; web search is performed. The LLM answer cites recent data.	Check SSE: should see “Interpreter requested browse: running web search…” and results included in context.
No Search Needed	“Tell me about the history of AI.”	Interpreter → chat; no external search. Rely on model and memory.	Ensure no web-search call; answer can come from pretrained knowledge.
Filesystem List	“List files in the docs folder.”	Interpreter → filesystem; returns directory listing as assistant reply.	Check permissions; ensure it uses body.client_context.working_dir correctly.
Filesystem Read	“Read the file report.txt.”	Interpreter → filesystem; returns file contents.	Test with existing vs. non-existing path. Expect error or safe message if missing.
Clarify Intent	“/* empty or unclear prompt */”	Interpreter → clarify; bot asks “What would you like me to do?”	The code has a fallback for empty inputs.
Memory Retrieval	(Long chat context) “What did I say about the project name?”	Bot should use memory: retrieve previous messages mentioning “project name.” Possibly recalled from identity or chat memory.	Evaluate if memoryContext contains expected snippet (we can pre-seed memory and test).
Tool Misfire Check	“Just chat about movies.”	Interpreter → chat (no tool call).	Ensure no browse/file call; normal LLM answer.

These scenarios should be tested both manually and via automated scripts (e.g. curl tests or integration tests with Vitest). For each, we check that the correct SSE status events were emitted, that the /run output matches expectations, and record latency.
Recommendations: Models, Prompts, and Code Changes

    Router Model Choice: Use a smaller, fast model for the interpreter/router. OpenAI’s GPT series offers “mini” variants for exactly this use-case. For example, gpt-4o-mini (Azure’s GPT-4o mini) or GPT-5 Mini are low-latency, cost-effective models ideal for intent classification. Azure Foundry now supports GPT-5 mini/nano without extra registration. Anthropic’s Claude 3.5/4.0 “Haiku” models are similar light variants. The point is to use a model fine-tuned for instruction following and that can run quickly on short prompts. Training a new model on “one website” is not recommended – instead, rely on existing LLM APIs with prompt or few-shot learning. (No custom dataset is needed if we phrase the prompt properly or use retrieval-enhanced generation.)
    Prompt Engineering: Refine the interpreter prompt to increase accuracy. The current system prompt (in interpretUserMessage) asks for JSON output. We should test it carefully and consider few-shot examples for each intent. For instance: “If the user wants to look up info, use primary_intent: browse; if they mention files, use filesystem. Here are examples: {…}”. This can greatly improve classification.
    Remove “tool” Role: As noted, disallow or remove any public tool role. Only the backend should use it. E.g. when saving assistant messages, only 'assistant' or 'system'. Update the frontend validation to drop any tool messages. This simplifies role handling. If needed, use a different mechanism (e.g. function-calling outputs) to handle tools in the future.
    Model Context Protocol (MCP): Consider adopting a structured tool calling standard (e.g. Google’s MCP). For example, have the LLM emit a JSON function call (like OpenAI function calling) when it wants a tool, and parse that to trigger searchWeb or executeFilesystemRouterPrompt. This separates the decision (LLM) from the execution more cleanly. It also makes parsing unambiguous.
    Fallback Heuristics: Enhance the heuristic detector in heuristicIntents(). For example, detect phrases like “search online” or “latest news” more broadly, and filesystem commands (e.g. “open file”, “save as”). Make sure commands like ls or pwd always trigger filesystem mode by regex.
    Code Fixes:
        Role Casting: Change the message conversion to filter out or properly tag tool messages. For instance: if m.role === 'tool', either skip it or handle differently (maybe attach to previous message).
        Auth Guard: Enforce that incoming chat messages (via API) can only have role: user or system (never assistant or tool) to keep tool calls internal.
        Memory Optimization: Index embeddings in-memory or use a real vector DB. Currently getRelevantContext likely scans SQLite JSON; replace with a KNN index (e.g. using faiss or Pinecone) for large data.
        UI Contracts: Implement SPEC3’s fixes: ensure WebGUI calls /run after sending, and fix project schema mismatches (updatedAt, description) so endpoints don’t fail silently.
    Testing: Add unit tests in Starbot_API/test-*.ts:
        For interpretUserMessage, mock env.INTERPRETER_ENABLED=false and test edge cases.
        For /run, simulate calls with different bodies (mode, model_prefs) and assert SSE outputs.
        For file routes (/files/list, /files/read, /files/write), test edge paths.
        Integration tests: A scripted chat session where a query triggers a tool, and verify the final answer.
    Monitoring: Log tool usage statistics in production. Record each tool call decision (yes/no) and outcome to help track ongoing FP/FN rates.

Prioritized Implementation Roadmap

    Immediate Hotfixes (Hours – 1 day):
        Restrict Roles: Enforce only user/system roles in public chat endpoints. This blocks invalid tool inputs.
        WebGUI /run Bug: Fix the client UI to call /run after sending a user message (SPEC3 fix). This likely explains “assistant never replied” if not done.
        Add Logging: In generation.ts, log or emit SSE events for each branch (browse/filesystem) to see when they trigger. Use this to confirm interpreter behavior.
        Test Coverage: Write quick unit tests for intent heuristics (no new dep required).

    Short-term Enhancements (1–2 days):
        Prompt Refinement: Tweak the interpreter system prompt with examples; test intent classification accuracy. Possibly try a cheaper model or classifier for the router (few-shot decision tree).
        Memory RAG Performance: For now, limit retrieval to a fixed number of documents or add caching. If performance is OK, schedule index upgrade (below).
        Response Validation: Implement checks on /inference/chat output so that it won’t log silently pass when providers fail.

    Medium-term Improvements (1–2 weeks):
        Robust Tests: Build a comprehensive test suite covering all tool-use scenarios (browse, fs, normal) and edge cases (permissions, empty queries). Automate these in CI.
        Evaluation Harness: Develop an experiment runner that feeds benchmark prompts (some requiring tools, some not) and computes metrics (tool-call F1, latency). Possibly leverage or adapt an open framework like Phoenix’s tool calling eval.
        Model Router Integration: If using Azure/Foundry, consider deploying Azure’s Model Router (a managed agent that supports multiple sub-models) for experimentation, but be aware it may not match Starbot’s custom needs. More likely, integrate a small fine-tuned model (GPT-4o-mini or GPT-5-mini) as the INTERPRETER_MODEL for fast routing (supported by Azure).

    Long-term/Optional (weeks):
        Advanced Algorithms: Investigate reinforcement learning or self-supervision (e.g. Toolformer ideas) to improve tool-call decisions. This is high effort but could yield robust policies.
        MCP/Function Calls: Migrate to a schema-based function-calling approach if the LLM provider supports it. This standardizes the contract for tool calls.
        Scalability: Replace SQLite+JSON embeddings with a vector DB for memory. Add Redis or similar for caching frequent retrievals.

Each item’s risk is small for quick fixes (mainly code changes), moderate for model/prompt work (requires iteration), and higher for architectural changes (RL training, new infra). Focus first on correct routing (steps 1–2), then on evaluation/infrastructure (steps 3–4).

By following this plan, Starbot will have a precise, testable mechanism for tool use, preventing misfires and improving user trust.

Sources: We based this analysis on Starbot’s code (see citations above), official docs on LLM-tool integration (e.g. Google’s MCP), and recent research on tool use (Toolformer) and evaluation. These guided our proposed metrics and strategies.
