# 黑洞扭曲增强 + 配色解耦 实施计划 (v1.5)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**基线：** 分支 `feat/black-hole-idle-theme`，v1.4 已落地（主题菜单 `themeId===3` 模型，无 `blackHoleMode` 布尔开关）。
**Spec：** `docs/superpowers/specs/2026-07-25-black-hole-distortion-and-color-decouple-design.md` (v1.5)。
**关联：** 旧 v1.3 idle 计划（`blackHoleMode` 布尔）已从仓库删除。产品 idle 照片主路径为 **`PhotoVortexLayer`**；本计划的 CSS 六层 warp 在 2026-07-30 起作为自动失败回退。

**本计划定位（2026-07-30）：** intensity + appearance 锁定 + `blackHoleMath`/`useGravityWarp` 均在用；GridView 仅在 PhotoVortex WebGL 初始化、捕获、上传或上下文失败时驱动 CSS warp。赛博朋克主题见 `docs/superpowers/plans/2026-07-26-cyberpunk-idle-glitch-impl.md`。

**Goal（历史）：** 在 v1.4 之上做 6 层 CSS 卡片扭曲与配色解耦。正常 live gravity 用 PhotoVortex，失败时用 CSS warp。

**Tech Stack：** Vue 3 + Pinia + 现有 `blackHoleMath.ts` / `useGravityWarp.ts`。无 Rust / 无新依赖。

**Note on tests：** `src-vite` 无 Vitest/Jest。纯数学用 Node assert 脚本验证；feature 行为用 §7 自测清单（17 case）手测。

---

## 顺序总览（依赖）

```
Task 1  configStore.js        + dynamicThemeIntensity:1 + hydrate 默认 1        ─┐
Task 2  utils.ts              setTheme 钉 dark；isBlackHoleTheme 三参数不动     ─┤（无依赖，可并行）
Task 3  blackHoleMath.ts      CardWarp 6层；computeCardWarp 加 intensity+swirl  ─┤
        readPrimaryColor() 导出                                            ─┘
Task 4  useGravityWarp.ts     消费 intensity；cardWarpCss→setProperty 写变量       ├─ 依赖 Task 3
        L4/L5 阈值；>80 降级；ResizeObserver 刷新时点；will-change              ─┘
Task 5  BlackHoleBackground.vue  不改 shader；appearance 由 Home 控              ─┐
Task 6  Home.vue              isBlackHoleTheme 三参数调用不动；黑洞传 appearance=1 ┤（无依赖）
        provide intensity                                                   ─┘
Task 7  Settings.vue          appearance :disabled+hint；currentTheme set 双钉+清残留   ├─ 依赖 Task 1/2
        intensity select + intensityOptions computed                           ─┘
Task 8  app.css               .bh-card::before/::after（z-index 1/2，与 group-hover 协调）  ─┐
Task 9  locales en/zh.json     3 key                                            ├─ Task 7 引用 key
Task 10 自测                   §7 的 17 case；性能核对（可见卡<60、>80 降级）              ─┘
```

---

## Task 1 — `configStore.js`：新增 `dynamicThemeIntensity`

- [ ] 在 `settings` 对象新增字段：
  ```js
  dynamicThemeIntensity: 1,  // 0=关 0.5=弱 1=标准 1.5=强（动态主题共用）
  ```
- [ ] 在 hydrate 逻辑里：旧配置无该字段时填 `1`（见 spec §4.1，case 13 验证）。
- [ ] 命名 `dynamicThemeIntensity`（非 `blackHoleIntensity`），为后续动态主题复用。
- [ ] 枚举 `[0, 0.5, 1, 1.5]`，UI 用 select（4 项：关/弱/标准/强）。

**核对：** spec §4.1、§5、`configStore.js` 现有字段结构。

---

## Task 2 — `utils.ts`：`setTheme` 钉暗 + 确认 `isBlackHoleTheme`

- [ ] `setTheme`：当 `clampThemeId(themeId) === THEME_ID.BLACK_HOLE` 时，`document.documentElement.setAttribute('data-theme', 'dark')` 并 `return`（spec §3.2）。
- [ ] `isBlackHoleTheme(appearance, lightTheme, darkTheme)`（utils.ts:46-53）**三参数签名保持不变**，本次不改（spec §3.1）。仅确认现状与 v1.5 一致。
- [ ] 不动 `migrateThemeSettings` 的 `blackHoleMode` 遗留清零逻辑（spec 未涉及，且旧 plan 的 `blackHoleMode` 持久字段已废弃，不要重新引入）。

**核对：** spec §3.1 / §3.2、§5、`utils.ts`。

---

## Task 3 — `blackHoleMath.ts`：扩展 6 层 + 强度 + 颜色

- [ ] 扩展 `CardWarp` 类型，新增 6 层字段：`stretchX, stretchY, dispPx, hueShift, ring, tear`（spec §2.2）。
- [ ] `computeCardWarp` 新增参数 `intensity` 与 `swirl`（Issue C：补 swirl 参数），签名参考：
  `computeCardWarp(cx, cy, HX, HY, R_event, R_inf, orbitPhase, swirl = 12, intensity)`（spec §5）。
- [ ] **扩展而非重写**（Issue D）：保留 `dx/dy/dist/angle/t/s/targetR/orbit/a2/nx/ny` 求法不动；把 `scale/rotDeg/blur` 替换为带 I 版本，新增 `stretchX/stretchY/dispPx/hueShift/ring/tear` 输出。**只跑本节公式一次**，勿既跑基类又套本节。
- [ ] 按 spec §2.3 实现每层（L1 乘 I；L2 用 `k=clamp(s*I,0,1)` 进 `lerp` 第二参数；L3+L4 `lerp(0,4,s)*I`；L4 启用条件 `t>0.5 && I>0 && !(I>=1.5 && visibleCardCount>40)`；L5 `t>0.35`）。
- [ ] `cardWarpCss` 改为返回 struct `{ transform: string, filter: string, vars: Record<string,string> }`（Issue B）。`vars` 含 `--bh-tear / --bh-tear-op / --bh-ring / --bh-ring-op`；**不含** `--bh-primary`（`--bh-primary` 由 `useGravityWarp` 单独写，全局同值）。
- [ ] 新增导出 `readPrimaryColor()`：封装 `getComputedStyle(document.documentElement).getPropertyValue('--color-primary')`（与 `BlackHoleBackground.vue` 的 `readPrimary()` 同源，复用之）。
- [ ] I=0 时 `transform`/`filter` 输出单位值（`rotate(0)`/`scale(1,1)`/`blur(0)`），`vars` 全 0/transparent；clear 不由 intensity 触发。

**核对：** spec §2.1–§2.3、§3.1、§5、`blackHoleMath.ts` 现有 `computeCardWarp`（:54-84）。

---

## Task 4 — `useGravityWarp.ts`：写 CSS 变量 + 阈值 + 降级

- [ ] 从 `inject`/props 消费 `intensity`（`dynamicThemeIntensity`）；每轮把 `I` 传入 `computeCardWarp`。
- [ ] 落元素：
  ```ts
  const r = cardWarpCss(...);
  el.style.transform = r.transform;
  el.style.filter = r.filter;
  for (const [k, v] of Object.entries(r.vars)) el.style.setProperty(k, v);
  el.style.setProperty('--bh-primary', primaryColor);  // readPrimaryColor() 取
  ```
- [ ] L4 阈值：`t>0.5` 才上色散 drop-shadow；强档降级：`I>=1.5 && visibleCardCount>40` 跳过 L4（spec §2.5）。
- [ ] L5 阈值：`t>0.35` 才上 `::before` 撕裂切片。
- [ ] 可见卡上限：每轮 `querySelectorAll('.bh-card').length` 若 `>80` 跳过 L4/L5，只做 L1+L2（spec §2.5；注：GridView 虚拟滚动，通常 <60，>80 为安全上限）。
- [ ] **ResizeObserver 刷新时点**：监听 GridView 缩放 / 布局变化时，失效并重算 `cx/cy`（`getBoundingClientRect` 缓存）；否则卡片中心漂移导致扭曲方向错乱。
- [ ] **will-change 合成器行为**：gravity 期间 `will-change: transform, filter`；clear 时移除。注意多 `filter`（blur + hue-rotate + drop-shadow）叠加时合成器走 GPU 层，避免与现有 `group-hover` transform 冲突。
- [ ] 节流仍 120ms/轮；wheel/scroll → idle=false → 整表 clear（沿用 §6.3）。
- [ ] clear 时：移除内联 `transform`/`filter`/`will-change`；数值变量置 0、移除 `--bh-primary`（置 `transparent`，勿置 0）。

**核对：** spec §2.3、§2.5、§2.6、§5、claim 6。

---

## Task 5 — `BlackHoleBackground.vue`：不改 shader

- [ ] 不改动 shader 源码（防回归，spec §3.5）。
- [ ] `appearance` prop 由 Home 在黑洞主题下恒传 `1`（暗）；`watch(() => props.appearance, refreshPrimary)` 保留。
- [ ] **不**在这里做 appearance 调色切换逻辑（调色固定由上游 `appearance=1` 决定）。

**核对：** spec §1、§3.5、§5。

---

## Task 6 — `Home.vue`：三参数调用 + 传 appearance + provide intensity

- [ ] `isBlackHoleTheme(appearance, light, dark)` 调用保持三参数不变（spec §3.1，Home.vue:275）。
- [ ] 黑洞主题下向 `BlackHoleBackground` 传 `appearance=1`。
- [ ] `provide('dynamicThemeIntensity', ...)` 或在 `useGravityWarp` 注入处提供 `intensity`（供 Task 4 消费）。
- [ ] `gravityActive` 组装点不变（store + idle + reduced-motion + visibility + isMaximized + inputStack）。

**核对：** spec §1、§3.1、§3.5、§5。

---

## Task 7 — `Settings.vue`：appearance 置灰 + 双钉 + 强度 select

- [ ] `isBlackHole` computed（三参数包装 `isBlackHoleTheme`，spec §3.4）：
  ```ts
  const isBlackHole = computed(() => isBlackHoleTheme(
    Number(config.settings.appearance),
    Number(config.settings.lightTheme),
    Number(config.settings.darkTheme),
  ));
  const isDynamicTheme = isBlackHole;  // 本期动态主题===黑洞；未来扩展 ||
  ```
- [ ] appearance `<select>` 加 `:disabled="isBlackHole"` + `:class="{'opacity-50 cursor-not-allowed': isBlackHole}"`；黑洞下显示 hint `&#123;&#123; $t('settings.general.black_hole_appearance_locked') &#125;&#125;`（spec §3.4）。
- [ ] 改写 `currentTheme` computed 的 `set`（真实名 `currentTheme`，非 `themeModel`）：选黑洞时双钉 `lightTheme=darkTheme=3`；切走时写活动槽并清非活动槽残留 3（**必做**，防翻 appearance 复活，spec §3.3 + case 10b/10c）。
  ```ts
  set(value) {
    if (value === THEME_ID.BLACK_HOLE) {
      config.settings.lightTheme = THEME_ID.BLACK_HOLE;
      config.settings.darkTheme = THEME_ID.BLACK_HOLE;
    } else {
      if (config.settings.appearance === 0) {
        config.settings.lightTheme = value;
        if (config.settings.darkTheme === THEME_ID.BLACK_HOLE) config.settings.darkTheme = value;
      } else {
        config.settings.darkTheme = value;
        if (config.settings.lightTheme === THEME_ID.BLACK_HOLE) config.settings.lightTheme = value;
      }
    }
  }
  ```
- [ ] 新增 `dynamic_theme_intensity` `<select>`（`v-if="isDynamicTheme"`，spec §4.5），`v-model.number="config.settings.dynamicThemeIntensity"`，`v-for="item in intensityOptions"`。
- [ ] `intensityOptions` computed（spec §4.5）：`labels = localeMsg.value.settings.general.intensity_options`（["关","弱","标准","强"]）+ `values=[0,0.5,1,1.5]` → `{label, value}[]`。
- [ ] 外观区顺序：appearance → theme → (dynamic_theme_intensity if dynamic) → black_hole hint if black hole（spec §4.5）。

**核对：** spec §3.3、§3.4、§4.5、§5。

---

## Task 8 — `app.css`：`.bh-card` 伪元素

- [ ] 补 `.bh-card { position: relative; }`（建立伪元素定位上下文，spec §2.4）。
- [ ] `.bh-card::before`（z-index:1）：`repeating-linear-gradient` scanline + `transform: translateX(var(--bh-tear,0))` + `opacity: var(--bh-tear-op,0)` + `mix-blend-mode: overlay` + `transition: opacity 120ms ease-out` + `pointer-events: none`。
- [ ] `.bh-card::after`（z-index:2）：`box-shadow: 0 0 calc(var(--bh-ring,0)*20px) var(--bh-primary, transparent)` + `opacity: var(--bh-ring-op,0)` + `transition` + `pointer-events: none`。
- [ ] **z-index 协调**：`::before`=1 / `::after`=2 在卡片内容之上、但在现有 `group-hover` 遮罩/选中框之下或之上需确认——避免遮挡缩略图交互。与 `group-hover` 叠加时伪元素 `pointer-events:none` 保证不抢点击。

**核对：** spec §2.4、§5、claim 6。

---

## Task 9 — `locales/en.json` / `zh.json`：3 个 key

在 `settings.general` 下新增（spec §4.6）：
- `black_hole_appearance_locked`：zh「黑洞主题下配色模式不生效」/ en「Color mode is locked under Black hole theme」
- `dynamic_theme_intensity`：zh「动态主题强度」/ en「Dynamic theme intensity」
- `intensity_options`：zh `["关","弱","标准","强"]` / en `["Off","Subtle","Standard","Intense"]`

**核对：** spec §4.6、§5。

---

## Task 10 — 自测（spec §7，17 case）

- [ ] 逐条跑 §7 自测清单：case 1–14（含 10b/10c 残留槽复活路径堵死验证）、case 11（>80 降级）、case 12（reduced-motion 零开销）、case 13（hydrate 默认 1）、case 14（appearance 值保留）。
- [ ] 性能核对：黑洞+最大化静止时可见卡通常 <60 张；构造 >80 张场景确认 L4/L5 跳过不卡顿（spec §2.5）。
- [ ] 纯数学用 Node assert 脚本校验 `computeCardWarp` / `cardWarpCss` 在 I=0 输出单位值、I=1.5 撕裂/色散更强、I=0.5 拉伸>1（无压缩回归，case 7）。
- [ ] git 提交前在 `dev` 模式 + 打包 exe 各验证一次（打包行为 dev 成功不保证）。

**核对：** spec §7、§6。

---

## 验收门禁

- [ ] 全部 17 case 通过。
- [ ] `cargo check` / 前端 build 无新增错误（本次仅前端，无 Rust 改动）。
- [ ] GROW：更新 `.mex/ROUTER.md`（已含 v1.5 引用）、`.mex/context/` 与匹配 `patterns/`（如本次可归纳为 `patterns/change-black-hole-distortion.md`），`mex log` 记录决策/风险。

> 注：本计划覆盖 spec v1.5 的 §5（10 文件改动）、§4.6（3 i18n key）、§7（自测清单）。v1.4 §11 case 3b 已被本 spec §7 case 3 替代（appearance 在黑洞下 disabled）。`isBlackHoleTheme` 全程三参数签名，与 utils.ts:46-53 一致。
