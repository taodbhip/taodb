# 自行部署

运行你自己的 taoDB 服务器。你的数据、你的服务器、你的 API token。

## 方式一：Docker Compose（推荐）

```bash
# 下载 docker-compose.yml
curl -O https://raw.githubusercontent.com/taodbhip/taodb/main/docker-compose.yml

# 设置管理员 token（可选，默认 tk_admin）
export TAODB_ADMIN_TOKEN=your-secure-token

# 启动
docker-compose up -d

# 验证
curl http://localhost:8765/health
```

数据存储在 Docker volume（`taodb-data`）中。容器重启或升级不会丢失。

备份：

```bash
docker run --rm -v taodb-data:/data -v $(pwd):/backup alpine tar czf /backup/taodb-backup.tar.gz -C /data .
```

## 方式二：独立二进制

```bash
# 安装
curl -fsSL https://raw.githubusercontent.com/taodbhip/taodb/main/install.sh | bash

# 启动服务
taodb serve --addr :8765 --data ./taodb-data --admin-token your-token

# 后台运行
nohup taodb serve --addr :8765 --data ./taodb-data > taodb.log 2>&1 &
```

## 启动后配置

创建用户和项目：

```bash
ADMIN_TOKEN="tk_admin"
SERVER="http://localhost:8765"

# 创建用户
curl -s -X POST $SERVER/v1/users \
  -H "x-api-token: $ADMIN_TOKEN" \
  -d '{"user_id":"me","email":"me@example.com"}'

# 响应中包含你的 API token。保存它。
# {"user_id":"me","api_token":"tk_xxxx_xxxx",...}

# 创建项目
curl -s -X POST $SERVER/v1/projects \
  -H "x-api-token: tk_xxxx_xxxx" \
  -d '{"project_id":"myproject","name":"My Project"}'
```

## 配合 Agent 使用

服务器启动后：

**MCP agent（Claude Code、Cursor、Windsurf）：**
不需要配置服务器。MCP agent 用本地 stdio ——项目目录里 `taodb init`，重启 agent 即可。服务器是给 HTTP 访问用的。

**HTTP 访问（移动端、Web 应用、非 MCP 客户端）：**

```bash
# 召回记忆
curl -X POST $SERVER/v1/recall \
  -H "x-api-token: tk_xxxx_xxxx" \
  -H "x-project-id: myproject" \
  -d '{"query":"auth模块","within_days":30}'

# 存入记忆
curl -X POST $SERVER/v1/memories \
  -H "x-api-token: tk_xxxx_xxxx" \
  -H "x-project-id: myproject" \
  -d '{"text":"修复了 token refresh 的 bug"}'
```

## 生产建议

**HTTPS：** 在前面加 nginx 或 Caddy：

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

用 Caddy（自动 HTTPS）：

```
taodb.yourdomain.com {
    reverse_proxy localhost:8765
}
```

**备份：** 定期备份数据目录。Docker 见上方备份命令。独立部署用 rsync 备份 `taodb-data/`。

**监控：** `/health` 接口返回 `{"status":"ok"}`。接入 uptime 监控。

**安全：**
- 用 nginx/Caddy 做反向代理，不直接暴露 taoDB 端口
- 设置强 `TAODB_ADMIN_TOKEN`
- 限制 API 访问 IP（可选）

---

返回 [文档](index.md) | [API 参考](api-reference.md)
