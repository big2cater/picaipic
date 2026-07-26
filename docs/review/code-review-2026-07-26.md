# PicAiPic 代码可信审查报告

- 日期：2026-07-26
- 审查范围：src-tauri（Rust 后端）、src-vite（Vue 前端）跨 3 趟定向实读
- 方法：逐行/定向实读源码，初稿“全面审查”疑似项逐一核实（非凭印象）
- 结论：**未发现致命 Bug 或明确数据丢失路径**。工程质量总体很高，初稿疑似项几乎全为误报。共确认 10 项真实观察（B1–B10），其中 B9 原文对“覆盖活库”的描述**不准确**（见下方勘误）；修复状态见第 3 节。

> **勘误 / 修复状态（2026-07-26 同日二次核实 + craft）**  
> 下列项在审查后已在工作区落地，或需纠正报告措辞：  
> - §2 表中多条“已节流/已修复”反映**当前代码**，部分是审查后同一分支的修复，非审查当日原始快照。  
> - **B9 关键夸大**：`restore_databases` 每次使用**新 UUID 库路径**写入，**不会**原地覆盖已在用的库 `.db`；真实问题是 `fs::write` **非原子**与配置保存失败时的孤儿文件。已改为 temp+rename，并拒绝覆盖已存在路径。  
> - **B1/B2/B3/B4/B6/B7/B8** 已修复或加固（见 §3 状态列）。  
> - **B5/B10** 仍为设计取舍，不按缺陷硬改。

---

## 1. 覆盖范围（诚实标注）

### 已实读（3 趟）
| 层 | 文件/函数 |
|---|---|
| 渲染 | `VirtualScroll.vue`、`GridView.vue`、`blackHoleMath.ts` |
| 文件操作 | `t_cmds.rs`(move/copy/delete/import/scan/status)、`t_utils.rs`(FileTransfer/扫描/embed缓存)、`t_sqlite.rs`(prune/embed缓存/日期查询) |
| 语义/聚类 | `search_similar_images`、`t_cluster.rs`(KNN建图/Chinese Whispers/ANN策略) |
| 批量/打印 | `batchProcess.ts`、`printLayout.ts` |
| AI 插件 | `t_plugin.rs`(签名/路径校验/安装)、`t_ai_prompt.rs`(prompt解析)、`t_sandbox.rs`(防火墙/env白名单) |
| 备份恢复 | `t_storage.rs`、`t_sqlite.rs` 相关段 |
| EXIF | `t_image.rs`(`read_exif_permissive`) |
| 命令注册 | `main.rs` `generate_handler!` 结构 |
| 并发 | `Content.vue`(`runWithKeyedConcurrency`) |

### 未实读（仅声明，未读源码）
- `t_video.rs`（视频命令）、`t_migration.rs` 单测、i18n 流程
- 各内置工具**完整**运行时（裁剪/拼图/相框/调色/照片风格——本轮仅看了 `ImageEditor.vue` 预览 URL 管理）
- EXIF 写回/编辑其余路径
- 插件运行时沙箱在 macOS 上的具体实现

---

## 2. 核实为安全/误报的项（共 12 项，初稿疑似均已排除）

| 原编号 | 条目 | 核实证据 |
|---|---|---|
| 2.1 | VirtualScroll `getBoundingClientRect` | `clientHeight`/`clientWidth` + `ResizeObserver` 已正确补偿 |
| 7.1 | `processing_budget` 未生效 | 真实 `Semaphore` + `.acquire()`，并发受控 |
| 3.2 | 日期时区不一致 | EXIF 本地存 + `strftime localtime` + 前端本地午夜对齐，一致 |
| 1.1 | 黑洞 `emit('radii')` 每帧 | `emitRadiiIfChanged` 0.5px delta 守卫，已节流 |
| 1.2 | `useGravityWarp` O(n·m) | 清理/过滤走 `Set`，非笛卡尔积 |
| 3.1 | `formatFileSize` "0 KB" | `<1024` 提前 `return ...B` |
| 3.3 | `formatCaptureSettings` 前导逗号 | 正则 `/^,\s*/` 已剥离 |
| 5.1 | URL 导入无超时/无限体积 | 30s/10s 超时 + `Content-Length` + 流式硬上限 |
| 10.1 | Settings 轮询未清理 | `onUnmounted` `clearInterval` + `pollStop` |
| 8.1 | 聚类 O(n²) | 仅 `n < CLUSTER_N_EXACT` 走精确，大库走 ANN |
| 9.3 | 阈值数组传参 | 前端取数组单值赋 `params.threshold` |
| 扫描 | 取消扫描误删文件 | `delete_unseen` 仅 `scan_complete` 调用 |

---

## 3. 真实发现（B1–B10）与修复状态

| ID | 模块 | 问题（核实后） | 优先级 | 状态 |
|---|---|---|---|---|
| **B9** | 备份/恢复 | **勘误**：非“覆盖活库”。真实问题：对新库路径 `fs::write` 非原子；`save_app_config` 失败可留孤儿 `.db`。 | 🟠 中（数据安全，收窄） | **已修**：`write_file_atomic`（temp+rename）、拒绝覆盖已存在路径、配置失败清理已写文件 |
| **B1** | 文件操作 | `copyFile` 先落盘后 `addFileToDb`；入库失败曾留孤儿 | 🟠 中 | **已修**：`remove_untracked_file` + 前端 `cleanupCopyIndexOrphan`；主机 `import_file` DB 失败回滚落盘 |
| **B8** | 插件信任 | `is_path_inside` 对不存在路径 `canonicalize` 失败回退原始值 + `starts_with` 不解析 `..` | 🟡 中低 | **已修**：lexical `..` 折叠 + 优先 canonicalize；单测覆盖逃逸 |
| **B6** | AI 状态并发 | 多处 `lock().unwrap()` 锁中毒后 panic | 🟡 中低 | **已修**：`lock_mutex` → `PoisonError::into_inner` |
| **B7** | 扫描 | `index_album` 未查重即 spawn | 🟡 中低 | **已修**：入口 `album_scan_active` 短路（worker 仍有 `AlbumScanGuard`） |
| **B10** | 沙箱 | 网络沙箱默认 `NotEnforced` + soft-fail（已声明信任边界） | 🟡 设计备注 | **不改**（产品/路线图取舍） |
| **B2** | 渲染 | `scrollToItem` 顶部对齐少算 48px | 🟢 低 | **已修**：`itemTop - topPadding` |
| **B3** | 渲染 | 日期组选中态 O(组大小) 模板重算 | 🟢 低 | **已修**：`dateGroupSelectionState` Map |
| **B4** | 渲染 | `renderItems` + displayIndex 双遍 O(n) | 🟢 低 | **已修（部分）**：`renderLayout` 单遍；未做可见窗口惰性 |
| **B5** | 语义搜索 | 首搜全量 embed 矩阵（内存换速度） | 🟢 设计取舍 | **部分缓解**：进程内 cache + 后台 ANN + `warm_embed_matrix_cache`；磁盘持久化仍 deferred |

---

## 4. 第三趟新增核实结论

- **`t_ai_prompt.rs`**：4MB/2MB 预算上限、`from_be_bytes` 越界检查、`catch_unwind` 包裹——**无 Bug**；提示注入不构成安全问题（仅文本提取入库，参数化写入）。
- **`t_sandbox.rs`**：`sanitize_rule_id` 把特殊字符转 `_` 并截断 64，无 netsh 命令行注入；env 白名单拒绝 `AWS_SECRET`/`OPENAI_API_KEY` 等——**无注入风险**。
- **`t_image.rs` EXIF**：`read_exif_permissive` segment 有界（u16≤65533）、`catch_unwind`、fallback 读 128KB——**稳健**。
- **`main.rs`**：`generate_handler!` 编译期保证命令注册完整（引用未定义命令直接编译失败，前端 `api.js` 与列表对应）。
- **`ImageEditor.vue`**：预览 `createObjectURL` 配 `revoke*`——无 URL 泄漏（低优先级观察：个别分支先 create 后 revoke 旧，顺序可更稳）。

---

## 5. 建议下一步（更新）

已完成（本分支 craft）：

1. B9 恢复写盘原子化 + 配置失败清理 + 拒绝覆盖已存在路径  
2. B1 复制/import 孤儿回滚  
3. B8 `is_path_inside` 加固 + 单测  
4. B6/B7 锁中毒容忍 + 扫描入口去重  
5. B2–B4 渲染小修  

仍可选：

- B5 磁盘级 embed/ANN 持久化（大工程）  
- B10 仅在沙箱路线图推进时加深强制  
- ImageEditor object URL revoke 顺序打磨  

---

## 6. 备注

- 本报告聚焦“是否已实读并核实”，未覆盖全部 27 万行规模的全部边界路径。
- B10 是已声明的设计取舍（协作式沙箱），非本次新发现缺陷。
- 全文结论基于 2026-07-26 工作区；**§3 状态列反映同日修复后的代码**，与原始审查快照不完全相同。
