# HTTP API（高级 / 可选）

> **大多数用户不需要这个。** 默认模式是 MCP stdio——agent 通过 `.mcp.json` 发现 taoDB，走本地标准输入输出通信。零配置。
>
> HTTP 模式用于：非 MCP agent 集成、远程访问、自定义工具、多用户服务器部署。

## 启动 HTTP 服务

```bash
taodb serve --addr :8765 --data ./taodb-memory --admin-token YOUR_TOKEN
```

## 认证

除 `/health` 外所有接口需要认证。

**用户接口：** 通过 `Authorization: Bearer YOUR_TOKEN` 或 `x-api-token: YOUR_TOKEN` 传递 token。记忆操作需要同时传递 `x-project-id: your-project`。

**管理员接口：** 使用 `--admin-token` 指定的 token。

## 接口列表

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/health` | 健康检查 |
| `POST` | `/v1/memories` | 写入记忆 |
| `GET` | `/v1/recent?n=10` | 最近 N 条 |
| `POST` | `/v1/recall` | 时空召回 |
| `DELETE` | `/v1/memories/:id` | 删除记忆 |
| `POST` | `/v1/decay` | 触发衰减 |
| `GET` | `/v1/stats` | 存储统计 |
| `POST` | `/v1/recall/constraints` | 约束层召回 |
| `POST` | `/v1/recall/sensory` | 感官触发召回 |
| `POST` | `/v1/recall/narrative` | 叙事层召回 |
| `POST` | `/v1/users` | 创建用户（管理员） |
| `GET` | `/v1/users` | 列出用户（管理员） |
| `POST` | `/v1/projects` | 创建项目 |
| `GET` | `/v1/projects` | 列出项目 |

## 示例

写入记忆：

```bash
curl -X POST http://localhost:8765/v1/memories \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "x-project-id: myproject" \
  -H "Content-Type: application/json" \
  -d '{"text": "关键事件描述"}'
```

召回：

```bash
curl -X POST http://localhost:8765/v1/recall \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "x-project-id: myproject" \
  -H "Content-Type: application/json" \
  -d '{"query": "认证", "top_k": 10, "within_days": 30, "min_energy": 0.3}'
```

---

返回 [MCP 工具参考](api-reference.md)。
