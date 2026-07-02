# Self-Host Deployment

Run your own taoDB server. Your data, your server, your API token.

## Option 1: Docker Compose (Recommended)

```bash
# Download docker-compose.yml
curl -O https://raw.githubusercontent.com/taodbhip/taodb/main/docker-compose.yml

# Set admin token (optional, defaults to tk_admin)
export TAODB_ADMIN_TOKEN=your-secure-token

# Start
docker-compose up -d

# Verify
curl http://localhost:8765/health
```

Data is stored in a Docker volume (`taodb-data`). It persists across container restarts and upgrades.

To back up:

```bash
docker run --rm -v taodb-data:/data -v $(pwd):/backup alpine tar czf /backup/taodb-backup.tar.gz -C /data .
```

## Option 2: Standalone Binary

```bash
# Install
curl -fsSL https://raw.githubusercontent.com/taodbhip/taodb/main/install.sh | bash

# Start server
taodb serve --addr :8765 --data ./taodb-data --admin-token your-token

# Background with systemd, launchd, or nohup
nohup taodb serve --addr :8765 --data ./taodb-data > taodb.log 2>&1 &
```

## Setup After Starting

Create a user and project:

```bash
ADMIN_TOKEN="tk_admin"
SERVER="http://localhost:8765"

# Create user
curl -s -X POST $SERVER/v1/users \
  -H "x-api-token: $ADMIN_TOKEN" \
  -d '{"user_id":"me","email":"me@example.com"}'

# The response includes your API token. Save it.
# {"user_id":"me","api_token":"tk_xxxx_xxxx",...}

# Create project
curl -s -X POST $SERVER/v1/projects \
  -H "x-api-token: tk_xxxx_xxxx" \
  -d '{"project_id":"myproject","name":"My Project"}'
```

## Using With Your Agent

Once the server is running:

**MCP agents (Claude Code, Cursor, Windsurf):**
You don't need to configure the server at all. MCP agents use local stdio — just run `taodb init` in your project directory and restart. The server is for HTTP access only.

**HTTP access (mobile apps, web apps, non-MCP clients):**

```bash
# Recall memories
curl -X POST $SERVER/v1/recall \
  -H "x-api-token: tk_xxxx_xxxx" \
  -H "x-project-id: myproject" \
  -d '{"query":"auth module","within_days":30}'

# Store a memory
curl -X POST $SERVER/v1/memories \
  -H "x-api-token: tk_xxxx_xxxx" \
  -H "x-project-id: myproject" \
  -d '{"text":"Fixed the token refresh bug"}'
```

## Production Notes

**HTTPS:** Put nginx or Caddy in front:

```nginx
server {
    listen 443 ssl;
    server_name taodb.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/taodb.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/taodb.yourdomain.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:8765;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

With Caddy (automatic HTTPS):

```
taodb.yourdomain.com {
    reverse_proxy localhost:8765
}
```

**Backup:** Back up the data volume or directory regularly. For Docker, see the backup command above. For standalone, rsync `taodb-data/` to your backup target.

**Monitoring:** The `/health` endpoint returns `{"status":"ok"}`. Hook it into uptime monitoring.

**Firewall:** If exposing directly to the internet, limit access:
- Use nginx/Caddy as reverse proxy
- Or use iptables/ufw to restrict incoming connections
- Set a strong `TAODB_ADMIN_TOKEN`

---

Back to [Documentation](index.md) | [API Reference](api-reference.md)
