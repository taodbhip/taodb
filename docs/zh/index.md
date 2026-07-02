# TaoDB — AI 创作者的记忆引擎

## 问题

LLM agent 是失忆的。这不只是程序员的痛——AI 写作者断了叙事线、AI 设计师忘了设计系统、AI 视频制作者每次都要重新交代客户 brief。每次会话从零开始。Claude Code 打开你的项目，不记得上周修了什么 bug。Cursor Agent 不知道哪些架构决策导致了今天的代码结构。Windsurf 想不起三个会话前刚解决过的 bug 模式。

向量数据库解决不了这个问题。它们给你"语义相似"的文本——但相似不等于连续。知道一段代码"关于认证"和知道"我们在 sprint 3 修了 token rotation 的竞态条件，这个修复在 login flow 里引入了回归"是两回事。

**LLM 需要的是记忆。不是语义搜索。是时空记忆。**

## TaoDB 是什么

TaoDB 是 LLM agent 的**时空记忆引擎**。存原始记忆，按**时间**和**空间**取回——不做向量相似度计算。

```
写入:  LLM 产生记忆 → taoDB 存入，附带时间戳 + 空间标签
召回:  LLM 需要上下文 → taoDB 返回该时间窗 + 空间范围内的记忆
       → LLM 自己读、自己理解、自己判断相关性
```

TaoDB 做三件事：

1. **时间索引** — 每条记忆有叙事时间戳。按时间窗召回："最近 5 回 / 30天 / 3个sprint 发生了什么。"
2. **空间索引** — 每条记忆有容器标签：`module:auth`、`人物:桑安歌`、`场景:邯郸酒肆`。按空间范围召回。
3. **能量模型** — 记忆随时间衰减，重要信息设能量地板防止遗忘。无关细节自然消退。


## 工作方式

TaoDB 是一个**本地 MCP 服务**。不联网。不占端口。不需要 API token。

```
taodb init           # 在项目里创建 .mcp.json
重启 agent           # agent 检测到 .mcp.json，通过 MCP stdio 拉起 taoDB
                     # taoDB 读写本地 taodb-memory/ 目录
                     # agent 有记忆了。不可见。不需要任何配置。
```

Agent 和 taoDB 之间走 stdin/stdout 通信——跟 agent 调用其他 MCP 工具完全一样。不需要启动服务器。不需要配 token。不需要开防火墙。


## TaoDB 不是什么

TaoDB **不是**向量数据库。没有 embedding，没有相似度搜索，没有 FTS 排序。如果你需要"找和这段相似的段落"——用 Pinecone 或 Qdrant。

TaoDB **不是**搜索引擎。不做 BM25，不做相关性排序，不理解你的内容。理解是 LLM 的事。TaoDB 只做 LLM 做不到的事：持久化存储 + 时空索引。

TaoDB **不是**智能体。不做任何决策。不自动提取、不自动摘要、不自动触发。你的 agent 驱动整个循环。TaoDB 是记忆，不是大脑。

## 谁在用

TaoDB 是为 **AI 创作者** 设计的——所有用 LLM agent 作为创作搭档的人。

**Vibe coder** — agent 记住你改过哪些模块、修过哪些 bug、做过哪些架构决策。不用每次都重新解释代码库。

**网文作者** — 写作 agent 记得角色状态、物件历史、第 50 回埋的伏笔。时间索引天然对应章回编号。

**知识工作者** — 研究项目、会议记录、学习笔记全部按时间索引。agent 能回答"3 月份我学过哪些关于这个话题的内容"，无需你手动整理。

**AI 设计师** — agent 记住设计系统规则、组件迭代、客户反馈。每次改版都有上下文。不用重新解释栅格系统和品牌色。

**视频/广告制作者** — 项目 brief、剪辑决策、平台规格、客户修改意见。跨项目、跨 campaign、跨平台的管线连续性。

**Agent 开发者** — 构建需要跨会话持久记忆的 agent。MCP 原生、零配置、嵌入式。丢进去 agent 就有海马体。

## 它跟别人不一样在哪

| | TaoDB | 向量数据库 (Pinecone/Qdrant) | Mem0 / Zep |
|---|---|---|---|
| **索引** | 时间 + 空间 | 向量相似度 | 语义 + 对话 |
| **取回方式** | "auth 模块上周发生了什么" | "和这段文本相似的内容" | "跟这条消息语义相关的内容" |
| **衰减** | 有——能量地板模型 | 无 | 无 |
| **协议** | MCP（agent 原生，stdio） | REST / gRPC | REST |
| **依赖** | 零（嵌入式 redb） | 云端 / 重 | PostgreSQL / 云端 |

## 快速开始

```bash
# 安装
curl -fsSL https://raw.githubusercontent.com/taodbhip/taodb/main/install.sh | bash

# 初始化项目
cd my-project
taodb init

# 重启 agent。完毕。
```

不需要 API Key。不需要注册。不需要云服务。agent 通过 MCP 协议自动发现 taoDB，即刻开始使用。

---

**下一步：** [快速入门](getting-started.md) → 首次会话演练。
