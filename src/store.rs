//! 存储层 — redb + 时间索引
//!
//! 表:
//!   memories  — key=ULID, value=CRC(bincode(Memory))
//!   timeline  — key=time_ns_BE+ULID, value=ULID (按时间排序)
//!
//! 存: put(memory)
//! 取: get_by_id(id) / recent(n) / time_range(from, to, limit)

use crate::model::{Memory, potential_energy, potential_energy_narrative};
use anyhow::Result;
use parking_lot::RwLock;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::collections::HashSet;
use std::path::Path;

const TABLE_MEMORIES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("memories");
const TABLE_TIMELINE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("timeline");
/// 感官索引 — impression → ULID列表
/// Proust: 感官触发召回 — "涩"激活所有共享涩感的记忆
const TABLE_SENSE_INDEX: TableDefinition<&[u8], &[u8]> = TableDefinition::new("sense_index");

pub struct Store {
    db: Database,
    cache: RwLock<Vec<Memory>>,
}

fn timeline_key(time_ns: i64, id: &str) -> Vec<u8> {
    let mut k = Vec::with_capacity(8 + id.len());
    k.extend_from_slice(&time_ns.to_be_bytes());
    k.extend_from_slice(id.as_bytes());
    k
}

fn timeline_start(time_ns: i64) -> Vec<u8> {
    let mut k = Vec::with_capacity(8);
    k.extend_from_slice(&time_ns.to_be_bytes());
    k
}

fn timeline_end(time_ns: i64) -> Vec<u8> {
    let mut k = Vec::with_capacity(9);
    k.extend_from_slice(&time_ns.to_be_bytes());
    k.push(0xff);
    k
}

impl Store {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let base = path.as_ref();
        if !base.exists() {
            std::fs::create_dir_all(base)?;
        }
        let db_path = if base.is_dir() {
            base.join("taodb.redb")
        } else {
            base.to_path_buf()
        };
        let db = Database::create(&db_path)?;
        {
            let txn = db.begin_write()?;
            txn.open_table(TABLE_MEMORIES)?;
            txn.open_table(TABLE_TIMELINE)?;
            txn.open_table(TABLE_SENSE_INDEX)?;
            txn.commit()?;
        }
        let store = Self {
            db,
            cache: RwLock::new(Vec::new()),
        };
        store.load_all()?;
        Ok(store)
    }

    fn load_all(&self) -> Result<()> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(TABLE_MEMORIES)?;
        let mut cache = self.cache.write();
        for item in table.iter()? {
            let (_, val) = item?;
            let raw = crate::crc::decode_tolerant(val.value());
            let mem: Memory = bincode::deserialize(&raw)?;
            cache.push(mem);
        }
        eprintln!("loaded {} memories", cache.len());
        Ok(())
    }

    pub fn put(&self, mem: &Memory) -> Result<()> {
        let raw = bincode::serialize(mem)?;
        let with_crc = crate::crc::encode_with_crc(&raw);
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(TABLE_MEMORIES)?;
            table.insert(mem.id.to_string().as_bytes(), with_crc.as_slice())?;
            let mut tl = txn.open_table(TABLE_TIMELINE)?;
            tl.insert(
                timeline_key(mem.time.absolute_ns, &mem.id.to_string()).as_slice(),
                mem.id.to_string().as_bytes(),
            )?;
            // P1-2: update sense_index atomically within the same transaction
            if !mem.senses.is_empty() {
                let mut si = txn.open_table(TABLE_SENSE_INDEX)?;
                for sense in &mem.senses {
                    let key = sense.impression.as_bytes();
                    let mut existing: Vec<String> = if let Ok(Some(val)) = si.get(key) {
                        bincode::deserialize(val.value()).unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    let mem_id = mem.id.to_string();
                    if !existing.contains(&mem_id) {
                        existing.push(mem_id);
                        let encoded = bincode::serialize(&existing)?;
                        si.insert(key, encoded.as_slice())?;
                    }
                }
            }
        }
        txn.commit()?;
        let mut cache = self.cache.write();
        if let Some(existing) = cache.iter_mut().find(|m| m.id == mem.id) {
            *existing = mem.clone();
        } else {
            cache.push(mem.clone());
        }
        Ok(())
    }

    pub fn get_by_id(&self, id: &str) -> Option<Memory> {
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(TABLE_MEMORIES).ok()?;
        let val = table.get(id.as_bytes()).ok()??;
        let data = crate::crc::decode_tolerant(val.value());
        bincode::deserialize(&data).ok()
    }

    /// 按时间范围查询记忆 ID（O(log n + k)）
    pub fn time_range(&self, from_ns: i64, to_ns: i64, limit: usize) -> Vec<String> {
        let txn = match self.db.begin_read() {
            Ok(t) => t,
            Err(_) => return vec![],
        };
        let table = match txn.open_table(TABLE_TIMELINE) {
            Ok(t) => t,
            Err(_) => return vec![],
        };
        let start = timeline_start(from_ns);
        let end = timeline_end(to_ns);
        let mut ids = Vec::new();
        if let Ok(range) = table.range(start.as_slice()..end.as_slice()) {
            for item in range {
                if ids.len() >= limit {
                    break;
                }
                if let Ok((_, val)) = item
                    && let Ok(s) = std::str::from_utf8(val.value())
                {
                    ids.push(s.to_string());
                }
            }
        }
        ids
    }

    /// 回忆增强：被 recall 触发后 energy +delta，仅更新缓存（性能优先）
    /// 调用 decay_all() 时统一写回磁盘
    pub fn boost_energy(&self, ids: &[String], delta: f32) {
        let mut cache = self.cache.write();
        for mem in cache.iter_mut() {
            if ids.contains(&mem.id.to_string()) {
                mem.potential_energy = f32::min(mem.potential_energy + delta, 1.0);
            }
        }
    }

    /// 将 cache 中所有 energy 变更写回磁盘
    pub fn flush_energy(&self) -> Result<()> {
        let txn = self.db.begin_write()?;
        let mut table = txn.open_table(TABLE_MEMORIES)?;
        let cache = self.cache.read();
        for mem in cache.iter() {
            if let Ok(raw) = bincode::serialize(mem) {
                let with_crc = crate::crc::encode_with_crc(&raw);
                table.insert(mem.id.to_string().as_bytes(), with_crc.as_slice())?;
            }
        }
        drop(cache);
        drop(table);
        txn.commit()?;
        Ok(())
    }

    /// 返回高能记忆（不限时间，potential_energy > min_energy）
    pub fn energy_range(&self, min_energy: f32, limit: usize) -> Vec<Memory> {
        self.cache
            .read()
            .iter()
            .filter(|m| m.potential_energy > min_energy)
            .take(limit)
            .cloned()
            .collect()
    }

    /// 返回最近 N 条记忆（从 cache 尾部取，O(1)）
    pub fn indexed_recent(&self, n: usize) -> Vec<Memory> {
        let cache = self.cache.read();
        let len = cache.len();
        if len == 0 {
            return vec![];
        }
        let start = len.saturating_sub(n);
        let mut result: Vec<Memory> = cache[start..].to_vec();
        result.reverse(); // 最近在前
        result
    }

    /// 返回时空窗口内的记忆（带 limit）
    pub fn window_memories(&self, seed_ids: &HashSet<String>, days: i64, limit: usize) -> Vec<Memory> {
        let mut result = Vec::new();
        let mut seen: HashSet<String> = seed_ids.iter().cloned().collect();

        // 先加种子
        for id in seed_ids {
            if let Some(m) = self.get_by_id(id) {
                seen.insert(id.clone());
                result.push(m);
            }
        }

        // 按时间范围扫描
        let txn = match self.db.begin_read() {
            Ok(t) => t,
            Err(_) => return result,
        };
        let table = match txn.open_table(TABLE_TIMELINE) {
            Ok(t) => t,
            Err(_) => return result,
        };

        // 从种子的时间范围扫描
        for seed_id in seed_ids {
            if result.len() >= limit {
                break;
            }
            if let Some(seed) = self.get_by_id(seed_id) {
                let from = seed.time.absolute_ns - days * 86400 * 1_000_000_000_i64;
                let to = seed.time.absolute_ns + days * 86400 * 1_000_000_000_i64;
                if let Ok(range) = table.range(timeline_start(from).as_slice()..timeline_end(to).as_slice()) {
                    for item in range {
                        if result.len() >= limit {
                            break;
                        }
                        if let Ok((_, val)) = item
                            && let Ok(s) = std::str::from_utf8(val.value())
                            && !seen.contains(s)
                        {
                            seen.insert(s.to_string());
                            if let Some(m) = self.get_by_id(s) {
                                result.push(m);
                            }
                        }
                    }
                }
            }
        }
        result
    }

    /// 从 cache 取指定 ID 的记忆（O(n) 扫描，用于小量读取）
    pub fn get_cached(&self, id: &str) -> Option<Memory> {
        self.cache.read().iter().find(|m| m.id.to_string() == id).cloned()
    }

    pub fn all(&self) -> Vec<Memory> {
        self.cache.read().clone()
    }

    // ── 感官索引 (Proust: involuntary sensory-triggered recall) ──

    /// 用感官锚点查询共享记忆的 ID 列表
    pub fn get_ids_by_senses(&self, impressions: &[String], limit: usize) -> Vec<String> {
        let txn = match self.db.begin_read() {
            Ok(t) => t,
            Err(_) => return vec![],
        };
        let table = match txn.open_table(TABLE_SENSE_INDEX) {
            Ok(t) => t,
            Err(_) => return vec![],
        };
        let mut seen = HashSet::new();
        let mut results = Vec::new();
        for imp in impressions {
            if results.len() >= limit {
                break;
            }
            if let Ok(Some(val)) = table.get(imp.as_bytes()) {
                // Value is bincode-encoded Vec<String> of ULIDs
                if let Ok(ids) = bincode::deserialize::<Vec<String>>(val.value()) {
                    for id in ids {
                        if results.len() >= limit {
                            break;
                        }
                        if seen.insert(id.clone()) {
                            results.push(id);
                        }
                    }
                }
            }
        }
        results
    }

    /// 更新感官索引 — 为每个 impression 添加 memory ID
    pub fn update_sense_index(&self, mem: &Memory) -> Result<()> {
        if mem.senses.is_empty() {
            return Ok(());
        }
        let txn = self.db.begin_write()?;
        let mut table = txn.open_table(TABLE_SENSE_INDEX)?;
        for sense in &mem.senses {
            let key = sense.impression.as_bytes();
            let mut existing: Vec<String> = if let Ok(Some(val)) = table.get(key) {
                bincode::deserialize(val.value()).unwrap_or_default()
            } else {
                Vec::new()
            };
            let mem_id = mem.id.to_string();
            if !existing.contains(&mem_id) {
                existing.push(mem_id);
                let encoded = bincode::serialize(&existing)?;
                table.insert(key, encoded.as_slice())?;
            }
        }
        drop(table);
        txn.commit()?;
        Ok(())
    }

    /// 按感官触发查询完整 Memory (按匹配感官数量 + 时间邻接排序)
    pub fn sensory_recall(&self, impressions: &[String], top_k: usize, narrative_span_days: i64) -> Vec<Memory> {
        let mut memories: Vec<Memory> = Vec::new();
        let mut seen = HashSet::new();
        // 按感官匹配数降序排列 impressions — 更多记忆共享的感官权重更高
        for imp in impressions {
            if memories.len() >= top_k * 2 {
                break;
            }
            let ids = self.get_ids_by_senses(std::slice::from_ref(imp), 100);
            for id in ids {
                if memories.len() >= top_k * 2 {
                    break;
                }
                if seen.insert(id.clone())
                    && let Some(mem) = self.get_by_id(&id)
                {
                    // 应用叙事时间窗过滤
                    if narrative_span_days > 0 {
                        let anchor = self.latest_time();
                        let span_ns = narrative_span_days * 86400 * 1_000_000_000_i64;
                        let dist = (mem.time.absolute_ns - anchor).abs();
                        if dist > span_ns {
                            continue;
                        }
                    }
                    memories.push(mem);
                }
            }
        }
        // 按匹配感官数量 DESC + 时间邻接 DESC 排序
        let anchor = self.latest_time();
        memories.sort_by(|a, b| {
            let a_count = a.senses.iter().filter(|s| impressions.contains(&s.impression)).count();
            let b_count = b.senses.iter().filter(|s| impressions.contains(&s.impression)).count();
            b_count.cmp(&a_count).then_with(|| {
                let a_dist = (a.time.absolute_ns - anchor).abs();
                let b_dist = (b.time.absolute_ns - anchor).abs();
                a_dist.cmp(&b_dist)
            })
        });
        memories.truncate(top_k);
        memories
    }

    fn latest_time(&self) -> i64 {
        self.cache.read().iter().map(|m| m.time.absolute_ns).max().unwrap_or(0)
    }

    pub fn recent(&self, n: usize) -> Vec<Memory> {
        let cache = self.cache.read();
        let len = cache.len();
        if len <= n {
            cache.clone()
        } else {
            cache[len - n..].to_vec()
        }
    }

    pub fn count(&self) -> usize {
        self.cache.read().len()
    }

    /// 容器分布统计（LLM schema 感知）
    pub fn container_distribution(&self) -> Vec<crate::model::ContainerStats> {
        let cache = self.cache.read();
        let mut map: std::collections::HashMap<String, (usize, i64)> = std::collections::HashMap::new();
        for mem in cache.iter() {
            for c in &mem.space.containers {
                let entry = map.entry(c.clone()).or_insert((0, 0));
                entry.0 += 1;
                if mem.time.absolute_ns > entry.1 {
                    entry.1 = mem.time.absolute_ns;
                }
            }
        }
        let mut stats: Vec<crate::model::ContainerStats> = map
            .into_iter()
            .map(|(name, (count, latest_time_ns))| crate::model::ContainerStats {
                name,
                count,
                latest_time_ns,
            })
            .collect();
        stats.sort_by_key(|b| std::cmp::Reverse(b.count));
        stats
    }

    /// 全局时间跨度
    pub fn time_span(&self) -> Option<(i64, i64)> {
        let cache = self.cache.read();
        let min = cache.iter().map(|m| m.time.absolute_ns).min()?;
        let max = cache.iter().map(|m| m.time.absolute_ns).max()?;
        Some((min, max))
    }

    /// energy_floor 分布
    pub fn energy_floor_distribution(&self) -> Vec<crate::model::EnergyFloorBucket> {
        let cache = self.cache.read();
        let mut map: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
        for mem in cache.iter() {
            // 按 0.1 分桶
            let bucket = (mem.energy_floor * 10.0).round() as i64;
            *map.entry(bucket).or_insert(0) += 1;
        }
        let mut buckets: Vec<crate::model::EnergyFloorBucket> = map
            .into_iter()
            .map(|(bucket, count)| crate::model::EnergyFloorBucket {
                floor: bucket as f32 / 10.0,
                count,
            })
            .collect();
        buckets.sort_by(|a, b| a.floor.partial_cmp(&b.floor).unwrap_or(std::cmp::Ordering::Equal));
        buckets
    }

    /// 最近使用的 containers（最近 10 条）
    pub fn recent_containers(&self, n: usize) -> Vec<String> {
        let cache = self.cache.read();
        let len = cache.len();
        if len == 0 {
            return vec![];
        }
        let start = len.saturating_sub(n);
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for mem in cache[start..].iter().rev() {
            for c in &mem.space.containers {
                if seen.insert(c.clone()) {
                    result.push(c.clone());
                }
            }
        }
        result
    }

    /// 模糊匹配已有的 container（找语义最近的）
    pub fn fuzzy_match_container(&self, target: &str) -> Option<String> {
        let cache = self.cache.read();
        let target_lower = target.to_lowercase();
        // 收集所有已知 container
        let mut all_containers: std::collections::HashSet<String> = std::collections::HashSet::new();
        for mem in cache.iter() {
            for c in &mem.space.containers {
                all_containers.insert(c.clone());
            }
        }
        // 精确匹配
        if all_containers.contains(target) {
            return Some(target.to_string());
        }
        // 忽略大小写匹配
        for c in &all_containers {
            if c.to_lowercase() == target_lower {
                return Some(c.clone());
            }
        }
        // 前缀匹配（如 "人物:" 开头）
        let prefix = if target.contains(':') {
            target.split(':').next().unwrap_or("").to_string() + ":"
        } else {
            return None; // 无前缀，不做模糊匹配
        };
        for c in &all_containers {
            if c.starts_with(&prefix) && c.to_lowercase().contains(&target_lower[prefix.len()..]) {
                return Some(c.clone());
            }
        }
        None
    }

    /// 从 containers 推导叙事时间
    /// 识别 "第N回" 模式，自动计算 time_ns
    pub fn derive_time_ns_from_containers(&self, containers: &[String]) -> Option<i64> {
        for c in containers {
            // 匹配 "第N回" 或 "第N回_XXX"
            if let Some(n) = Self::parse_chapter_number(c) {
                // time_ns(第N回) = BASE + (N-1) × 86400 × 10^9
                let base: i64 = 1700000000000000000;
                return Some(base + (n - 1) * 86400 * 1_000_000_000_i64);
            }
        }
        None
    }

    /// 删除一条记忆（从 cache 和 MEMORIES 表移除；timeline 索引接受少量残留）
    pub fn forget(&self, memory_id: &str) -> bool {
        let txn = match self.db.begin_write() {
            Ok(t) => t,
            Err(_) => return false,
        };
        let found = {
            let mut table = match txn.open_table(TABLE_MEMORIES) {
                Ok(t) => t,
                Err(_) => return false,
            };
            matches!(table.remove(memory_id.as_bytes()), Ok(Some(_)))
        };
        if found {
            // Clean sense_index: remove this memory ID from all impression lists
            {
                let mut sense_table = match txn.open_table(TABLE_SENSE_INDEX) {
                    Ok(t) => t,
                    Err(_) => return found,
                };
                // Iterate all sense entries and prune the deleted ID
                let mut to_update: Vec<(String, Vec<String>)> = Vec::new();
                if let Ok(iter) = sense_table.iter() {
                    for item in iter {
                        if let Ok((key, val)) = item
                            && let Ok(impression) = std::str::from_utf8(key.value())
                            && let Ok(mut ids) = bincode::deserialize::<Vec<String>>(val.value())
                        {
                            let before = ids.len();
                            ids.retain(|id| id != memory_id);
                            if ids.len() != before {
                                to_update.push((impression.to_string(), ids));
                            }
                        }
                    }
                }
                for (impression, ids) in to_update {
                    if let Ok(encoded) = bincode::serialize(&ids) {
                        let _ = sense_table.insert(impression.as_bytes(), encoded.as_slice());
                    }
                }
            }
            let mut cache = self.cache.write();
            cache.retain(|m| m.id.to_string() != memory_id);
        }
        let _ = txn.commit();
        found
    }

    pub fn decay_all(&self) -> Result<()> {
        let mut cache = self.cache.write();
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(TABLE_MEMORIES)?;
            for mem in cache.iter_mut() {
                let raw_energy = potential_energy(
                    mem.time.absolute_ns,
                    mem.emotion.iter().map(|e| e.intensity).fold(0.0_f32, f32::max),
                    1.0,
                );
                // 取 max(formula, floor, current_boosted) 保留召回增强
                mem.potential_energy = f32::max(raw_energy, f32::max(mem.energy_floor, mem.potential_energy));
                let raw = bincode::serialize(&*mem)?;
                let with_crc = crate::crc::encode_with_crc(&raw);
                table.insert(mem.id.to_string().as_bytes(), with_crc.as_slice())?;
            }
        }
        txn.commit()?;
        Ok(())
    }

    /// 以叙事锚点执行能量衰减（非墙上时钟）
    pub fn decay_all_narrative(&self, anchor_ns: i64) -> Result<()> {
        let mut cache = self.cache.write();
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(TABLE_MEMORIES)?;
            for mem in cache.iter_mut() {
                let raw_energy = potential_energy_narrative(
                    anchor_ns,
                    mem.time.absolute_ns,
                    mem.emotion.iter().map(|e| e.intensity).fold(0.0_f32, f32::max),
                    1.0,
                );
                mem.potential_energy = f32::max(raw_energy, f32::max(mem.energy_floor, mem.potential_energy));
                let raw = bincode::serialize(&*mem)?;
                let with_crc = crate::crc::encode_with_crc(&raw);
                table.insert(mem.id.to_string().as_bytes(), with_crc.as_slice())?;
            }
        }
        txn.commit()?;
        Ok(())
    }

    /// 从 container 字符串解析章回编号
    /// 匹配 "第148回"、"第148回_无声张嘴" 等模式
    fn parse_chapter_number(s: &str) -> Option<i64> {
        let after_di = s.find('第')?;
        let num_start = after_di + '第'.len_utf8();
        let rest = &s[num_start..];
        let num_end = rest.find(|c: char| !c.is_ascii_digit())?;
        let num_str = &rest[..num_end];
        let n: i64 = num_str.parse().ok()?;
        let after_num = &rest[num_end..];
        if after_num.starts_with('回') { Some(n) } else { None }
    }
}
