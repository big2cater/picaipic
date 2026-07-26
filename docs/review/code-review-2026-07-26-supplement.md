# PicAiPic 补充代码审计报告（核验修订版）

- 日期：2026-07-26
- 修订：2026-07-26（对照工作区源码二次核验；纠正过时/夸大项）
- 范围：全项目静态分析 + 结构性/性能审计（补充 [主报告](code-review-2026-07-26.md) 的 B1–B10）
- 方法：jCodeMunch 索引级分析 + 定向源码实读 + 本轮 `rg`/实读复核
- 结论：**无致命 Bug**。原稿 12 项结构 + 6 项性能中，**真债约半数**；若干「立即修复」项已过时或 ROI 极低。优先做 hygiene / 锁一致性 / 长期拆分，勿按原稿第 5 节原样排期。

---

## 0. 核验总表（以当前代码为准）

| ID | 原稿判定 | 核验 | 说明 |
|---|---|---|---|
| S1 | 严重 | **属实（债）** | `AFile::new` 仍 ~938–1441，职责多；非线上故障 |
| S2 | 严重 | **属实（债）** | `Content.vue` ~8145 行；`ref`≈77、`computed`≈63（非「120+ ref」）；性能开销未 profiling |
| S3 | 中等 | **属实（债）** | 多函数高复杂度；影响回归成本 |
| S4 | 中等 | **属实（轻）** | 2 组循环仍在；Vue 组件循环常见；`api/config/store` 更值得注意 |
| S5 | 中等 | **夸大** | 「249 unstable」多为叶子模块指标噪音；真枢纽是 `t_sqlite` 体积 |
| S6 | 中等 | **收窄 → 本轮加固** | 原有 embed/search 单测；**新增** `query_builder_tests`（14）：`build_file_type_condition` / `build_search_query_parts` / `build_smart_rule_condition` / `build_smart_query_parts` / `sj_*` |
| S7 | 低 | **属实（轻）** | `build.rs` ~12 处 `.unwrap()`（非 17+）；编译期，风格问题 |
| S8 | 低 | **部分过时** | `t_cmds::lock_mutex` 已 poison-safe；残留：`main`/`t_sqlite` AI 锁、`t_face`、`t_dedup`、`t_utils` 进度 tracker 等 |
| S9 | 低 | **属实（设计）** | sleep 多用于进程启停/健康检查；不全是无脑轮询；事件化非必须 |
| S10 | 低 | **夸大** | 多数是 `Arc` clone；`face_indices.clone()` 在聚类建图一次，ROI 低 |
| S11 | 信息 | **非问题** | 插件树「死代码」不经 Rust import 图，预期如此 |
| S12 / P5 | 信息/低 | **属实** | 多处调试 `console.log`；错误路径也有用 `log` 的 |
| P1 | 高价值 | **半过时** | 单 `Mutex<AiEngine>` 仍在；**解码已在锁外**；人脸是独立 `FaceState`；剩余是 CLIP 推理串行 |
| P2 | 高价值 | **推测** | 巨型组件真；Map 化键盘/URL 缓存无卡顿证据 |
| P3 | 中等 | **取舍** | 128KB 预读在；为减少 open；非明显错误 |
| P4 | 中等 | **半属实** | ~200MB 量级对；已有 cache/warm/后台 ANN；mmap SQLite blob 不现实 |
| P6 | 低 | **属实（设计）** | 固定 poll 可接受；Notify 是增强非缺陷 |

**本轮落地修复（见 §5）：** S8 残留 poison 锁统一恢复；P5 清理调试 `console.log`（失败路径改 `console.error` 或删除纯调试）。

---

## 1. 覆盖范围

| 层 | 关注点 |
|---|---|
| Rust 后端 | `t_sqlite.rs`、`t_plugin.rs`、`build.rs`、`t_cluster.rs`、`t_cmds.rs`、`t_image.rs`、`t_face.rs`、`main.rs` |
| Vue 前端 | `Content.vue`、`AlbumFolder`/`AlbumList`/`MoveTo`、api/config/store |
| 架构 | 依赖循环、耦合指标、死代码、测试覆盖 |
| 运行时 | Mutex、sleep、clone、分配模式 |

主报告 B1–B10 不重复；本报告只谈结构/性能债。

---

## 2. 结构性项（修订后）

### S1 [债/高] `AFile::new()` 大体量

- **文件**：`src-tauri/src/t_sqlite.rs:938–1441`
- **核验**：属实。混合类型判断、图/视/RAW 元数据、EXIF、GPS、Live Photo、header 预读等。
- **建议**：拆 `extract_*` 可单测函数；**排期重构**，非 hotfix。

### S2 [债/高] `Content.vue` 单体

- **文件**：`src-vite/src/components/Content.vue`（~8145 行）
- **核验**：职责多、状态多属实；「120+ ref 已有可测量开销」**无证据**。
- **建议**：改相关功能时渐进抽 composable；避免单独 2–3 周大拆。

### S3 [债/中] `t_sqlite` 其他高复杂度函数

- 属实。优先未来给 `build_smart_rule_condition` / `search_similar_images` 加行为测试再考虑拆。

### S4 [轻] 前端依赖循环 2 组

```
AlbumFolder.vue → AlbumList.vue → MoveTo.vue
api.js → config.js → libraryStore.js
```

- 属实。组件循环可接受；横切 `api/config/store` 解耦有价值但非紧急。

### S5 [夸大] 「249 个不稳定模块」

- 耦合雷达把大量叶子标成 I>0.5 正常。关注 `t_sqlite` / 核心路径测试即可。

### S6 [收窄 → 已加固] 测试覆盖

- **错误原文**：「`t_sqlite.rs` 零测试 / `has_tests=false`」
- **事实**：已有 image-search top-k 与 embed score 单测。
- **本轮**：`query_builder_tests`（14）覆盖 file-type mask、search WHERE（Live companion / 排除文件夹 / 名/收藏/评分/标签/人脸 / 本地日历日 / 跨日界线 GPS）、smart rule（size/name/favorite/rating/tag/person/date/unsupported）、match all|any、`sj_*` 解析。
- **仍缺**：`AFile::insert`/`update` 的 DB 集成测试（需 fixture 库）。

### S7 [轻] `build.rs` unwrap

- ~12 处；编译失败即止；可逐步 `.expect("…")`。

### S8 [部分过时 → 本轮收尾] Mutex poison

- **已有**：`t_cmds::lock_mutex` → `PoisonError::into_inner`；部分 `t_sqlite` thumb 锁、`t_utils` album scan 守卫。
- **本轮统一**：共享 `t_common::lock_mutex`；覆盖 `main` 启动 AI、`t_sqlite` AI 路径、`t_face`、`t_dedup`、`t_utils` progress/cancel。
- **不改**：`Option`/`Result` 上的业务 `.unwrap()`（如 tokenizer 假定已 load）——与 poison 无关。

### S9 [设计] 硬编码 sleep

- 插件/视频等待进程；常量化优于全事件化。低优先级。

### S10 [夸大] 热路径 clone

- `(*state).clone()` 多为 `Arc`。
- `face_indices.clone()` 建 HNSW 一次；**不按本周项**。

### S11 [非问题] 插件目录「死代码」

- 样本插件不进 Rust import 图；清理时再删。

### S12 [轻] 调试日志

- 见 P5；本轮清理。

---

## 3. 性能项（修订后）

### P1 [半过时] AiState 锁

| 原稿 | 现状 |
|---|---|
| 单 Mutex 包整机 | 仍是 `AiState(Mutex<AiEngine>)` |
| 解码在锁内 | **否** — `generate_embedding` 先 `load_image_for_clip_embed`，再锁内 `encode_image_from_dynamic` |
| 人脸与 CLIP 抢同一把锁 | **否** — `FaceState` 独立 |
| ONNX 推理串行 | **是** — CLIP 吞吐上限 |

**剩余工作（中期）**：多 session / 拆文本与视觉锁；**不要**再当「短期把解码移出锁」排期。

### P2 [推测] Content 响应式

- 单体债真；具体优化需 profiling。

### P3 [取舍] 128KB header 预读

- 设计为减少多次 `File::open`；保留。

### P4 [半属实] 嵌入矩阵内存

- 量级对；warm/cache/ANN 已有。mmap 直接读 SQLite blob **不可行**。磁盘持久化仍 deferred（与主报告 B5 一致）。

### P5 [轻 → 本轮] console 调试

- 删除纯调试 `console.log`；失败信息用 `console.error`（或静默由上层 toast）。

### P6 [设计] 插件 poll

- 可增强为 Notify；非缺陷。

---

## 4. 与主报告关系

| 主报告 | 本报告 |
|---|---|
| 文件原子性 / 并发正确 / 数据一致 / 插件路径 | 结构债、锁 hygiene、日志、长期 perf 上限 |
| B6 poison（cmds 已修） | S8 边角收尾 |
| B5 embed 矩阵取舍 | P4 补充，不矛盾 |

---

## 5. 优先修复（修订）

### 本轮已做

1. **文档**：本文件核验修订（状态表 + 降级错误优先级）
2. **S8**：统一 poison-safe `lock_mutex`，清生产路径 `Mutex::lock().unwrap()`
3. **P5**：清理生产调试 `console.log`
4. **S6**：`t_sqlite::query_builder_tests` 14 项（search + smart-rule SQL 形状）

### 明确不做（本稿）

- S10 `face_indices` → `Arc<[usize]>`（ROI 低）
- P1「解码移出锁」（已完成）
- S1/S2 大拆、P4 mmap、S4 依赖倒置、P6 事件化

### 后续值得做（有带宽时）

| 优先级 | 项 |
|---|---|
| 中 | S6 剩余：`insert`/`update` DB 集成 fixture |
| 中 | S1：拆 `AFile::new`（独立 PR，需元数据 fixture） |
| 中 | P1 剩余：CLIP 锁/session 粒度（有大库 perf 证据再做） |
| 低 | S2 顺手 composable；S7 expect；S9 常量化 |

---

## 6. 项目健康总评（修订）

| 维度 | 评分 | 说明 |
|---|---|---|
| 复杂度 | B | 均值可控，头部函数超标 |
| 死代码 | A | 插件树假阳性为主 |
| 依赖循环 | B | 2 组，可控 |
| 耦合 | C→B- | 指标噪音大；枢纽文件真实 |
| 测试 | 部分↑ | embed + search top-k + **query/smart SQL 形状**；CRUD 集成仍薄 |
| 综合 | **B** | 与主报告一致：工程健康，债可控 |

**总评**：主报告 B 类安全/一致性问题更关键。补充稿有价值处是 **S1/S2/S3 结构债与 P1 剩余推理串行**；不宜把静态复杂度报告当成「本周 12 项事故清单」。
