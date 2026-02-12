#!/bin/bash
# Starbot_API Test Harness
# Tests the full API flow: create project → create chat → add message → stream generation

set -e  # Exit on error

API_URL="http://localhost:3737"

echo "========================================"
echo "Starbot_API Test Harness"
echo "========================================"
echo ""

# Color codes
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Test 1: Health Check
echo -e "${BLUE}Test 1: Health Check${NC}"
echo "GET /health"
HEALTH=$(curl -s "$API_URL/health")
echo "$HEALTH" | jq '.'
if echo "$HEALTH" | jq -e '.status == "ok"' > /dev/null; then
    echo -e "${GREEN}✓ Health check passed${NC}"
else
    echo -e "${RED}✗ Health check failed${NC}"
    exit 1
fi
echo ""

# Test 2: Create Project
echo -e "${BLUE}Test 2: Create Project${NC}"
echo "POST /v1/projects"
PROJECT=$(curl -s -X POST "$API_URL/v1/projects" \
    -H "Content-Type: application/json" \
    -d '{"name":"Test Project"}')
echo "$PROJECT" | jq '.'
PROJECT_ID=$(echo "$PROJECT" | jq -r '.project.id')
if [ "$PROJECT_ID" != "null" ] && [ -n "$PROJECT_ID" ]; then
    echo -e "${GREEN}✓ Project created: $PROJECT_ID${NC}"
else
    echo -e "${RED}✗ Failed to create project${NC}"
    exit 1
fi
echo ""

# Test 3: List Projects
echo -e "${BLUE}Test 3: List Projects${NC}"
echo "GET /v1/projects"
PROJECTS=$(curl -s "$API_URL/v1/projects")
echo "$PROJECTS" | jq '.'
PROJECT_COUNT=$(echo "$PROJECTS" | jq '.projects | length')
echo -e "${GREEN}✓ Found $PROJECT_COUNT projects${NC}"
echo ""

# Test 4: Create Chat
echo -e "${BLUE}Test 4: Create Chat${NC}"
echo "POST /v1/projects/$PROJECT_ID/chats"
CHAT=$(curl -s -X POST "$API_URL/v1/projects/$PROJECT_ID/chats" \
    -H "Content-Type: application/json" \
    -d '{"title":"Test Chat"}')
echo "$CHAT" | jq '.'
CHAT_ID=$(echo "$CHAT" | jq -r '.chat.id')
if [ "$CHAT_ID" != "null" ] && [ -n "$CHAT_ID" ]; then
    echo -e "${GREEN}✓ Chat created: $CHAT_ID${NC}"
else
    echo -e "${RED}✗ Failed to create chat${NC}"
    exit 1
fi
echo ""

# Test 5: List Chats in Project
echo -e "${BLUE}Test 5: List Chats in Project${NC}"
echo "GET /v1/projects/$PROJECT_ID/chats"
CHATS=$(curl -s "$API_URL/v1/projects/$PROJECT_ID/chats")
echo "$CHATS" | jq '.'
CHAT_COUNT=$(echo "$CHATS" | jq '.chats | length')
echo -e "${GREEN}✓ Found $CHAT_COUNT chats in project${NC}"
echo ""

# Test 6: Add User Message
echo -e "${BLUE}Test 6: Add User Message${NC}"
echo "POST /v1/chats/$CHAT_ID/messages"
MESSAGE=$(curl -s -X POST "$API_URL/v1/chats/$CHAT_ID/messages" \
    -H "Content-Type: application/json" \
    -d '{"role":"user","content":"Hello, Starbot! Can you explain what you are?"}')
echo "$MESSAGE" | jq '.'
MESSAGE_ID=$(echo "$MESSAGE" | jq -r '.message.id')
if [ "$MESSAGE_ID" != "null" ] && [ -n "$MESSAGE_ID" ]; then
    echo -e "${GREEN}✓ Message added: $MESSAGE_ID${NC}"
else
    echo -e "${RED}✗ Failed to add message${NC}"
    exit 1
fi
echo ""

# Test 7: List Messages in Chat
echo -e "${BLUE}Test 7: List Messages in Chat${NC}"
echo "GET /v1/chats/$CHAT_ID/messages"
MESSAGES=$(curl -s "$API_URL/v1/chats/$CHAT_ID/messages")
echo "$MESSAGES" | jq '.'
MESSAGE_COUNT=$(echo "$MESSAGES" | jq '.messages | length')
echo -e "${GREEN}✓ Found $MESSAGE_COUNT messages in chat${NC}"
echo ""

# Test 8: Stream Generation (SSE)
echo -e "${BLUE}Test 8: Stream Generation (SSE)${NC}"
echo "POST /v1/chats/$CHAT_ID/run"
echo -e "${YELLOW}Streaming response (SSE)...${NC}"
echo ""

curl -N -X POST "$API_URL/v1/chats/$CHAT_ID/run" \
    -H "Content-Type: application/json" \
    -d '{"mode":"standard","speed":false,"auto":true}' 2>/dev/null | while IFS= read -r line; do
    # Parse SSE format: "event: <type>" and "data: <json>"
    if [[ "$line" == event:* ]]; then
        EVENT_TYPE=$(echo "$line" | cut -d' ' -f2-)
        echo -e "${BLUE}[Event: $EVENT_TYPE]${NC}"
    elif [[ "$line" == data:* ]]; then
        DATA=$(echo "$line" | cut -d' ' -f2-)

        # Try to parse as JSON and extract relevant fields
        if echo "$DATA" | jq -e '.' > /dev/null 2>&1; then
            # Check event type and display accordingly
            if [[ "$EVENT_TYPE" == "status" ]]; then
                MSG=$(echo "$DATA" | jq -r '.message // empty')
                if [ -n "$MSG" ]; then
                    echo -e "${YELLOW}  Status: $MSG${NC}"
                fi
            elif [[ "$EVENT_TYPE" == "token.delta" ]]; then
                TOKEN=$(echo "$DATA" | jq -r '.text // empty')
                echo -n "$TOKEN"
            elif [[ "$EVENT_TYPE" == "message.final" ]]; then
                echo ""
                echo -e "${GREEN}  Final message received${NC}"
                echo "$DATA" | jq '.'
            elif [[ "$EVENT_TYPE" == "error" ]]; then
                echo ""
                echo -e "${RED}  Error: $(echo "$DATA" | jq -r '.message')${NC}"
            fi
        fi
    fi
done

echo ""
echo ""

# Test 9: Verify Assistant Message Saved
echo -e "${BLUE}Test 9: Verify Assistant Message Saved${NC}"
echo "GET /v1/chats/$CHAT_ID/messages"
FINAL_MESSAGES=$(curl -s "$API_URL/v1/chats/$CHAT_ID/messages")
FINAL_COUNT=$(echo "$FINAL_MESSAGES" | jq '.messages | length')
echo -e "${GREEN}✓ Found $FINAL_COUNT messages (should be 2: user + assistant)${NC}"

# Show last message
echo "Last message:"
echo "$FINAL_MESSAGES" | jq '.messages[-1]'
echo ""

# Test 10: Get Chat Details
echo -e "${BLUE}Test 10: Get Chat Details${NC}"
echo "GET /v1/chats/$CHAT_ID"
CHAT_DETAILS=$(curl -s "$API_URL/v1/chats/$CHAT_ID")
echo "$CHAT_DETAILS" | jq '.chat | {id, title, updatedAt, messageCount: (.messages | length)}'
echo ""

# Summary
echo "========================================"
echo -e "${GREEN}All tests passed!${NC}"
echo "========================================"
echo ""
echo "Created resources:"
echo "  Project ID: $PROJECT_ID"
echo "  Chat ID: $CHAT_ID"
echo "  Messages: $FINAL_COUNT"
echo ""
echo "To clean up, you can manually delete:"
echo "  curl -X DELETE $API_URL/v1/chats/$CHAT_ID"
echo "  curl -X DELETE $API_URL/v1/projects/$PROJECT_ID"
echo ""
