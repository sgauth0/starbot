# Tool Usage Testing Guide

## Quick Verification

### 1. Build Check
```bash
cd Starbot_API
npm run build
```
Expected: No errors, clean TypeScript compilation

### 2. Run Tests
```bash
npm test
```
Expected: 28 tests passing, including 4 tool registry tests

### 3. Start Server
```bash
npm run dev
```
Expected output:
```
Initializing tool system...
✓ Web search tool registered (if enabled)
✓ Calculator tool registered
✓ File read tool registered
Tool system initialized with 3 tool(s)
Registered tools: calculator, read_file, web_search
🧠 Starbot API listening on http://127.0.0.1:3737
```

## Manual Testing

### Prerequisites
1. Start API: `npm run dev` in Starbot_API
2. Get a running LLM provider (Azure, Vertex, Bedrock, etc.)
3. Create a project and chat via API

### Test Scenarios

#### Test 1: Calculator Tool
**Request:**
```bash
curl -X POST http://localhost:3737/v1/chats/{chatId}/run \
  -H "Content-Type: application/json" \
  -d '{"message": "What is 1234 * 5678?"}'
```

**Expected Flow:**
1. LLM sees calculator tool available
2. LLM calls: `calculator` with expression "1234 * 5678"
3. API executes and returns: 7006652
4. LLM incorporates result: "The answer is 7,006,652"

**Check SSE Events:**
- `tool.start` with tool_name: "calculator"
- `token.delta` with final answer
- `tool.end` with success: true

#### Test 2: File Read Tool
**Request:**
```bash
curl -X POST http://localhost:3737/v1/chats/{chatId}/run \
  -H "Content-Type: application/json" \
  -d '{"message": "Show me the first 20 lines of package.json"}'
```

**Expected Flow:**
1. LLM calls: `read_file` with file_path: "package.json", max_lines: 20
2. API reads and returns file content
3. LLM shows the file content

#### Test 3: Web Search Tool (requires API key)
**Prerequisite:** Set `WEB_SEARCH_ENABLED=true` and `BRAVE_SEARCH_API_KEY=...`

**Request:**
```bash
curl -X POST http://localhost:3737/v1/chats/{chatId}/run \
  -H "Content-Type: application/json" \
  -d '{"message": "What is the latest news about Claude AI?"}'
```

**Expected Flow:**
1. LLM calls: `web_search` with query
2. API searches and returns results
3. LLM synthesizes answer from results

#### Test 4: Code Execution Tool
**Prerequisite:** Set `CODE_EXECUTION_ENABLED=true`

**Request:**
```bash
curl -X POST http://localhost:3737/v1/chats/{chatId}/run \
  -H "Content-Type: application/json" \
  -d '{"message": "Write Python code to list numbers 1 to 10 and run it"}'
```

**Expected Flow:**
1. LLM calls: `execute_code` with language: "python", code: "for i in range(1, 11): print(i)"
2. API executes and captures output
3. LLM shows: "1\n2\n3\n...\n10"

#### Test 5: Multi-Tool Usage
**Request:**
```bash
curl -X POST http://localhost:3737/v1/chats/{chatId}/run \
  -H "Content-Type: application/json" \
  -d '{"message": "Calculate 25% of 8 million"}'
```

**Expected Flow:**
1. LLM calls: `calculator` with "0.25 * 8000000"
2. API returns: 2000000
3. LLM shows answer: "25% of 8 million is 2,000,000"

## Monitoring Tool Execution

### Enable Debug Logging
```bash
LOG_LEVEL=debug npm run dev
```

This shows:
- Tool registration on startup
- Tool calls being made
- Tool execution results

### Check SSE Events

Use browser DevTools or curl to observe SSE stream:

```bash
curl -N -H "Accept: text/event-stream" \
  -X POST http://localhost:3737/v1/chats/{chatId}/run \
  -H "Content-Type: application/json" \
  -d '{"message": "Query that uses tools"}' | grep -E "(tool\.|message\.final)"
```

Expected events:
```
data: {"type":"tool.start","tool_call_id":"call_123","tool_name":"calculator"}
data: {"type":"tool.end","tool_call_id":"call_123","tool_name":"calculator","success":true,"duration_ms":45}
data: {"type":"message.final","content":"...","provider":"azure"}
```

## Troubleshooting

### Tools Not Being Called

**Check 1:** Verify tools are registered on startup
```
Initializing tool system...
✓ Calculator tool registered
✓ File read tool registered
Tool system initialized with 2 tool(s)
```

**Check 2:** Verify TOOLS_ENABLED=true in .env
```bash
grep TOOLS_ENABLED Starbot_API/.env
```

**Check 3:** Use a model with 'tools' capability
Good choices:
- GPT-4.1 (Azure)
- Gemini 2.5 Flash (Vertex)
- Claude 3.5 Sonnet (Bedrock)

### Tool Execution Fails

**Calculator errors:** Check expression syntax
- Good: "1234 * 5678", "sqrt(16)", "sin(pi/2)"
- Bad: "1234 x 5678", "sqrt[16]"

**File read errors:** Check file exists and path is relative
- Good: "package.json", "src/index.ts"
- Bad: "/etc/passwd", "../../../etc/passwd"

**Code execution timeout:** Code must complete in 5 seconds
- Avoid: infinite loops, long operations
- Good: simple calculations, list operations

**Web search fails:** Check API key is set
```bash
grep BRAVE_SEARCH_API_KEY Starbot_API/.env
```

## Performance Testing

### Single Tool Call
Expected: 500ms - 2 seconds (depends on tool)

### Multiple Tool Calls (2-3)
Expected: 2 - 5 seconds

### Tool with Long Output
Expected: 1 - 3 seconds (depends on file/search size)

### Code Execution (5s timeout)
Expected: Up to 5 seconds per execution

## Test Results Checklist

After running tests:
- [ ] `npm run build` completes without errors
- [ ] `npm test` shows 28/28 passing
- [ ] `npm run dev` shows tool system initialization
- [ ] Calculator tool executes correctly
- [ ] File read tool executes correctly
- [ ] Web search tool works (if enabled)
- [ ] Code execution tool works (if enabled)
- [ ] SSE events stream correctly
- [ ] Tool results appear in final response
- [ ] Error handling works (invalid tool, bad arguments)

## Reporting Issues

If tools don't work as expected:

1. Check startup logs for tool registration
2. Enable debug logging: `LOG_LEVEL=debug npm run dev`
3. Verify environment variables are set
4. Try with a known-working LLM (GPT-4.1)
5. Check browser console for SSE stream errors
6. Verify tool parameters are valid JSON

Report with:
- LLM model used
- Tool name that failed
- Request message
- Debug logs
- Expected vs actual result
