# Starbot Production Deployment Guide

Deploy Starbot to `/var/www/sites/stella/starbot.cloud`

## Prerequisites

- Ubuntu/Debian server with sudo access
- Node.js 20+ installed
- Nginx installed
- Domain `starbot.cloud` pointing to server

## Step 1: Build the Applications

### Build API

```bash
cd ~/projects/starbot/Starbot_API

# Install dependencies
npm install --production=false

# Build TypeScript to JavaScript
npm run build

# Install production dependencies only
npm install --production

# Verify build
ls -la dist/
```

### Build WebGUI

```bash
cd ~/projects/starbot/Starbot_WebGUI

# Install dependencies
npm install

# Build for production with standalone output
npm run build

# The standalone server will be at: .next/standalone/server.js
```

**Note:** If standalone mode isn't enabled, update `next.config.ts`:

```typescript
const nextConfig: NextConfig = {
  reactCompiler: true,
  output: 'standalone',  // Add this line
};
```

Then rebuild.

## Step 2: Configure Environment Variables

### API Environment

```bash
cd ~/projects/starbot/Starbot_API

# Create production .env file
cat > .env << 'EOF'
NODE_ENV=production
PORT=3737
HOST=127.0.0.1

# Database
DATABASE_URL=file:../starbot.db

# OpenAI (optional, for embeddings)
OPENAI_API_KEY=sk-your-key-here

# Add other provider keys as needed
# ANTHROPIC_API_KEY=
# GOOGLE_API_KEY=
EOF

# Secure the file
chmod 600 .env
```

### WebGUI Environment

The WebGUI uses `NEXT_PUBLIC_API_URL` which should be set in the systemd service file (already configured to use `https://starbot.cloud/v1`).

## Step 3: Update CORS Configuration

Update API to allow the production domain:

```bash
cd ~/projects/starbot/Starbot_API

# Edit src/index.ts and add your domain to CORS origins
# Then rebuild:
npm run build
```

## Step 4: Set Up Nginx

```bash
# Copy nginx config
sudo cp ~/projects/starbot/deploy/nginx-starbot.cloud.conf \
  /etc/nginx/sites-available/starbot.cloud

# Create symlink
sudo ln -s /etc/nginx/sites-available/starbot.cloud \
  /etc/nginx/sites-enabled/starbot.cloud

# Test nginx config
sudo nginx -t

# Reload nginx
sudo systemctl reload nginx
```

## Step 5: Set Up Systemd Services

```bash
# Copy service files
sudo cp ~/projects/starbot/deploy/starbot-api.service \
  /etc/systemd/system/starbot-api.service

sudo cp ~/projects/starbot/deploy/starbot-webgui.service \
  /etc/systemd/system/starbot-webgui.service

# Reload systemd
sudo systemctl daemon-reload

# Enable services (start on boot)
sudo systemctl enable starbot-api
sudo systemctl enable starbot-webgui

# Start services
sudo systemctl start starbot-api
sudo systemctl start starbot-webgui

# Check status
sudo systemctl status starbot-api
sudo systemctl status starbot-webgui
```

## Step 6: Verify Deployment

```bash
# Check API health
curl http://localhost:3737/v1/health

# Check WebGUI
curl http://localhost:3000

# Check via nginx
curl http://starbot.cloud/v1/health
curl http://starbot.cloud
```

## Step 7: Set Up SSL with Let's Encrypt (Recommended)

```bash
# Install certbot
sudo apt install certbot python3-certbot-nginx

# Get SSL certificate
sudo certbot --nginx -d starbot.cloud -d www.starbot.cloud

# Certbot will automatically update nginx config
# Test renewal
sudo certbot renew --dry-run
```

## Step 8: Initialize Database

```bash
cd ~/projects/starbot/Starbot_API

# Run Prisma migrations (if using migrations)
npx prisma migrate deploy

# Or push schema (development mode)
npx prisma db push

# Verify database
ls -la ../starbot.db
```

## Useful Commands

### View Logs

```bash
# API logs
sudo journalctl -u starbot-api -f

# WebGUI logs
sudo journalctl -u starbot-webgui -f

# Nginx logs
sudo tail -f /var/log/nginx/starbot.cloud.access.log
sudo tail -f /var/log/nginx/starbot.cloud.error.log
```

### Restart Services

```bash
# Restart API
sudo systemctl restart starbot-api

# Restart WebGUI
sudo systemctl restart starbot-webgui

# Reload nginx
sudo systemctl reload nginx
```

### Update Application

```bash
# 1. Pull latest code
cd ~/projects/starbot
git pull

# 2. Rebuild API
cd Starbot_API
npm install
npm run build

# 3. Rebuild WebGUI
cd ../Starbot_WebGUI
npm install
npm run build

# 4. Restart services
sudo systemctl restart starbot-api
sudo systemctl restart starbot-webgui
```

## Troubleshooting

### Service won't start

```bash
# Check service status
sudo systemctl status starbot-api
sudo systemctl status starbot-webgui

# View detailed logs
sudo journalctl -u starbot-api -n 100 --no-pager
sudo journalctl -u starbot-webgui -n 100 --no-pager
```

### Port already in use

```bash
# Check what's using the port
sudo lsof -i :3737
sudo lsof -i :3000

# Kill the process
sudo kill <PID>
```

### Database locked

```bash
# Check database permissions
ls -la ~/projects/starbot/starbot.db

# Fix permissions
chmod 644 ~/projects/starbot/starbot.db
chown stella:stella ~/projects/starbot/starbot.db
```

### CORS errors

Make sure API CORS configuration includes your production domain. Edit `Starbot_API/src/index.ts`:

```typescript
await server.register(cors, {
  origin: [
    'http://localhost:8080',
    'http://127.0.0.1:8080',
    'http://localhost:3000',
    'http://127.0.0.1:3000',
    'https://starbot.cloud',        // Add production domain
    'https://www.starbot.cloud',    // Add www variant
  ],
  credentials: true,
});
```

Then rebuild and restart API.

## Security Checklist

- [ ] SSL certificate installed (HTTPS)
- [ ] Environment files have restricted permissions (600)
- [ ] Database file has restricted permissions (644)
- [ ] API only listens on localhost (not 0.0.0.0)
- [ ] Nginx rate limiting configured (if needed)
- [ ] Firewall configured (allow 80, 443; block 3000, 3737)
- [ ] Regular backups of starbot.db
- [ ] API keys in .env files (never in code)

## Performance Optimization

### Enable Gzip in Nginx

Add to nginx config:

```nginx
gzip on;
gzip_vary on;
gzip_types text/plain text/css application/json application/javascript text/xml application/xml text/javascript;
```

### Enable Caching

Already configured for `_next/static` files. Monitor and adjust as needed.

### Database Backups

```bash
# Create backup script
cat > ~/backup-starbot.sh << 'EOF'
#!/bin/bash
DATE=$(date +%Y%m%d_%H%M%S)
cp ~/projects/starbot/starbot.db ~/backups/starbot_$DATE.db
# Keep only last 7 days
find ~/backups -name "starbot_*.db" -mtime +7 -delete
EOF

chmod +x ~/backup-starbot.sh

# Add to crontab (daily at 2 AM)
crontab -e
# Add: 0 2 * * * /home/stella/backup-starbot.sh
```

## Monitoring

Consider setting up:
- **PM2** as alternative to systemd (auto-restart, clustering)
- **Prometheus + Grafana** for metrics
- **Sentry** for error tracking
- **Uptime monitoring** (UptimeRobot, etc.)
