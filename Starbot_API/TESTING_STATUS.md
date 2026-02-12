# Starbot_API Testing Status

## ✅ Current Status

**API Running**: http://127.0.0.1:3737

```
🧠 Starbot API listening on http://127.0.0.1:3737
📊 Health: http://127.0.0.1:3737/health

Configuration:
  Environment: development
  Server: 127.0.0.1:3737
  Configured providers: none ⚠️
  Tools enabled: true
  Web search enabled: false
  Triage model enabled: false
```

### Health Check
```bash
curl http://127.0.0.1:3737/health
# ✅ {"status":"ok","timestamp":"...","version":"1.0.0"}
```

### Database
```bash
curl http://127.0.0.1:3737/v1/projects
# ✅ Returns existing test project
```

---

## ⚠️ No Providers Configured

The API is running but **cannot generate responses** yet because no LLM providers are configured.

**Next Step**: Add at least one provider API key.

---

## 🔑 How to Add Provider Keys

### Option 1: Kimi (Moonshot) - Easiest

1. **Get API Key**: Sign up at https://platform.moonshot.cn/
2. **Add to .env**:
   ```bash
   nano /var/www/sites/stella/imterminally.online/Starbot_API/.env
   ```
3. **Set key**:
   ```env
   MOONSHOT_API_KEY=sk-your-key-here
   ```
4. **Restart API**:
   ```bash
   pkill -f "node.*Starbot_API"
   cd /var/www/sites/stella/imterminally.online/Starbot_API
   node dist/index.js > /tmp/starbot.log 2>&1 &
   ```
5. **Verify**:
   ```bash
   tail /tmp/starbot.log
   # Should show: "Configured providers: kimi"
   ```

### Option 2: Copy from Starlander

If you have keys in the main starlander installation:

```bash
# Copy Kimi key
grep MOONSHOT_API_KEY /var/www/sites/stella/imterminally.online/starlander/apps/api/.env >> /var/www/sites/stella/imterminally.online/Starbot_API/.env

# Copy Vertex keys
grep VERTEX_ /var/www/sites/stella/imterminally.online/starlander/apps/api/.env >> /var/www/sites/stella/imterminally.online/Starbot_API/.env

# Copy Azure keys
grep AZURE_ /var/www/sites/stella/imterminally.online/starlander/apps/api/.env >> /var/www/sites/stella/imterminally.online/Starbot_API/.env

# Restart API
pkill -f "node.*Starbot_API" && sleep 1
cd /var/www/sites/stella/imterminally.online/Starbot_API
node dist/index.js > /tmp/starbot.log 2>&1 &
```

---

## 🧪 Run Test Harness (Once Provider is Configured)

```bash
cd /var/www/sites/stella/imterminally.online/Starbot_API
./test-api.sh
```

**Expected Output**:
- ✅ Health check passes
- ✅ Create project/chat/message
- ✅ **Real LLM streaming response** (from Kimi/other provider)
- ✅ Message saved to database

---

## 🚀 Quick Test (Manual)

Once a provider is configured:

```bash
# 1. Create a chat
CHAT_ID=$(curl -s -X POST http://127.0.0.1:3737/v1/projects/ce409fe9-70c8-46f1-b3da-312d99bbe65e/chats \
  -H "Content-Type: application/json" \
  -d '{"title":"Quick Test"}' | jq -r '.chat.id')

echo "Chat ID: $CHAT_ID"

# 2. Add a user message
curl -X POST http://127.0.0.1:3737/v1/chats/$CHAT_ID/messages \
  -H "Content-Type: application/json" \
  -d '{"role":"user","content":"Hello! Can you write a haiku about coding?"}' | jq '.'

# 3. Stream a response
curl -N -X POST http://127.0.0.1:3737/v1/chats/$CHAT_ID/run \
  -H "Content-Type: application/json" \
  -d '{"mode":"standard"}'
```

**Expected Output**:
```
event: status
data: {"message":"Running triage..."}

event: status
data: {"message":"Routing (WRITE_REWRITE/standard, complexity: 2)..."}

event: status
data: {"message":"Using Kimi K2 (kimi)..."}

event: token.delta
data: {"text":"Coding"}

event: token.delta
data: {"text":" flows"}

... (more tokens)

event: message.final
data: {"id":"...","content":"Coding flows like rain\nBugs emerge in silent night\nDebug brings the dawn","provider":"kimi","model":"Kimi K2","usage":{...},"triage":{...}}
```

---

## 🐛 Troubleshooting

### "No models available"
- **Cause**: No providers configured
- **Fix**: Add API key to `.env` and restart

### "Provider 'kimi' is not available"
- **Cause**: Invalid API key
- **Fix**: Check key in `.env`, verify it works at https://platform.moonshot.cn/

### "Port 3737 already in use"
```bash
# Kill old instance
pkill -f "node.*Starbot_API"
# Or by port:
lsof -i :3737 | tail -n +2 | awk '{print $2}' | xargs kill -9
```

### Check logs
```bash
tail -f /tmp/starbot.log
```

---

## 📊 Current State

- [x] API compiled successfully
- [x] API running on port 3737
- [x] Health endpoint works
- [x] Database works
- [x] CRUD endpoints work
- [ ] **Provider configured** ⚠️ (Need API key)
- [ ] Generation tested with real LLM

**Next**: Add provider API key, then test generation!
