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
API_DIR="$PROJECT_ROOT/Starbot_API"
API_RUNTIME_USER="stella"
API_RUNTIME_GROUP="stella"
API_RUNTIME_ENV_FILE="$API_DIR/.env.runtime"
VERTEX_ADC_RUNTIME_DIR="/home/$API_RUNTIME_USER/.config/starbot"
VERTEX_ADC_RUNTIME_PATH="$VERTEX_ADC_RUNTIME_DIR/vertex-adc.json"
WEBGUI_DEPLOY_ROOT="/var/www/sites/stella/starbot.cloud"
WEBGUI_DEPLOY_DIR="$WEBGUI_DEPLOY_ROOT/Starbot_WebGUI"
WEBGUI_DOCKER_CONTAINER="docklite-site-starbot-web"
USE_DOCKER_WEB=0

echo -e "${YELLOW}📁 Project root: $PROJECT_ROOT${NC}"
echo -e "${YELLOW}📁 WebGUI deploy dir: $WEBGUI_DEPLOY_DIR${NC}"

if command -v docker >/dev/null 2>&1; then
    if sudo docker ps -a --format '{{.Names}}' | grep -Fxq "$WEBGUI_DOCKER_CONTAINER"; then
        USE_DOCKER_WEB=1
        echo -e "${YELLOW}🐳 Detected Docker WebGUI container: $WEBGUI_DOCKER_CONTAINER${NC}"
    fi
fi

# Step 1: Build API
echo -e "\n${GREEN}1️⃣  Building API...${NC}"
if id -u "$API_RUNTIME_USER" >/dev/null 2>&1; then
    echo -e "${YELLOW}Preparing API directory ownership for $API_RUNTIME_USER...${NC}"
    sudo chown -R "$API_RUNTIME_USER:$API_RUNTIME_GROUP" "$API_DIR"
    echo -e "${YELLOW}Building API as $API_RUNTIME_USER...${NC}"
    sudo -u "$API_RUNTIME_USER" bash -lc "cd '$API_DIR' && npm ci && npm run build"
else
    cd "$API_DIR"
    npm ci
    npm run build
fi

if [ ! -f "$API_DIR/dist/index.js" ]; then
    echo -e "${RED}❌ API build failed - dist/index.js not found${NC}"
    exit 1
fi

echo -e "${GREEN}✅ API built successfully${NC}"

# Step 1.5: Prepare provider runtime environment overrides
echo -e "\n${GREEN}1️⃣.5  Preparing provider runtime config...${NC}"
sudo rm -f "$API_RUNTIME_ENV_FILE"

if [ -f "$API_DIR/.env" ]; then
    ADC_SOURCE_PATH="$(sudo -u "$API_RUNTIME_USER" bash -lc "
      cd '$API_DIR'
      set -a
      # shellcheck disable=SC1091
      source ./.env >/dev/null 2>&1 || true
      set +a
      printf '%s' \"\${GOOGLE_APPLICATION_CREDENTIALS:-}\"
    ")"

    if [ -n "$ADC_SOURCE_PATH" ] && [ -f "$ADC_SOURCE_PATH" ]; then
        echo -e "${YELLOW}Preparing readable Vertex ADC for $API_RUNTIME_USER...${NC}"
        sudo mkdir -p "$VERTEX_ADC_RUNTIME_DIR"
        sudo cp "$ADC_SOURCE_PATH" "$VERTEX_ADC_RUNTIME_PATH"
        sudo chown "$API_RUNTIME_USER:$API_RUNTIME_GROUP" "$VERTEX_ADC_RUNTIME_PATH"
        sudo chmod 600 "$VERTEX_ADC_RUNTIME_PATH"

        {
            echo "GOOGLE_APPLICATION_CREDENTIALS=$VERTEX_ADC_RUNTIME_PATH"
        } | sudo tee "$API_RUNTIME_ENV_FILE" >/dev/null

        sudo chown "$API_RUNTIME_USER:$API_RUNTIME_GROUP" "$API_RUNTIME_ENV_FILE"
        sudo chmod 600 "$API_RUNTIME_ENV_FILE"
        echo -e "${GREEN}✅ Runtime provider override prepared${NC}"
    else
        echo -e "${YELLOW}ℹ️  No readable GOOGLE_APPLICATION_CREDENTIALS source found; skipping override${NC}"
    fi
else
    echo -e "${YELLOW}ℹ️  No API .env file found; skipping provider runtime override${NC}"
fi

# Step 2: Sync + Build WebGUI in /var/www
echo -e "\n${GREEN}2️⃣  Syncing and building WebGUI...${NC}"

if ! id -u starbot >/dev/null 2>&1; then
    echo -e "${YELLOW}Creating system user 'starbot'...${NC}"
    sudo useradd -r -s /usr/sbin/nologin starbot || true
fi

sudo mkdir -p "$WEBGUI_DEPLOY_DIR"
sudo rsync -a --delete \
    --exclude node_modules \
    --exclude .next \
    --exclude .git \
    "$PROJECT_ROOT/Starbot_WebGUI/" \
    "$WEBGUI_DEPLOY_DIR/"
sudo chown -R starbot:starbot "$WEBGUI_DEPLOY_ROOT"

echo -e "${YELLOW}Building WebGUI at $WEBGUI_DEPLOY_DIR as starbot...${NC}"
sudo -u starbot bash -lc "cd '$WEBGUI_DEPLOY_DIR' && rm -rf .next && npm ci && npm run build"

if [ ! -f "$WEBGUI_DEPLOY_DIR/.next/standalone/server.js" ]; then
    echo -e "${RED}❌ WebGUI build failed - standalone server not found${NC}"
    echo -e "${YELLOW}ℹ️  Make sure output: 'standalone' is set in next.config.ts${NC}"
    exit 1
fi

BUILD_ID_FILE="$WEBGUI_DEPLOY_DIR/.next/BUILD_ID"
STANDALONE_BUILD_ID_FILE="$WEBGUI_DEPLOY_DIR/.next/standalone/.next/BUILD_ID"
if [ -f "$BUILD_ID_FILE" ] && [ -f "$STANDALONE_BUILD_ID_FILE" ]; then
    BUILD_ID="$(cat "$BUILD_ID_FILE")"
    STANDALONE_BUILD_ID="$(cat "$STANDALONE_BUILD_ID_FILE")"
    if [ "$BUILD_ID" != "$STANDALONE_BUILD_ID" ]; then
        echo -e "${RED}❌ WebGUI build output mismatch${NC}"
        echo -e "${YELLOW}BUILD_ID (.next): $BUILD_ID${NC}"
        echo -e "${YELLOW}BUILD_ID (standalone): $STANDALONE_BUILD_ID${NC}"
        echo -e "${YELLOW}Refusing to publish mixed assets/server output.${NC}"
        exit 1
    fi
fi

echo -e "${YELLOW}Publishing WebGUI runtime files to $WEBGUI_DEPLOY_ROOT...${NC}"
sudo rm -rf \
    "$WEBGUI_DEPLOY_ROOT/.next" \
    "$WEBGUI_DEPLOY_ROOT/node_modules" \
    "$WEBGUI_DEPLOY_ROOT/public"
sudo rm -f \
    "$WEBGUI_DEPLOY_ROOT/server.js" \
    "$WEBGUI_DEPLOY_ROOT/package.json" \
    "$WEBGUI_DEPLOY_ROOT/package-lock.json"
sudo mkdir -p "$WEBGUI_DEPLOY_ROOT/.next/static" "$WEBGUI_DEPLOY_ROOT/public"

sudo rsync -a "$WEBGUI_DEPLOY_DIR/.next/standalone/" "$WEBGUI_DEPLOY_ROOT/"
sudo rsync -a "$WEBGUI_DEPLOY_DIR/.next/static/" "$WEBGUI_DEPLOY_ROOT/.next/static/"
sudo rsync -a "$WEBGUI_DEPLOY_DIR/public/" "$WEBGUI_DEPLOY_ROOT/public/"
sudo cp "$WEBGUI_DEPLOY_DIR/package.json" "$WEBGUI_DEPLOY_ROOT/package.json"
if [ -f "$WEBGUI_DEPLOY_DIR/package-lock.json" ]; then
    sudo cp "$WEBGUI_DEPLOY_DIR/package-lock.json" "$WEBGUI_DEPLOY_ROOT/package-lock.json"
fi
sudo chown -R starbot:starbot "$WEBGUI_DEPLOY_ROOT"

echo -e "${GREEN}✅ WebGUI built successfully${NC}"

# Step 3: Check database
echo -e "\n${GREEN}3️⃣  Checking database...${NC}"
if id -u "$API_RUNTIME_USER" >/dev/null 2>&1; then
    echo -e "${YELLOW}Syncing schema as $API_RUNTIME_USER...${NC}"
    sudo -u "$API_RUNTIME_USER" bash -lc "
      cd '$API_DIR'
      if [ -f .env ]; then
        set -a
        # shellcheck disable=SC1091
        source ./.env
        set +a
      fi
      DATABASE_URL=\"\${DATABASE_URL:-file:../starbot.db}\"
      DATABASE_URL=\"\$DATABASE_URL\" npx prisma db push
    "
else
    cd "$API_DIR"
    if [ -f ".env" ]; then
        set -a
        # shellcheck disable=SC1091
        source ./.env
        set +a
    fi
    DATABASE_URL="${DATABASE_URL:-file:../starbot.db}"
    DATABASE_URL="$DATABASE_URL" npx prisma db push
fi
echo -e "${GREEN}✅ Database schema synced${NC}"

# Step 4: Install systemd services
echo -e "\n${GREEN}4️⃣  Installing systemd services...${NC}"

echo -e "${YELLOW}Syncing starbot-api.service...${NC}"
sudo cp "$PROJECT_ROOT/deploy/starbot-api.service" /etc/systemd/system/

if [ "$USE_DOCKER_WEB" -eq 0 ]; then
    echo -e "${YELLOW}Syncing starbot-webgui.service...${NC}"
    sudo cp "$PROJECT_ROOT/deploy/starbot-webgui.service" /etc/systemd/system/
fi

sudo systemctl daemon-reload
sudo systemctl enable starbot-api
if [ "$USE_DOCKER_WEB" -eq 0 ]; then
    sudo systemctl enable starbot-webgui
fi
echo -e "${GREEN}✅ systemd unit files synced${NC}"

# Step 5: Install nginx config
echo -e "\n${GREEN}5️⃣  Installing nginx config...${NC}"

echo -e "${YELLOW}Syncing nginx config...${NC}"
sudo cp "$PROJECT_ROOT/deploy/nginx-starbot.cloud.conf" /etc/nginx/sites-available/starbot.cloud
sudo ln -sf /etc/nginx/sites-available/starbot.cloud /etc/nginx/sites-enabled/starbot.cloud

# Sync Docklite Caddy legacy route when present (this host serves starbot.cloud via Caddy).
if [ -d "/home/docklite-new-new/caddy/legacy" ] && [ -f "$PROJECT_ROOT/deploy/caddy-starbot-systemd.caddy" ]; then
    echo -e "${YELLOW}Syncing Docklite Caddy route...${NC}"
    sudo cp "$PROJECT_ROOT/deploy/caddy-starbot-systemd.caddy" \
      /home/docklite-new-new/caddy/legacy/starbot-systemd.caddy
fi

# Test nginx config
if sudo nginx -t; then
    echo -e "${GREEN}✅ Nginx config synced${NC}"
else
    echo -e "${RED}❌ Nginx config test failed${NC}"
    exit 1
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

if [ "$USE_DOCKER_WEB" -eq 1 ]; then
    echo -e "${YELLOW}Restarting Docker WebGUI container ($WEBGUI_DOCKER_CONTAINER)...${NC}"
    sudo systemctl disable --now starbot-webgui >/dev/null 2>&1 || true
    sudo docker restart "$WEBGUI_DOCKER_CONTAINER" >/dev/null
    if sudo docker ps --format '{{.Names}}' | grep -Fxq "$WEBGUI_DOCKER_CONTAINER"; then
        echo -e "${GREEN}✅ Docker WebGUI container is running${NC}"
    else
        echo -e "${RED}❌ Docker WebGUI container failed to start${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}Restarting starbot-webgui...${NC}"
    if sudo ss -ltnp | grep -q ':3001 '; then
        echo -e "${YELLOW}Port 3001 is already in use. Releasing stale listener...${NC}"
        sudo fuser -k 3001/tcp || true
        sleep 1
    fi
    sudo systemctl restart starbot-webgui
    sleep 2

    if sudo systemctl is-active --quiet starbot-webgui; then
        echo -e "${GREEN}✅ starbot-webgui is running${NC}"
    else
        echo -e "${RED}❌ starbot-webgui failed to start${NC}"
        echo -e "${YELLOW}View logs: sudo journalctl -u starbot-webgui -n 50${NC}"
        exit 1
    fi
fi

if sudo docker ps -a --format '{{.Names}}' | grep -Fxq "docklite_caddy"; then
    echo -e "${YELLOW}Reloading Docklite Caddy...${NC}"
    sudo docker restart docklite_caddy >/dev/null
    if ! sudo docker inspect docklite_caddy --format '{{json .NetworkSettings.Networks}}' | grep -q 'docklite_network'; then
        echo -e "${YELLOW}Docklite Caddy not attached to docklite_network. Reconnecting...${NC}"
        sudo docker network connect docklite_network docklite_caddy || true
    fi
    echo -e "${GREEN}✅ Docklite Caddy restarted${NC}"
fi

echo -e "${YELLOW}Reloading nginx...${NC}"
if [ "$USE_DOCKER_WEB" -eq 1 ]; then
    echo -e "${YELLOW}Docker WebGUI mode detected; stopping nginx to avoid conflicts with Docklite Caddy on :80/:443...${NC}"
    sudo systemctl stop nginx >/dev/null 2>&1 || true
    echo -e "${GREEN}✅ nginx stopped (Caddy owns public ports)${NC}"
else
    if sudo systemctl is-active --quiet nginx; then
        sudo systemctl reload nginx
        echo -e "${GREEN}✅ Nginx reloaded${NC}"
    else
        echo -e "${YELLOW}⚠️  nginx service is not active. Attempting to start nginx...${NC}"
        if sudo systemctl start nginx; then
            echo -e "${GREEN}✅ Nginx started${NC}"
        else
            echo -e "${YELLOW}⚠️  Could not start nginx. Continuing with API/WebGUI deployment.${NC}"
        fi
    fi
fi

# Step 7: Verify deployment
echo -e "\n${GREEN}7️⃣  Verifying deployment...${NC}"

echo -e "${YELLOW}Testing API health...${NC}"
if curl -sf http://localhost:3737/v1/health > /dev/null; then
    echo -e "${GREEN}✅ API health check passed${NC}"
else
    echo -e "${RED}❌ API health check failed${NC}"
fi

echo -e "${YELLOW}Testing WebGUI...${NC}"
if [ "$USE_DOCKER_WEB" -eq 1 ]; then
    if curl -sf -H 'Host: starbot.cloud' http://127.0.0.1 > /dev/null; then
        echo -e "${GREEN}✅ WebGUI responding via Docker Caddy${NC}"
    else
        echo -e "${RED}❌ WebGUI not responding via Docker Caddy${NC}"
    fi
else
    if curl -sf http://localhost:3001 > /dev/null; then
        echo -e "${GREEN}✅ WebGUI responding${NC}"
    else
        echo -e "${RED}❌ WebGUI not responding${NC}"
    fi
fi

# Summary
echo -e "\n${GREEN}🎉 Deployment complete!${NC}"
echo -e "\n${YELLOW}📊 Service Status:${NC}"
sudo systemctl status starbot-api --no-pager -l | head -3
if [ "$USE_DOCKER_WEB" -eq 1 ]; then
    sudo docker ps --format 'table {{.Names}}\t{{.Status}}' | grep -E "NAMES|$WEBGUI_DOCKER_CONTAINER"
else
    sudo systemctl status starbot-webgui --no-pager -l | head -3
fi

echo -e "\n${YELLOW}🔗 URLs:${NC}"
echo -e "  API:    http://localhost:3737/v1/health"
if [ "$USE_DOCKER_WEB" -eq 1 ]; then
    echo -e "  WebGUI: https://starbot.cloud (via Docker Caddy)"
else
    echo -e "  WebGUI: http://localhost:3001"
    echo -e "  Public: http://starbot.cloud (via nginx)"
fi

echo -e "\n${YELLOW}📝 Next Steps:${NC}"
echo -e "  1. Set up SSL: ${GREEN}sudo certbot --nginx -d starbot.cloud -d www.starbot.cloud${NC}"
echo -e "  2. View API logs: ${GREEN}sudo journalctl -u starbot-api -f${NC}"
if [ "$USE_DOCKER_WEB" -eq 1 ]; then
    echo -e "  3. View WebGUI logs: ${GREEN}sudo docker logs -f $WEBGUI_DOCKER_CONTAINER${NC}"
else
    echo -e "  3. View WebGUI logs: ${GREEN}sudo journalctl -u starbot-webgui -f${NC}"
fi
echo -e "  4. View nginx logs: ${GREEN}sudo tail -f /var/log/nginx/starbot.cloud.access.log${NC}"
