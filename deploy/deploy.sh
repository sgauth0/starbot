#!/bin/bash
# Quick deployment script for Starbot
# Usage: ./deploy.sh

set -e  # Exit on error

echo "🚀 Deploying Starbot to production..."

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

echo -e "${YELLOW}📁 Project root: $PROJECT_ROOT${NC}"

# Step 1: Build API
echo -e "\n${GREEN}1️⃣  Building API...${NC}"
cd "$PROJECT_ROOT/Starbot_API"
npm install
npm run build

if [ ! -f "dist/index.js" ]; then
    echo -e "${RED}❌ API build failed - dist/index.js not found${NC}"
    exit 1
fi

echo -e "${GREEN}✅ API built successfully${NC}"

# Step 2: Build WebGUI
echo -e "\n${GREEN}2️⃣  Building WebGUI...${NC}"
cd "$PROJECT_ROOT/Starbot_WebGUI"
npm install
npm run build

if [ ! -f ".next/standalone/server.js" ]; then
    echo -e "${RED}❌ WebGUI build failed - standalone server not found${NC}"
    echo -e "${YELLOW}ℹ️  Make sure output: 'standalone' is set in next.config.ts${NC}"
    exit 1
fi

echo -e "${GREEN}✅ WebGUI built successfully${NC}"

# Step 3: Check database
echo -e "\n${GREEN}3️⃣  Checking database...${NC}"
cd "$PROJECT_ROOT/Starbot_API"

if [ ! -f "../starbot.db" ]; then
    echo -e "${YELLOW}⚠️  Database not found. Pushing schema...${NC}"
    npx prisma db push
else
    echo -e "${GREEN}✅ Database exists${NC}"
fi

# Step 4: Install systemd services
echo -e "\n${GREEN}4️⃣  Installing systemd services...${NC}"

if [ ! -f "/etc/systemd/system/starbot-api.service" ]; then
    echo -e "${YELLOW}Installing starbot-api.service...${NC}"
    sudo cp "$PROJECT_ROOT/deploy/starbot-api.service" /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable starbot-api
    echo -e "${GREEN}✅ starbot-api.service installed${NC}"
else
    echo -e "${YELLOW}ℹ️  starbot-api.service already exists${NC}"
fi

if [ ! -f "/etc/systemd/system/starbot-webgui.service" ]; then
    echo -e "${YELLOW}Installing starbot-webgui.service...${NC}"
    sudo cp "$PROJECT_ROOT/deploy/starbot-webgui.service" /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable starbot-webgui
    echo -e "${GREEN}✅ starbot-webgui.service installed${NC}"
else
    echo -e "${YELLOW}ℹ️  starbot-webgui.service already exists${NC}"
fi

# Step 5: Install nginx config
echo -e "\n${GREEN}5️⃣  Installing nginx config...${NC}"

if [ ! -f "/etc/nginx/sites-available/starbot.cloud" ]; then
    echo -e "${YELLOW}Installing nginx config...${NC}"
    sudo cp "$PROJECT_ROOT/deploy/nginx-starbot.cloud.conf" /etc/nginx/sites-available/starbot.cloud
    sudo ln -sf /etc/nginx/sites-available/starbot.cloud /etc/nginx/sites-enabled/starbot.cloud

    # Test nginx config
    if sudo nginx -t; then
        echo -e "${GREEN}✅ Nginx config installed${NC}"
    else
        echo -e "${RED}❌ Nginx config test failed${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}ℹ️  Nginx config already exists${NC}"
fi

# Step 6: Restart services
echo -e "\n${GREEN}6️⃣  Restarting services...${NC}"

echo -e "${YELLOW}Restarting starbot-api...${NC}"
sudo systemctl restart starbot-api
sleep 2

if sudo systemctl is-active --quiet starbot-api; then
    echo -e "${GREEN}✅ starbot-api is running${NC}"
else
    echo -e "${RED}❌ starbot-api failed to start${NC}"
    echo -e "${YELLOW}View logs: sudo journalctl -u starbot-api -n 50${NC}"
    exit 1
fi

echo -e "${YELLOW}Restarting starbot-webgui...${NC}"
sudo systemctl restart starbot-webgui
sleep 2

if sudo systemctl is-active --quiet starbot-webgui; then
    echo -e "${GREEN}✅ starbot-webgui is running${NC}"
else
    echo -e "${RED}❌ starbot-webgui failed to start${NC}"
    echo -e "${YELLOW}View logs: sudo journalctl -u starbot-webgui -n 50${NC}"
    exit 1
fi

echo -e "${YELLOW}Reloading nginx...${NC}"
sudo systemctl reload nginx
echo -e "${GREEN}✅ Nginx reloaded${NC}"

# Step 7: Verify deployment
echo -e "\n${GREEN}7️⃣  Verifying deployment...${NC}"

echo -e "${YELLOW}Testing API health...${NC}"
if curl -sf http://localhost:3737/v1/health > /dev/null; then
    echo -e "${GREEN}✅ API health check passed${NC}"
else
    echo -e "${RED}❌ API health check failed${NC}"
fi

echo -e "${YELLOW}Testing WebGUI...${NC}"
if curl -sf http://localhost:3000 > /dev/null; then
    echo -e "${GREEN}✅ WebGUI responding${NC}"
else
    echo -e "${RED}❌ WebGUI not responding${NC}"
fi

# Summary
echo -e "\n${GREEN}🎉 Deployment complete!${NC}"
echo -e "\n${YELLOW}📊 Service Status:${NC}"
sudo systemctl status starbot-api --no-pager -l | head -3
sudo systemctl status starbot-webgui --no-pager -l | head -3

echo -e "\n${YELLOW}🔗 URLs:${NC}"
echo -e "  API:    http://localhost:3737/v1/health"
echo -e "  WebGUI: http://localhost:3000"
echo -e "  Public: http://starbot.cloud (via nginx)"

echo -e "\n${YELLOW}📝 Next Steps:${NC}"
echo -e "  1. Set up SSL: ${GREEN}sudo certbot --nginx -d starbot.cloud -d www.starbot.cloud${NC}"
echo -e "  2. View API logs: ${GREEN}sudo journalctl -u starbot-api -f${NC}"
echo -e "  3. View WebGUI logs: ${GREEN}sudo journalctl -u starbot-webgui -f${NC}"
echo -e "  4. View nginx logs: ${GREEN}sudo tail -f /var/log/nginx/starbot.cloud.access.log${NC}"
