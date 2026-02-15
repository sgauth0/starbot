# Starbot Production Deployment - Quick Start

## TL;DR - One Command Deployment

```bash
cd ~/projects/starbot/deploy
./deploy.sh
```

This script will:
1. ✅ Build API and WebGUI
2. ✅ Set up database
3. ✅ Install systemd services
4. ✅ Configure nginx
5. ✅ Start everything
6. ✅ Verify deployment

---

## Manual Deployment (Step by Step)

### 1. Build Applications

```bash
# Build API
cd ~/projects/starbot/Starbot_API
npm install
npm run build

# Build WebGUI
cd ~/projects/starbot/Starbot_WebGUI
npm install
npm run build
```

### 2. Configure Environment

```bash
# Create API .env file
cd ~/projects/starbot/Starbot_API
cat > .env << 'EOF'
NODE_ENV=production
PORT=3737
HOST=127.0.0.1
DATABASE_URL=file:../starbot.db
OPENAI_API_KEY=sk-your-key-here
EOF

chmod 600 .env
```

### 3. Set Up Services

```bash
# Copy systemd files
sudo cp ~/projects/starbot/deploy/*.service /etc/systemd/system/

# Reload and enable
sudo systemctl daemon-reload
sudo systemctl enable starbot-api starbot-webgui
sudo systemctl start starbot-api starbot-webgui
```

### 4. Configure Nginx

```bash
# Copy nginx config
sudo cp ~/projects/starbot/deploy/nginx-starbot.cloud.conf \
  /etc/nginx/sites-available/starbot.cloud

# Enable site
sudo ln -s /etc/nginx/sites-available/starbot.cloud \
  /etc/nginx/sites-enabled/starbot.cloud

# Test and reload
sudo nginx -t && sudo systemctl reload nginx
```

### 5. Set Up SSL (Optional but Recommended)

```bash
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d starbot.cloud -d www.starbot.cloud
```

---

## Verify Everything Works

```bash
# Check services
sudo systemctl status starbot-api
sudo systemctl status starbot-webgui

# Test endpoints
curl http://localhost:3737/v1/health
curl http://localhost:3000

# Test via domain
curl http://starbot.cloud/v1/health
```

---

## Common Commands

```bash
# View logs
sudo journalctl -u starbot-api -f
sudo journalctl -u starbot-webgui -f

# Restart services
sudo systemctl restart starbot-api
sudo systemctl restart starbot-webgui
sudo systemctl reload nginx

# Update after code changes
cd ~/projects/starbot
git pull
cd Starbot_API && npm run build
cd ../Starbot_WebGUI && npm run build
sudo systemctl restart starbot-api starbot-webgui
```

---

## Architecture Overview

```
Internet → Nginx (port 80/443)
             ├─→ /v1/* → API (localhost:3737)
             └─→ /* → WebGUI (localhost:3000)
```

**Ports:**
- **80/443** - Nginx (public)
- **3000** - Next.js WebGUI (localhost only)
- **3737** - Fastify API (localhost only)

**Files:**
- `/etc/nginx/sites-enabled/starbot.cloud` - Nginx config
- `/etc/systemd/system/starbot-api.service` - API service
- `/etc/systemd/system/starbot-webgui.service` - WebGUI service
- `/home/stella/projects/starbot/starbot.db` - SQLite database
- `/home/stella/projects/starbot/Starbot_API/.env` - API secrets

---

## Troubleshooting

### Services won't start
```bash
sudo journalctl -u starbot-api -n 100
sudo journalctl -u starbot-webgui -n 100
```

### Port conflicts
```bash
sudo lsof -i :3737  # Check API port
sudo lsof -i :3000  # Check WebGUI port
```

### 502 Bad Gateway
- Check if services are running: `sudo systemctl status starbot-*`
- Check nginx logs: `sudo tail -f /var/log/nginx/starbot.cloud.error.log`

### Database errors
```bash
# Rebuild database
cd ~/projects/starbot/Starbot_API
npx prisma db push
```

---

## Security Checklist

- [ ] SSL certificate installed (run certbot)
- [ ] `.env` file permissions set to 600
- [ ] Services only listen on localhost (not 0.0.0.0)
- [ ] Firewall allows 80/443, blocks 3000/3737
- [ ] API keys stored in .env (never in code)

---

For detailed deployment guide, see [DEPLOYMENT.md](./DEPLOYMENT.md)
