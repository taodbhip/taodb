# TaoDB v2 设计方案

> 基于 EM-LLM / Shadow-Loom / Proust 无意记忆 / CoALA 四篇论文 + 通鉴世界写作实践

## 零、定位

TaoDB 是 LLM 的海马体。不是智能体，不是数据库。

- LLM = 大脑（决策、判断、创作）
- Skills = 工作记忆（当前任务的操作流程）
- TaoDB = 长期记忆（感官世界模型的持久存储）

TaoDB 只做三件事：**按感官索引存储、按感官触发召回、约束层与叙事层分离。**

## 一、核心洞察

### 从论文

| 论文 | 洞察 | 对 TaoDB 的意义 |
|------|------|----------------|
| EM-LLM | 贝叶斯惊异度做事件分割；两阶段检索（相似度+时间邻接） | 摄入时用惊异度检测事件边界；召回排序加入时间邻接权重 |
| Shadow-Loom | LLM只在边界使用；fabula/syuzhet双时间索引；约束化简报→渲染→审计 | 记忆存fabula时间(因果链) + syuzhet索引(叙事流)；约束层记忆永远在上下文 |
| Proust | 感官触发比主动检索产生更强烈的体验复现；情绪是决定性因素 | **感官索引是一等公民**，和时间/空间索引同级 |
| CoALA | 模块化记忆：工作/情景/语义/程序 | TaoDB = 情景记忆 + 语义记忆模块 |

### 从实践

| 发现 | 对 TaoDB 的意义 |
|------|----------------|
| "极其"崩溃是LLM的session退化，不是产品问题 | 不做"极其"检查，不做任何输出层面的QC |
| 约束层(规则/角色框架) vs 叙事层(事件/描写) | 分离存储，分离召回，分离注入 |
| 感官触发工作：写到涩→鼓沿木缝/指甲泥/掌根螺旋纹同时浮上来 | 感官索引维度是TaoDB区别于向量数据库的核心 |
| 世界模型从文档来(设定集/简报/章回规划)→存为约束→永远在上下文 | 摄入流程：世界文档→约束层。叙事→叙事层。两种memorize |

## 二、存储模型

### Memory 结构（现有基础上增强）

```rust
struct Memory {
    id: Ulid,
    
    // ── 双时间索引 (Shadow-Loom fabula/syuzhet) ──
    time_ns: i64,              // fabula: 叙事内时间线 (如第152回=1782532456372428000)
    chapter_index: String,     // syuzhet: 叙事呈现顺序 (如"卷二/第152回")
    
    // ── 感官索引 (Proust) ──
    senses: Vec<SenseAnchor>,  // 从叙事中提取的感官锚点 ("涩","凉","颤","跳")
    
    // ── 空间索引 ──
    containers: Vec<String>,   // "人物:桑安歌", "场景:邯郸酒肆", "物件:鼓"
    
    // ── 约束标记 ──
    energy_floor: f32,         // 0.0=叙事记忆(衰减), 0.5+=半永久, 0.7+=永久约束(不衰减)
    memory_type: MemoryType,   // Constraint | Narrative
    
    // ── 内容 ──
    text: String,              // 记忆文本 (LLM写入的摘要)
    body_state: Option<BodyState>,      // 身体状态 (JSON)
    emotional_mark: Option<EmotionalMark>, // 情绪标记 (JSON)
}

enum MemoryType {
    Constraint,  // 世界规则、角色感知框架、物件链 —— 不衰减，永远在召回结果里
    Narrative,   // 叙事事件 —— 随时间衰减
}
```

### 感官索引（新增，和时空索引同级）

```
sense_index (独立索引，不进主存储):
  "涩" → [mem_005, mem_012, mem_027, mem_041]
  "凉" → [mem_003, mem_007, mem_019, mem_033]
  "颤" → [mem_012, mem_022, mem_038]
  "跳" → [mem_012, mem_015, mem_041]
  ...
```

为什么独立索引：感官是跨容器、跨时间、跨人物的。鼓沿木缝的涩和指甲泥的涩和掌根螺旋纹的涩——它们分属不同回、不同人物、不同场景，但在感官维度上是同一个东西。TaoDB的价值就在这：不是"查找桑安歌的记忆"，是"所有涩的记忆自己浮上来"。

## 三、召回模型

### 三层召回，分开调用

**第一层：约束层 (LLM不需要调用，TaoDB在会话开始时自动返回)**

```
recall_constraints() → Vec<Memory>
  条件: energy_floor > 0.5
  返回格式: 结构化约束列表
  用途: 注入LLM系统提示词，作为"不可违背的世界模型"
  
  例:
  约束-世界规则: "工具不感应矿脉。人的身体记忆感应。"
  约束-桑安歌感知: "她用喉咙/声带/筑弦感知世界。振动和频率是她的感官语言。"
  约束-柏正则感知: "他用虎口茧/膝盖/凿子/右脚踝感知世界。石头的硬度和凿子的偏向是他的感官语言。"
  约束-物件链: "凿子→匾额刀法→四十万人名→剑上六万道痕。方向往右偏，快到尽头时往回勾。"
```

**第二层：感官触发层 (LLM调用recall_sensory)**

```
recall_sensory(senses: Vec<String>, top_k: usize) → Vec<Memory>
  从sense_index匹配共享感官的记忆
  排序: 感官匹配数 DESC + 时间邻接权重
  用途: 当前写作场景中出现的感官状态自动激活共享记忆
  
  例: LLM正在写桑安歌喉咙涩味 → recall_sensory(["涩","紧","颤"]) 
      → 返回鼓沿木缝的涩、指甲在刻痕里的泥、老槐掌根的螺旋纹
```

**第三层：叙事时空层 (LLM调用recall_narrative)**

```
recall_narrative(
  persons: Vec<String>,      // 人物过滤
  locations: Vec<String>,    // 空间过滤
  objects: Vec<String>,      // 物件链
  narrative_span_days: i64,  // fabula时间窗
  chapter_span: usize,       // syuzhet叙事窗 (前N回)
  top_k: usize
) → Vec<Memory>
  多维度并行 → 合并去重 → 按多维评分排序
  评分: 感官匹配 + 时间邻接 + 空间重叠 + 物件链权重
```

### 和当前多维召回的差异

当前：一个 recall(query, containers, time_span, energy_floor) 做所有事。约束、感官、叙事混在一个结果列表里返回。

改后：**约束层自动加载、感官触发独立入口、叙事层多维但不带约束。** LLM通过skills知道什么时候调哪个——这不是TaoDB的逻辑，是skills的工作。

## 四、Memorize接口

### 叙事记忆

```
memorize_narrative({
  what: "桑安歌发现鼓腔泛音与筑第七弦同频。鼓皮敲击压密点振动频率和筑面板年轮密度对应。灵力在声门下缩了一下，多出的空腔频率刚好套上鼓腔共鸣。",
  senses: ["涩", "颤", "跳"],
  objects: ["鼓", "筑", "弦"],
  persons: ["桑安歌"],
  containers: ["邯郸酒肆", "第152回"],
  chapter_index: "卷二/第152回",
  time_ns: 1782532456372428000,
  body_state: "喉咙里三层疤发紧，声门下灵力在颤",
  energy_floor: 0.0,
})
```

### 约束记忆

```
memorize_constraint({
  what: "桑安歌感知世界的方式：振动和频率。筑弦的振、鼓皮的跳、喉咙里三层疤的涩——世界通过身体共振进入她。她的感官器官是喉咙和筑，她的感官语言是振动。",
  containers: ["world_rule", "人物:桑安歌", "感知框架"],
  energy_floor: 0.7,  // 永久，不衰减
})
```

### 设计理由

LLM决定存什么、怎么存。TaoDB不自动提取感官——LLM在写后存记忆时自己判断这篇叙事里有哪几个突出的感官锚点。这保持了"LLM是大脑，TaoDB是海马体"的边界。

## 五、不做的

根据"TaoDB不是智能体"原则：

- ❌ 不做自动事件分割（EM-LLM的Bayesian surprise）——LLM判断
- ❌ 不做叙事质量评分（Shadow-Loom的narrative physics）——LLM判断
- ❌ 不做自动感官提取（从文本中提取感官词）——LLM调用memorize时自己提供
- ❌ 不做因果推理——LLM的事
- ❌ 不做"极其"检测或任何输出QC——LLM session退化问题，不是存储问题

## 六、与Creator IDE的关系

```
Creator IDE
├── Skills (工作记忆: 写作流程、身体锚定、QC硬指标)
├── Agent (Claude CLI管理: 会话池、系统提示词组装)
├── Immersion Engine (写前沉浸: 调recall_constraints + recall_sensory → 组装身体锚定文本)
└── 前端 (编辑器 + 写作面板)

TaoDB (长期记忆: MCP接口)
├── recall_constraints() → 约束层记忆 (永远是上下文)
├── recall_sensory(senses) → 感官触发记忆
├── recall_narrative(persons, locations, objects, time) → 叙事时空记忆
├── memorize_narrative(...) → 存叙事记忆
└── memorize_constraint(...) → 存约束记忆
```

Skills层决定工作流：
1. 写前 → recall_constraints (自动) + recall_sensory(POV人物近期感官)
2. 写中 → LLM自由调用recall_sensory触发相关记忆
3. 写后 → memorize_narrative (LLM自己提取感官锚点)

Immersion Engine用constraints + sensory building身体锚定文本，注入agent系统提示词。
