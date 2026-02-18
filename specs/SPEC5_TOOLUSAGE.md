
Executive Summary

Starbot is a multi-component system consisting of a Fastify/TypeScript API (Starbot_API) with Prisma/SQLite, a Next.js Web GUI, and a Rust TUI. The API’s core loop retrieves relevant memory, runs a “triage” classifier to pick a model tier, selects an LLM via a model catalog, then streams back tokens via SSE. Currently explicit tool invocation (e.g. calling web search, file I/O, or code execution) is not implemented beyond a generic “tools enabled” flag. In fact, the code casts all messages to the roles user|assistant|system when sending to the LLM (dropping any “tool” messages). This leaves gaps in contextual understanding and external action: for example, queries that require up-to-date data, calculations, or file lookup cannot be fulfilled by tools, leading to hallucination or failure. State-of-the-art research (e.g. Google’s Model Context Protocol (MCP) and the Toolformer framework) shows that LLMs can learn when and how to call tools. We propose an evaluation-driven roadmap to diagnose and remedy these issues: identifying failures (e.g. missing tool calls, corrupted context), designing metrics (precision/recall of tool calls, F1, latency, user satisfaction), and implementing enhancements (prompt engineering, classifiers, RL reward models, context-windows, heuristics). Key recommendations include fixing the message-role casting bug, adding a tool-calling mechanism in the API routing, and establishing unit/integration tests to verify correct tool use. A prioritized implementation plan outlines short-term fixes (role handling, test coverage) and longer-term ML strategies (few-shot prompts, reward shaping) with estimated effort and risk. The following sections detail the architecture, failure modes, example failures/successes, diagnostics, evaluation, test scenarios, and proposed solutions.
1. Current Architecture and Tool-Selection Pathways

Starbot’s architecture is a multi-tier agent (Fig.1). The Starbot_API is a Fastify TypeScript server with a Prisma/SQLite backend. Under /v1/chats/:chatId/run, it performs the following steps:

    Memory Retrieval: It calls getRelevantContext(userMessage, projectId, workspaceId, N) to fetch top-N semantic memory chunks related to the last user query. The retrieved context (if any) is prepended as a system message. (Failures in retrieval are caught and ignored, continuing with empty memory.)
    Triage Classification: It runs a triage model (runTriage) on the last user message to assign a category/lane/complexity. This maps to a tier (quick/standard/deep) of LLM to use.
    Model Selection: It uses the triage tier (and optional user overrides) to pick a primary LLM model from a catalog of provider models. The model catalog lists many models and their capabilities (text, vision, tools, streaming, etc.). The code then attempts the primary model and fallbacks if failures occur (401/403 or other errors).
    Message Conversion: All persisted chat messages (from the SQLite DB) are converted into LLM format with roles 'user'/'assistant'/'system'. Crucially, the code ignores any ‘tool’ role in messages: it unconditionally casts m.role to one of user|assistant|system. (As noted in the code review: “Messages are cast to provider role union… DB allows tool, so this can leak invalid roles into providers if tooling is added later.”.) No branching logic inspects the user query for tool use at this stage – the entire chat (plus memory) is sent to the LLM as a single prompt.
    LLM Streaming: The API then calls the chosen LLM’s sendChatStream with the assembled message list. Tokens are streamed back via SSE (token.delta events), until the model completes. The final assistant message is saved to the DB, and a message.final SSE event is emitted.

Importantly, no tool calls happen within the API. There are no conditions in the generation route that detect a need for an external tool or execute one. Even though the model catalog marks many models as having the ‘tools’ capability, the current codebase lacks any function that performs (for example) web searches, code execution, or file I/O. The “tools enabled” flag in .env is currently unused in code. In summary, Starbot’s context-tracking path involves memory retrieval and chat history, while its tool path is essentially non-existent except for a placeholder flag.
<figure> ```mermaid flowchart TB subgraph Starbot_API direction LR ChatAPI((/v1/chats/:chatId/run)) --> Triage[Triage Decision (Category/Tier)] Triage --> ModelCatalog["Select Model\n(by tier and prefs)"] Memory[Retrieve Memory Context] --> ModelPrompt[Form Prompt: system(memory)+chat] ChatAPI --> Memory ModelPrompt --> ModelCatalog ModelCatalog --> LLMProvider[LLM Provider\n(e.g. Kimi, Azure, etc.)] LLMProvider --> ModelResponse{"Stream Response"} ModelResponse --> |"token.delta"| ChatAPI ModelResponse --> |"message.final"| ChatAPI ChatAPI --> SQLiteDB[(SQLite DB)] ChatAPI --> User_UI[Web/TUI Clients] end subgraph Clients User[User] -->|UI sends /run| ChatAPI end ``` <figcaption>**Fig. 1.** Starbot architecture: User queries go to Starbot_API (Fastify). The API runs memory retrieval, triage/classification, selects an LLM model, and streams back tokens. (No external tool calls are currently integrated.) *Tool invocation would augment this flow by branching into external tools (web search, code execution, etc.) when needed.*</figcaption> </figure>
2. Likely Failure Modes and Root Causes

Based on the code paths above and repository analysis, we identified several failure modes related to tool invocation and context tracking:

    Unrecognized ‘tool’ role: As noted, the API casts all message roles to user|assistant|system. If any logic or future feature injects a message with role: "tool", it will be silently miscast or dropped. The result is that a user message intended to trigger a tool (e.g. “tool.search: COVID-19 stats”) would be sent to the LLM as a generic user message. The LLM therefore never actually executes the tool. (In practice, Starbot never populates tool roles, but this bug was highlighted as a future correctness issue.)

    No tool-action logic: Without code to handle it, any query requiring an external tool will simply be answered (or hallucinated) by the LLM alone. For example, asking “What’s the latest weather in London?” would rely entirely on the model’s training, producing outdated or incorrect info. Even though models may have huge context windows, they cannot fetch real-time data without a tool. Similarly, a question like “Open the file report.pdf and summarize the second paragraph” has no effect because Starbot_API never reads files. In short, necessary tool calls yield false negatives (needed but not invoked), leading to inaccurate answers.

    Over-invoking tools (false positives): If a future heuristic or classifier incorrectly decides a tool is needed, it might waste time calling APIs when the LLM could have answered on its own. Without safeguards, the bot could, for example, call a web search for a trivial question, slowing down response. (This risk is low in the current code since no tool logic exists; but an aggressive prompt-engineering approach might suffer from it.)

    Context window limits and truncation: The system retrieves only the last ~50 messages plus memory. Very long chats may overflow providers’ context windows, causing older context to be dropped. This can hurt coherence and relevance. Also, if memory retrieval fails (e.g. due to an embedding store error), the API catches it and continues with no memory, silently reducing context.

    Triage errors: The triage model could assign an inappropriately low tier to a hard question (FN) or an overly high tier to an easy one (FP), affecting which model is used. While not directly about tools, mis-triage changes which LLM (with or without tools support) is selected.

    Provider limitations: Some chosen models might support “tools” (e.g. function calling) but the application isn’t using that feature. Even if we fix role casting, we need provider-specific code (e.g. OpenAI function-calling JSON schema) to utilize tools. For example, if a chosen LLM supports function calls, Starbot_API must format queries and parse responses. Without that, the “tools” capability in the model catalog is moot.

In summary, the root cause is that Starbot_API’s code path does not include any logic to decide to invoke an external tool or to actually invoke it. All answers come from the LLM as pure text generation. Any needed tool calls must be detected and executed at the application layer.
3. Code Examples of Correct/Incorrect Tool Use

The repository contains no explicit tool-invocation logic, so our examples focus on evidence of the gap and how it manifests. Below we catalog key code snippets and behaviors:
File path	Code Snippet (context)	Observed Behavior	Explanation of Failure/Success
Starbot_API/src/routes/generation.ts<br> (generation route)	```ts<br>providerMessages.push(...chat.messages.map(m => ({<br> role: m.role as 'user'	'assistant'	'system',<br> content: m.content,<br>})));```
(Hypothetical) Insert Tool Call Logic	ts<br>if (query.includes("web search")) {<br> const result = await WebSearchTool.search(query);<br> providerMessages.push({role: 'assistant', content: result});<br>} else { ... }	Behavior: (Not present in repo; example of intended use) If written, this would call an external search tool when certain keywords are detected.	Why needed: Without code like this, the bot never actually invokes any tools. Testing a query like “Search for X” currently returns a hallucinated answer instead of real search results. Implementing such a branch would improve correctness.

Example 1 above shows the current code. A user message intended to call a tool (if any existed) would be treated as a regular message. There is no “correct usage” example in the existing code, so all tool invocations are currently missing (false negatives). A correct usage would involve detecting the need (e.g. via keywords or structured output) and then calling await toolAPI() and feeding the tool’s output back into the conversation. We must build that logic ourselves.
4. Diagnostic Path for Tool-Invocation Errors

To reproduce and isolate tool-invocation failures, one can follow these steps:

    Set Up Environment: Ensure the API is running with TOOLS_ENABLED=true (and any relevant API keys configured) even though the logic is not yet implemented. This confirms the flag isn’t blocking anything unexpected.

    Prepare Chat Data: Using the WebGUI or TUI, start a new chat. Submit messages that should invoke tools. For example:
        Query requiring real-time data: “What’s the latest stock price of XYZ?”
        A math or code question: “Calculate 12345 * 67890” (which a calculator tool could answer exactly).
        File system ask (if implemented later): “List files in directory /tmp.”

    Observe SSE Events: Connect a client (e.g. browser or curl) to POST /v1/chats/<chatId>/run. Watch the SSE stream. The API will emit status events (triage, model used) and token.delta events. If no tool logic is triggered, you will see only these and finally a message.final.

    Check Provider Responses: Note that the responses will come purely from the chosen LLM. If a tool call was expected, check whether the assistant answer is off (e.g. a plausible guess or an apology).

    Inspect API Logs: Enable debug logging in Starbot_API. The logs around the generation route should show steps like memory retrieval and provider streaming. Confirm that no code path for tool calls is executed.

    Inject Simulated Tool Logic (optional): For deeper debugging, temporarily add code in generation.ts (or another service) to force a tool call. For example, intercept queries containing a special prefix (like “!search”). Run again and see whether the tool is called or if the casting bug mis-handles the message. This helps locate where the logic fails.

    Unit Tests for Message Roles: Create a small test in tests/service (if not present) to verify that a message with role='tool' is handled. This will fail currently, confirming the casting issue.

By following this path, one can confirm that (a) queries needing tools do not currently call them, and (b) the code does not route or transform messages to invoke tools.
5. Evaluation Framework and Metrics

To measure progress in tool invocation and context tracking, we define these evaluation metrics and criteria:

    Tool-Call Precision/Recall (and F1):
        True Positive (TP): A tool was needed for a query and Starbot correctly invoked it.
        False Positive (FP): A tool was invoked when it was not needed.
        False Negative (FN): A tool was needed but not invoked.
        We compute Precision = TP/(TP+FP) and Recall = TP/(TP+FN). For example, if out of 100 queries needing tools Starbot correctly uses tools in 80 (TP), misses 20 (FN), and it erroneously called tools in 10 others (FP), then precision=80/(80+10)=0.89, recall=80/(80+20)=0.80. A high F1 (~2PR/(P+R)) is desired.
    Context Accuracy: For tasks testing contextual understanding (where memory or conversation history matters), measure success rate of correct answers with vs without memory context. This can be binary (correct/incorrect) or graded if partial.
    Latency: Measure the time from user query to final answer. Tool calls will add latency, so we track end-to-end latency (ms) and also overhead per tool call. A target might be e.g. <1s for simple queries, <5s with tools.
    User Satisfaction: Optionally, collect user feedback scores or task success ratings on a survey (1–5 scale) for answers with and without tools. Alternatively, use simulated metrics like “Hallucination rate” (how often the answer contains false info, ideally reduced by tools).
    Resource Usage: Optionally track tokens used or API calls to tools (for cost monitoring).

These metrics can be evaluated on a test suite of prompts (below). For instance, if precision is low (many FP), refine the tool-calling decision logic. If recall is low, improve recall by adding more triggers or fine-tuning the decision model (e.g. via Reinforcement Learning as in TRM).
6. Test Scenarios and Prompts

We propose a set of concrete test prompts that cover different tool categories and context needs. Each scenario specifies whether a tool should be invoked:

    Real-Time Data (Web Search) – Should use Search Tool:
    Prompt: “Who won the soccer World Cup last month?” (assuming the model’s knowledge cutoff is older than last month.)
    Expected: The bot should detect outdated knowledge and query a search tool (or retrieve a recent news source), then reply with the actual answer.
    Metrics: Check if a search API call was made (log) and if the final answer is correct.

    Math/Computation (Calculator) – Should use Calculator Tool:
    Prompt: “What is 23789 * 4123?”
    Expected: The bot should either compute via a calculator tool or answer correctly. A non-tool path would risk arithmetic error.
    Metrics: Verify if a computing API or function was called (or if the model got it right on its own).

    Code Execution – Should use Code/Interpreter Tool:
    Prompt: “Write and run a Python snippet to list even numbers from 1 to 20. What does it output?”
    Expected: The bot should generate code (or retrieve one) and then actually execute it via a code runner, returning 2,4,6,...,20.
    Metrics: Check code-runner logs and correctness of output.

    File System / Database Lookup – Should use File/DB Tool:
    Prompt: “Show me the content of the file /etc/hosts on the server.”
    Expected: The bot should call a file.read("/etc/hosts") tool (sandboxed) and return its content.
    Metrics: Confirm the file I/O was invoked (log) and check answer correctness.

    Chained Task (Memory Retrieval) – Context-Understanding:
    Setup: User has a multi-turn chat where they say earlier: “My favorite color is blue.” Later ask: “What did I say my favorite color was?”
    Expected: The bot should retrieve that memory and answer “You said it was blue.” (No tool needed, but context tracking is tested.)
    Metrics: Correctness of answer (context retrieval success).

    Generic QA – No Tool Needed:
    Prompt: “What is the capital of France?”
    Expected: No external tool needed; the model should answer “Paris” immediately. (This serves as an FP check.)
    Metrics: Ensure the bot does not call any tool (Precision test).

These scenarios should be automated in a test harness. For each, we log whether the correct tools were called and whether the final answer is correct. We can also vary model settings (modes: quick/standard/deep, with/without triage) to test robustness. Metrics like precision/recall and latency are computed across these cases.
7. Recommendations: Code Changes and Strategies

Based on the above, we recommend the following enhancements to Starbot:

    Handle “tool” Messages in Generation: Modify the message conversion in generation.ts to permit and process tool-role messages. For example, if a prior step (see below) adds a message {role: 'tool', content: ...}, the code could insert it as an assistant message or directly call the tool. E.g.:

    ts

    providerMessages.push(...chat.messages.map(m => {
      if (m.role === 'tool') {
        // Insert tool output into conversation as system or assistant message
        return { role: 'assistant', content: String(m.content) };
      }
      return { role: m.role as 'user' | 'assistant' | 'system', content: m.content };
    }));

    This ensures any tool outputs become part of the prompt. (Cite: current bug.)

    Prepend Tool-Calling Logic: Before sending to the LLM, implement a tool-decision step. This could be a simple rule-based check (if query contains keywords or is classified as “requires tool” by triage) or a small classifier model. If a tool is needed, call the appropriate function. Example (pseudo-code):

    ts

    const userQuery = lastUserMsg.content.toLowerCase();
    if (toolsEnabled) {
      if (userQuery.match(/search for/i)) {
        const result = await WebSearchAPI.search(userQuery);
        providerMessages.push({role:'system', content: `Search result: ${result}`});
      }
      // other tools: code execution, file read, etc.
    }

    After calling a tool, include its results in the message list (as above). This is analogous to the MCP idea of having LLM generate a “function call” and then running it.

    Use LLM Function-Calling (if available): If using a provider like OpenAI GPT-4 that supports function calls, define a JSON schema for each tool and let the model output function calls. Implement a handler that intercepts these calls, executes the tool, and feeds the function output back as a message. This follows standard practice (e.g. OpenAI function-calling APIs) and aligns with the MCP standard.

    Prompt Engineering / Few-Shot Examples: Update the system prompt or conversation instructions to remind the model to use tools when appropriate. For example, include an instruction like “You have access to tools such as web search and calculator. If a question requires updated facts or calculation, indicate which tool to use.” Provide a few example interactions (few-shot) that demonstrate tool usage vs non-usage (cf. the ReAct or few-shot paradigms). This can guide the model to yield a special token or phrase when tools are needed.

    Reward Modeling / Fine-Tuning: As suggested by recent research, one could train a small tool-call reward model (TRM) to judge each tool usage step. Integrate it via PPO or similar RL: if the model calls the right tool (and eventually produces the correct answer), give positive reward. This fine-tunes the LLM’s innate decision to use tools. The ICLR 2026 paper shows this improves performance over naive RL. Implementing this is higher effort but could be a final refinement.

    Extend Context Handling: To improve contextual understanding, consider increasing the number of past messages used, or selectively retrieving the most relevant ones via embeddings (already partly done). Ensure that retrieved memory is clearly injected (e.g. as a “System: Here is relevant info: …”). Also handle retrieval failures (perhaps by retry or alert).

    Unit/Integration Tests: Write tests for the tool logic. For example, a unit test could simulate triage(category="search") and verify that a “search” tool is actually called. Also test that messages with role="tool" in the DB result in correct prompts. Automate end-to-end tests like the scenarios above. Ensuring at least 80–90% test coverage on the generation route and any new tool-service modules is critical.

    Logging and Monitoring: Add explicit logging when a tool is invoked, including which tool and the query. This aids debugging and later collecting precision/recall stats. (Reference: MCP security guidelines suggest auditing.)

These changes range from code patches to advanced ML. The code patches (role handling, decision branching) are straightforward and low-risk. The ML strategies (few-shot tuning, RL) require research and careful data gathering, so are mid-to-high risk/effort.
8. Example Pseudocode Patches

Below are illustrative code sketches (not copied verbatim from repo) for key changes:

ts

// In generation.ts, after retrieving chat.messages:
for (const m of chat.messages) {
  if (m.role === 'tool') {
    // Insert tool output into conversation as assistant reply
    providerMessages.push({ role: 'assistant', content: String(m.content) });
  } else {
    providerMessages.push({
      role: m.role as 'user' | 'assistant' | 'system',
      content: m.content,
    });
  }
}

// Example tool-calling logic before streaming:
const userText = lastUserMsg.content;
if (env.TOOLS_ENABLED && /search for|wiki/i.test(userText)) {
  const query = extractQuery(userText);
  const results = await WebSearchAPI.search(query);
  // Record in messages DB with role 'tool':
  await prisma.message.create({ data: { chatId, role: 'tool', content: results } });
  // Also push into prompt:
  providerMessages.push({ role: 'assistant', content: results });
}
// Similar blocks for other tools (code, file, etc.)

For function-calling providers (like Azure OpenAI), one would define the tool schema and use sendChatStream with a function list, handling the tool invocation in the provider adapter layer.
9. Prioritized Implementation Roadmap
Priority	Task	Effort	Risk	Description
1	Fix message role casting bug	Low	Low	Modify generation.ts as above to handle role: "tool". Add unit tests to catch regressions. This ensures that any tool outputs will not be ignored.
1	Add simple tool-decision branch	Low	Med	Implement keyword-based or triage-based logic to call at least a web-search tool (e.g. via DuckDuckGo API). Update prompts to incorporate results. This is relatively quick to implement and yields immediate correctness gains.
2	Prompt Engineering & Few-Shot Examples	Med	Low	Extend system/user prompts to teach tool usage patterns (few-shot or instruction). Test interactively with ChatGPT-like models to refine. Low risk, moderate effort to author examples.
2	Logging and Monitoring	Low	Low	Instrument API to log each tool call and response. Create monitoring dashboards or logs parsing for TP/FP counts. Low effort, improves future analysis.
3	Integrate Additional Tools (calculator, code, file)	Med	Med	Beyond web search, add modules for calculation and code execution (e.g. sandboxed Python). Each requires design (security, sandbox). Risk of errors/abuse.
3	Enhance Memory Retrieval Context	Med	Low	Tune number of retrieval chunks or switch to approximate search. Ensure memory context formatting is clear. Low risk, moderate effort.
4	Fine-tune or RL Train Tool-Invocation Model	High	High	Develop a reward model (like TRM) and apply PPO training so the LLM learns tool usage policies. High effort (data collection, training) but could yield the best precision/recall.
4	Comprehensive Testing and Benchmark Suite	Med	Low	Develop the scenario tests from §6 as automated tests. Measure metrics. Continuous integration (CI) can fail if precision/recall drop. Necessary for regression safety.

Estimates: Lower tasks (1–2) could be done in days to weeks; higher tasks (training RL, full suite) may take months and data. Risks: test suite and simple fixes have minimal risk. ML training can cause model performance shifts (reward-hacking, overfitting) and requires expertise.
10. Diagrams and Tables

Above we included Fig.1 (architecture flow). Below is a decision-flow diagram summarizing how we propose to integrate tool calls:

ToolFlow

Yes, search-related

Yes, math-related

Yes, code-related

No

User Query

Need External Tool?

Invoke Web-Search Tool

Invoke Calculator Tool

Invoke Code-Run Tool

LLM Only

Bot Answers

This diagram shows a high-level logic: the system should classify the query to decide if a tool is needed, dispatch the appropriate tool, feed its output into the LLM prompt, and then generate the final answer.

In conclusion, by closing the gap between detection and execution of tool calls, Starbot can greatly improve answer accuracy and user satisfaction. Metrics-driven testing will ensure we measure improvements (precision/recall of tool use, etc.). Combining code fixes (role handling, tool services) with research-backed techniques (prompting, reward modeling) offers a path to a more robust, context-aware, and capable Starbot agent.

Sources: Starbot repository code and docs; Google Model Context Protocol guide; Toolformer (NeurIPS 2023); ICLR 2026 Tool-Call Reward Model, among others.

Citations · 20
github.com
github.com
cloud.google.com
cloud.google.com
openreview.net
openreview.net

Sources scanned · 20
No sources scanned

Connector sources scanned
No connector sources scanned
