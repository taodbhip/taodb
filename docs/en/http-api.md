# HTTP API (Advanced / Optional)

> **Most users do not need this.** The default mode is MCP stdio — agent discovers taoDB via `.mcp.json` and communicates over local stdin/stdout. Zero configuration.
>
> HTTP mode is for: non-MCP agent integrations, remote access, custom tooling, or multi-user server deployments.

## Starting the HTTP Server

```bash
taodb serve --addr :8765 --data ./taodb-memory --admin-token YOUR_TOKEN
```

This starts taoDB as an HTTP server on port 8765. All endpoints require authentication.

## Authentication

All endpoints except `/health` require an API token.

**User endpoints:** Pass token via `Authorization: Bearer YOUR_TOKEN` or `x-api-token: YOUR_TOKEN`. Memory operations also need `x-project-id: your-project` header.

**Admin endpoints** (user/project creation): Use the admin token from `--admin-token`.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check |
| `POST` | `/v1/memories` | Store a memory |
| `GET` | `/v1/recent?n=10` | Recent N memories |
| `POST` | `/v1/recall` | Recall by time window + energy |
| `DELETE` | `/v1/memories/:id` | Delete a memory |
| `POST` | `/v1/decay` | Trigger energy decay |
| `GET` | `/v1/stats` | Storage statistics |
| `POST` | `/v1/recall/constraints` | Constraint-layer recall |
| `POST` | `/v1/recall/sensory` | Sensory-triggered recall |
| `POST` | `/v1/recall/narrative` | Narrative recall (person/location/object filter) |
| `POST` | `/v1/users` | Create user (admin token) |
| `GET` | `/v1/users` | List users (admin token) |
| `POST` | `/v1/projects` | Create project |
| `GET` | `/v1/projects` | List projects |

## Examples

Store a memory:

```bash
curl -X POST http://localhost:8765/v1/memories \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "x-project-id: myproject" \
  -H "Content-Type: application/json" \
  -d '{"text": "Key event description"}'
```

Recall:

```bash
curl -X POST http://localhost:8765/v1/recall \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "x-project-id: myproject" \
  -H "Content-Type: application/json" \
  -d '{"query": "authentication", "top_k": 10, "within_days": 30, "min_energy": 0.3}'
```

Narrative recall with filtering:

```json
{
  "persons": ["桑安歌"],
  "locations": ["邯郸"],
  "narrative_span_days": 30,
  "top_k": 10,
  "dimensions": ["天", "地"]
}
```

---

Back to [MCP Tools Reference](api-reference.md).
