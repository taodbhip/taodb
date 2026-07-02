# 常见问题

## 基本概念

**Q: TaoDB 和向量数据库有什么区别？**

向量数据库（Pinecone、Qdrant、Weaviate）按语义相似度索引。你存 embedding，按"找相似向量"查询。TaoDB 按时间和空间索引。你存带时间戳和容器标签的记忆，按"auth 模块上个月发生了什么"查询。

向量数据库回答"什么跟 X 相似"。TaoDB 回答"什么时候在哪里发生了什么"。它们是互补关系，不是竞争关系。

**Q: TaoDB 是搜索引擎吗？**

不是。TaoDB 没有全文搜索、没有 BM25 排序、没有基于内容的相关性评分。LLM 读原始记忆自己判断相关性。TaoDB 只负责在正确的时间提供正确的记忆子集。

**Q: TaoDB 内部使用 embedding 或 LLM 吗？**

不用。TaoDB 是纯存储引擎。使用 redb（嵌入式 B-tree）、bincode 序列化、CRC32 完整性校验。没有 embedding、没有 LLM 调用、没有外部 API 依赖。

**Q: 没用 LLM agent 能用 TaoDB 吗？**

可以通过 HTTP API 从任何应用调用。但 TaoDB 设计目标是给 LLM 消费——记忆是原始文本，由 AI 读取和理解。当通用数据库用可以，但偏离了设计意图。

**Q: 这跟直接用 SQLite 加时间戳有什么区别？**

TaoDB 在原始存储之上加了多层：
- 能量地板模型 + 自动衰减
- 多维召回评分（时间 × 空间 × 能量 × 身体/情感 × 物件）
- 约束层 vs 叙事层分离
- 感官跨索引
- 召回重新巩固增强
- MCP 协议集成

你可以在 SQLite 上自己建这些。TaoDB 把它们打包成一个目的明确的引擎。

## 安装与配置

**Q: 需要安装 Rust 吗？**

不需要。安装脚本下载预编译二进制。无需 Rust 工具链。

**Q: macOS 提示"无法验证开发者"怎么办？**

TaoDB 二进制经过临时签名但未公证（公证需要 Apple Developer 账号）。在 Finder 中右键点击二进制 → 打开，或运行：

```bash
xattr -d com.apple.quarantine /usr/local/bin/taodb
```

**Q: TaoDB 会联网吗？**

运行时零网络调用。二进制零遥测、零分析、零网络依赖。只读写本地文件。（安装脚本会调 GitHub API 查最新版本号，但那是安装脚本的行为，不是 taoDB 本身。）

**Q: 能在 CI/CD 中用吗？**

可以。它是单个二进制、零依赖，适合 CI 环境。在 CI pipeline 中用安装脚本或直接从 GitHub Releases 下载。

## 使用

**Q: TaoDB 能存多少条记忆？**

嵌入式 redb 数据库可扩展到百万级记录。启动时内存缓存加载所有记忆——性能取决于可用 RAM。1 万条记忆（每条约 10KB），内存约 100MB。10 万条约 1GB。

**Q: 什么时候该跑衰减？**

重要里程碑后：sprint 结束、卷完结、每月一次。不是每次会话。衰减是批量操作，不是每次会话的清理。

**Q: Agent 没用 taoDB 工具，怎么排查？**

1. 项目根目录有 `.mcp.json` 且是合法 JSON？
2. `taodb` 在 PATH 中？运行 `which taodb`。
3. `taodb init` 后重启 agent 了？
4. 检查 agent 的 MCP 日志看连接错误。

**Q: 多个 agent 能共享同一个 taoDB 吗？**

MCP 模式（本地）：每个项目目录一个 agent。redb 数据库被单个进程文件锁。

HTTP 模式（服务器）：多个 agent 可以连接到 `taodb serve`。用 API token 和 project ID 做隔离。

**Q: 怎么备份记忆？**

复制 `taodb-memory/` 目录。一切都在里面，以 redb 数据库文件存储。无需外部依赖，无需导出。

**Q: 能在不同机器间迁移记忆吗？**

把 `taodb-memory/` 复制到新机器。MCP 服务器下次启动时自动加载。

## 设计

**Q: 为什么用 redb 而不是 SQLite？**

redb 是纯 Rust 嵌入式 B-tree 引擎。提供 ACID 事务但无 SQL 开销，契合"零依赖"理念，对 taoDB 的键值访问模式语义更简单。

**Q: 为什么不在可选功能里加向量搜索？**

TaoDB 的核心论点就是"时空索引和语义搜索是根本不同的"。加向量搜索会模糊这个区分，让产品更难解释。如果你需要向量搜索，在 taoDB 旁边加一个向量数据库——它们服务于不同目的。

**Q: 为什么用 MCP 而不直接用 HTTP？**

MCP（Model Context Protocol）正在成为 LLM agent 工具集成的新标准。MCP stdio 传输意味着零网络开销、零配置、零认证搭建。agent 通过 `.mcp.json` 发现 taoDB，即刻开始使用。HTTP 作为备选协议提供给非 MCP 集成场景。

**Q: "能量"到底在算什么？**

能量是 0.0-1.0 的浮点数，由以下因素计算：原始记忆的情绪强度、与叙事锚点的时间距离、关联强度。公式采用 30 天（叙事时间）半衰期。`energy_floor` 作为硬下限——`energy_floor = 0.7` 的记忆无论多旧，能量始终 ≥ 0.7。

## 贡献

**Q: 怎么报 bug？**

在 GitHub 开 issue。附上 taoDB 版本（`taodb --version`）和复现步骤。

**Q: 能贡献代码吗？**

可以。见 [CONTRIBUTING.md](../CONTRIBUTING.md)。欢迎 PR——特别是容器 schema 模板、不同工作流的 agent instructions、平台支持。

**Q: 有路线图吗？**

设计理念见 [DESIGN.md](../DESIGN.md)。当前重心：稳定性、文档、MCP 生态集成。云托管和 Python SDK 在计划中但非当前优先。
