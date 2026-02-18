# Tool Usage Guide for Starbot

## Overview

Starbot now supports dynamic tool calling. The LLM can automatically decide when and how to use available tools during conversation. This guide shows how tools work and how to configure them.

## Available Tools

### 1. Web Search (`web_search`)

**Purpose:** Search the web for current information and recent events

**When to use:** Information queries, current events, recent developments

**Example requests:**
- "What's the latest news about AI?"
- "Search for the population of Tokyo"
- "Find recent updates on climate change"

**Tool Parameters:**
```json
{
  "query": "search query string",        // Required
  "num_results": 5                       // Optional (1-10, default: 5)
}
```

**Configuration:**
```bash
WEB_SEARCH_ENABLED=true
BRAVE_SEARCH_API_KEY=your_api_key_here
```

**Response Format:**
```json
{
  "query": "latest AI news",
  "results": [
    {
      "rank": 1,
      "title": "AI News Title",
      "url": "https://example.com",
      "snippet": "Result snippet..."
    }
  ]
}
```

---

### 2. Calculator (`calculator`)

**Purpose:** Perform mathematical calculations

**When to use:** Math problems, complex calculations, formulas

**Example requests:**
- "What is 2 + 2?"
- "Calculate the square root of 144"
- "Solve sin(pi/2) + cos(0)"
- "What's 50% of 1000?"

**Tool Parameters:**
```json
{
  "expression": "2 + 2 * sin(pi)"  // Required - mathematical expression
}
```

**Supported Operations:**
- Arithmetic: `+`, `-`, `*`, `/`, `%`, `^` (power)
- Functions: `sin`, `cos`, `tan`, `sqrt`, `log`, `exp`, `abs`, etc.
- Constants: `pi`, `e`, `phi`
- Complex expressions: `(5 + 3) * 2 - sqrt(16)`

**Configuration:**
```bash
TOOLS_ENABLED=true  # Always enabled
```

**Response Format:**
```json
{
  "expression": "sin(pi/2)",
  "result": "1"
}
```

---

### 3. Code Execution (`execute_code`)

**Purpose:** Execute Python or JavaScript code in a sandboxed environment

**When to use:** Data processing, quick scripts, algorithm testing

**Example requests:**
- "Execute this Python code: print(list(range(10)))"
- "Run a JavaScript function that calculates fibonacci"
- "Process this data with Python"

**Tool Parameters:**
```json
{
  "language": "python",          // Required: "python" or "javascript"
  "code": "print('hello')"        // Required: code to execute
}
```

**Configuration:**
```bash
TOOLS_ENABLED=true
CODE_EXECUTION_ENABLED=true    # ⚠️ Disabled by default - explicit opt-in required
```

⚠️ **SECURITY WARNING:** Code execution is disabled by default. Only enable in trusted environments.

**Limitations:**
- 5 second timeout
- 100KB max output
- No network access
- No file system write access (only read temporary files)

**Response Format:**
```json
{
  "language": "python",
  "output": "hello\nworld",
  "truncated": false
}
```

---

### 4. File Read (`read_file`)

**Purpose:** Read file contents from the workspace

**When to use:** Code review, configuration reading, documentation reference

**Example requests:**
- "Show me the contents of package.json"
- "Read the README file"
- "What does the main.ts file contain?"

**Tool Parameters:**
```json
{
  "file_path": "src/main.ts",        // Required: relative path
  "max_lines": 100                   // Optional (1-500, default: 500)
}
```

**Configuration:**
```bash
TOOLS_ENABLED=true  # Always enabled
```

**Response Format:**
```json
{
  "file_path": "package.json",
  "lines": ["line 1", "line 2", ...],
  "total_lines": 42,
  "truncated": false,
  "line_count": 42
}
```

---

## How Tool Calling Works

### 1. User Sends Message
```
User: "What is the current bitcoin price?"
```

### 2. System Processes Message
- Message sent to LLM
- Available tools included in LLM context
- LLM sees tool definitions

### 3. LLM Decides to Use Tool
```
LLM: "I need to search for current Bitcoin price information"
→ Calls: web_search(query="bitcoin price today")
```

### 4. Tool Executes
- Web search performs search
- Results returned as structured data
- Results added back to conversation

### 5. LLM Generates Final Response
```
LLM: "Based on the search results, Bitcoin is currently..."
```

### 6. Response Sent to User
```
User receives: Final response with current Bitcoin price
```

---

## Multi-Turn Tool Usage

The LLM can use multiple tools in sequence:

```
User: "Search for the population of Tokyo and calculate 10% of it"

LLM:
1. Call: web_search(query="Tokyo population")
   → Result: Tokyo population is 14 million
2. Call: calculator(expression="14000000 * 0.1")
   → Result: 1400000

Final Response: "Tokyo has a population of 14 million. 10% of that is 1.4 million people."
```

---

## Tool Execution Loop Details

The system implements up to **5 iterations** of tool execution:

```
Iteration 1:
├─ LLM processes initial message with tools
├─ LLM calls: tool_A(args)
├─ Tool executes → returns result
└─ Result added to conversation

Iteration 2:
├─ LLM sees tool result
├─ LLM calls: tool_B(args)
├─ Tool executes → returns result
└─ Result added to conversation

Iteration 3:
├─ LLM sees all previous results
├─ LLM generates final response
└─ Loop exits (no more tool calls)

Result: Final response to user
```

The loop automatically exits when:
- LLM finishes with final response (no more tool calls)
- Maximum iterations (5) reached
- Tool execution fails and LLM decides to stop

---

## Event Streaming

When tools are used, the client receives SSE events:

### Tool Start Event
```json
{
  "type": "tool.start",
  "data": {
    "tool_call_id": "call_12345",
    "tool_name": "web_search",
    "arguments": "{\"query\": \"AI news\"}"
  }
}
```

### Tool End Event
```json
{
  "type": "tool.end",
  "data": {
    "tool_call_id": "call_12345",
    "tool_name": "web_search",
    "success": true,
    "duration_ms": 250,
    "preview": "Search completed with 5 results..."
  }
}
```

### Token Delta Events
```json
{
  "type": "token.delta",
  "data": {
    "text": "The latest AI news is..."
  }
}
```

### Final Message Event
```json
{
  "type": "message.final",
  "data": {
    "id": "msg_123",
    "role": "assistant",
    "content": "Full response here...",
    "provider": "azure",
    "model": "gpt-4",
    "usage": {
      "promptTokens": 100,
      "completionTokens": 250,
      "totalTokens": 350
    }
  }
}
```

---

## Configuration Examples

### Minimal Setup (Calculator Only)
```bash
TOOLS_ENABLED=true
WEB_SEARCH_ENABLED=false
CODE_EXECUTION_ENABLED=false
```

### Standard Setup (Recommended)
```bash
TOOLS_ENABLED=true
WEB_SEARCH_ENABLED=true
BRAVE_SEARCH_API_KEY=your_key_here
CODE_EXECUTION_ENABLED=false
```

### Full Setup (Development Only)
```bash
TOOLS_ENABLED=true
WEB_SEARCH_ENABLED=true
BRAVE_SEARCH_API_KEY=your_key_here
CODE_EXECUTION_ENABLED=true  # ⚠️ Security risk - use carefully
```

### Disabled
```bash
TOOLS_ENABLED=false
```

---

## Best Practices

### For Users
1. **Be specific** in tool-requiring queries
   - ✅ "What's the current Bitcoin price?" (will trigger web search)
   - ❌ "Tell me about Bitcoin" (might not trigger search)

2. **Use multi-turn** for complex queries
   - ✅ "Search for Python 3.12 features, then explain the main one"
   - ❌ "Everything about Python"

3. **Let LLM decide** tool usage
   - The LLM automatically detects when tools are needed
   - You don't need to explicitly request tool usage

### For Developers
1. **Monitor tool execution**
   - Check SSE events for tool performance
   - Monitor `duration_ms` to detect slow tools

2. **Error handling**
   - Tool failures don't crash the generation
   - LLM can recover or report errors

3. **Rate limiting**
   - Consider implementing per-tool rate limits for expensive operations
   - Monitor total tool execution time per request

4. **Security**
   - Only enable code execution in controlled environments
   - Validate file paths in file read tool
   - Monitor tool usage in production

---

## Troubleshooting

### Tool Not Being Called
**Problem:** LLM isn't using tools when expected

**Solution:**
1. Verify `TOOLS_ENABLED=true`
2. Check if required tools are registered (see startup logs)
3. Ensure tool parameters are correct in request
4. Try explicitly mentioning what you need (e.g., "search for...")

### Tool Execution Timeout
**Problem:** Tool takes longer than 5 seconds

**Solution:**
- Only affects code execution tool
- Reduce code complexity
- Use calculator for math instead of code execution

### Web Search Not Working
**Problem:** Web search returns error

**Solution:**
1. Verify `WEB_SEARCH_ENABLED=true`
2. Check `BRAVE_SEARCH_API_KEY` is set correctly
3. Verify API key is valid and has quota

### File Read Returns Empty
**Problem:** File path not found

**Solution:**
1. Use relative paths from workspace root
2. Verify file exists
3. Check file path doesn't contain `..` (path traversal blocked)

---

## Future Enhancements

Planned improvements for the tool system:

- [ ] Function calling support for Vertex AI (Gemini)
- [ ] Function calling support for AWS Bedrock
- [ ] Tool parameter validation with Zod schemas
- [ ] Per-tool rate limiting
- [ ] Custom user-defined tools
- [ ] Tool composition (tools calling other tools)
- [ ] Async/long-running tool support
- [ ] Tool result caching
- [ ] Tool usage analytics

---

## Questions?

For more information, see:
- `IMPLEMENTATION_SUMMARY.md` - Technical implementation details
- `VERIFICATION_CHECKLIST.md` - Verification status
- `specs/SPEC5_TOOLUSAGE.md` - Original specification
- Source code: `Starbot_API/src/services/tools/`
