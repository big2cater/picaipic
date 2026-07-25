# 黑洞主题扭曲增强 + 配色解耦 设计稿

> 状态：设计稿 v1（待评审）
> 范围：纯前端（`src-vite`），不触及 Rust / Tauri / DB / AI
> 分支：`feat/black-hole-idle-theme`（基于 v1.4 已落地的主题菜单 + 宇宙背景 + CSS warp）
> 目标：①把照片预览的引力扭曲从「刚体位移」升级为「撕裂+色散+径向模糊+透镜环」多层视觉；②让「黑洞」特效主题不受配色模式影响，配色模式（明亮/暗黑）只对默认/复古/CMYK 生效；③新增「动态主题强度」调节，为未来更多动态主题预留扩展。

---

## 0. 相对 v1.4 的修订摘要

| v1.4 | 本稿 |
|---|---|
| 卡片 warp = `translate+rotate+scale+blur` 刚体 | 6 层叠加：位移旋转 / 各向异性拉伸 / 径向模糊 / 色散错位 / 横向撕裂切片 / 透镜环光（CSS filter + 伪元素） |
| shader `u_appearance` 跟随 `settings.appearance` 调色 | 黑洞主题下 `u_appearance` 恒为 1（暗）；shader 源码不改，只改上游传值 |
| `isBlackHoleTheme` 读 appearance 查活动槽（v1.4 现状） | **保持不变**（不改签名）；黑洞持久靠 §3.3 双钉两槽=3 + §3.4 appearance 置灰，不靠「任一=3」检测 |
| 黑洞主题下 appearance 仍可切换且影响 chrome | appearance select 在黑洞主题下 `disabled` + 灰色 + hint；`setTheme` 黑洞时钉 `data-theme=dark` |
| 无强度调节 | 新增 `settings.dynamicThemeIntensity`（0/0.5/1/1.5，select），仅动态主题显示，影响扭曲层不影响黑洞本体 |

---

## 1. 架构总览

```
选「黑洞」主题 (themeId=3, lightTheme 与 darkTheme 均钉为 3)
   ├─ setTheme: data-theme 钉为 'dark' (UI chrome 统一暗底座)
   ├─ Settings: appearance select disabled + 灰色 + hint
   │           dynamic_theme_intensity select 显示 (仅动态主题)
   ├─ isBlackHoleTheme: 保持 v1.4 签名 (读 appearance 查活动槽)；黑洞持久靠双钉两槽=3 + appearance 置灰
   └─ Home.vue 挂 BlackHoleBackground:
        ├─ shader u_appearance 恒为 1 (dark) — 宇宙调色固定
        ├─ useGravityWarp (强度 I = dynamicThemeIntensity):
        │    .bh-card 外层:
        │      L1 transform  位移旋转缩放
        │      L2 transform  各向异性拉伸 (转径向轴→拉伸→转回)
        │      L3 filter      径向模糊 blur
        │      L4 filter      多层 drop-shadow 色散 (t>0.5 才上)
        │      L5 ::before    横向撕裂切片 (t>0.35 才上)
        │      L6 ::after     透镜环光扭曲
        │    每层强度乘 I；I=0 仅背景、卡片不动
        └─ reduced-motion: 整关 (与现有一致)
```

### 数据流改动点

| 文件 | 改动 |
|---|---|
| `src-vite/src/common/utils.ts` | `setTheme` 黑洞钉 `data-theme=dark`；`isBlackHoleTheme` **签名不变**（沿用 v1.4 读 appearance 查活动槽） |
| `src-vite/src/common/blackHoleMath.ts` | `CardWarp` 扩展 6 层字段；`computeCardWarp` 加 `intensity` + `swirl` 参数；`cardWarpCss` 返回 struct `{transform, filter, vars}`；新增 `readPrimaryColor()` |
| `src-vite/src/composables/useGravityWarp.ts` | 写 transform/filter + CSS 变量（`--bh-tear` 等）；性能开关（L4/L5 阈值、>80 张降级） |
| `src-vite/src/components/BlackHoleBackground.vue` | 不改 shader；`appearance` prop 由 Home 在黑洞时恒传 1 |
| `src-vite/src/views/Home.vue` | `isBlackHoleTheme` 调用不变（沿用 v1.4 签名）；黑洞时传 `appearance=1`；provide `intensity` |
| `src-vite/src/views/Settings.vue` | appearance select `:disabled="isBlackHole"` + hint；新增 dynamic_theme_intensity select（仅动态主题） |
| `src-vite/src/stores/configStore.js` | 新增 `dynamicThemeIntensity` 字段（默认 1）+ hydrate |
| `src-vite/src/assets/app.css` | `.bh-card` `::before`/`::after` 伪元素定义（读 CSS 变量） |
| `src-vite/src/locales/{en,zh}.json` | `black_hole_appearance_locked`、`dynamic_theme_intensity`、`intensity_options` |

---

## 2. 扭曲特效公式（方案 A：CSS/滤镜级）

### 2.1 距离参数（沿用）

```
cx, cy   = 卡片中心 (getBoundingClientRect, 缓存至下轮)
HX, HY   = 视口中心
dx, dy   = cx - HX, cy - HY
dist     = hypot(dx, dy)
angle    = atan2(dy, dx)
t        = clamp((R_inf - dist) / R_inf, 0, 1)
s        = smoothstep(t)
I        = dynamicThemeIntensity  (0 / 0.5 / 1 / 1.5)
```

### 2.2 六层叠加

| 层 | 视觉表现 | CSS 实现 | 强度随 (s, I) |
|---|---|---|---|
| L1 位移旋转 | 灯丝拉伸方向感 | `translate(tx,ty) rotate(rotDeg) scale(scale)` | `tx,ty` 乘 I；`scale = lerp(1,0.5,clamp(s*I,0,1))`；`rotDeg` 整段乘 I |
| L2 各向异性拉伸 | 灯丝拉伸+延展 | transform 追加 `rotate(radialAxis) scale(stretchX,stretchY) rotate(-radialAxis)` | `stretchX = lerp(1, 1.8, clamp(s*I,0,1))`；`stretchY = lerp(1, 0.7, clamp(s*I,0,1))`（I 放进 lerp 第二参数，非外乘） |
| L3 径向模糊 | 径向模糊+延展 | `filter: blur(radialBlur)` | `radialBlur = lerp(0, 4, s) * I` |
| L4 色散错位 | 灯丝拉伸+色散错位 | filter 追加多层 `drop-shadow(±dispPx,0,red/cyan)` | `dispPx = lerp(0, 6, s) * I`；仅 `t > 0.5` 启用 |
| L5 横向撕裂叠层 | 横向碎片撕裂（近似） | `.bh-card::before`：`repeating-linear-gradient` scanline 纹理 + **整体** `translateX(tearOffset)`（单伪元素无法逐带独立平移，此为近似 scanline 位移，非真·逐带撕裂） | `tearOffset = lerp(0, 8, s) * I`；仅 `t > 0.35` 启用 |
| L6 透镜环光扭曲 | 透镜环状光扭曲 | `.bh-card::after`：`box-shadow: 0 0 calc(var(--bh-ring)*20px) var(--bh-primary)` | `ringGlow = lerp(0, 0.6, s) * I` |

### 2.3 组合公式

> **扩展声明（Issue D）**：本节在现有 `computeCardWarp`（`blackHoleMath.ts:54-84`）基础上**扩展**，非重写。保留 `dx/dy/dist/angle/t/s/targetR/orbit/a2/nx/ny` 的求法不动，将 `scale/rotDeg/blur` 替换为带 I 的版本，并新增 `stretchX/stretchY/dispPx/hueShift/ring/tear` 输出。实现时**只跑本节公式一次**，不要既跑基类又套本节（否则 scale 会被套两次）。
> 另注：基类 `blur` 仅 `t>0.7` 才生效，本节改为 `lerp(0,4,s)*I` 从 `t>0` 就起糊——有意增强径向模糊延展感，非 bug。

```ts
// L1 基础位移旋转缩放（乘 I；I=0 时输出空 transform，卡片不动但仍属动态主题）
// 注：所有 s*I 都钳到 [0,1]，防未来 I 放开后 lerp 越界（建议 8）
// 注：基类 scale=lerp(1,0.45,t) 用线性 t；本节改用 s=smoothstep(t) 作观感增强，最小 0.5 vs 0.45，眼校即可
const k = clamp(s * I, 0, 1);
const tx = (nx - cx) * I;
const ty = (ny - cy) * I;
const scale = lerp(1, 0.5, k);
// rotDeg 整段乘 I（含 orbit 项 a2-angle 与 swirl 项）——否则 I=0 时 orbit 仍在转（Bug 3）
const rotDeg = (((a2 - angle) * 180) / Math.PI + swirl * s) * I;

// L2 各向异性拉伸（径向轴）——I 放进 lerp 第二参数并钳 [0,1]（Bug 2）
// 旧写法 lerp(1,1.8,s)*I 在 I=0.5 时值域 [0.5,0.9] 全<1（压缩非拉伸）、I=1.5 时 stretchY=1.05>1（膨胀）
const stretchX = lerp(1, 1.8, k);
const stretchY = lerp(1, 0.7, k);

// transform 顺序（CSS 从右到左应用）：
// 最右 rotate(-radialAxis) 先执行 → 转到径向轴 → scale 拉伸 → rotate(radialAxis) 转回 → 外层位移旋转缩放
const transform = `translate(${tx}px,${ty}px) rotate(${rotDeg}deg) scale(${scale}) `
                + `rotate(${radialAxis}rad) scale(${stretchX},${stretchY}) rotate(${-radialAxis}rad)`;

// L3+L4 filter
const radialBlur = lerp(0, 4, s) * I;     // I=0 → 0
const dispPx = lerp(0, 6, s) * I;
const hueShift = lerp(0, 20, s) * I;
// L4 启用条件：t>0.5 且 I>0；I>=1.5 时也降档（建议 7）——近黑洞卡多时 drop-shadow 开销大
// visibleCardCount 由 useGravityWarp 每轮传入（≈ 网格 .bh-card 总数，非"可见数"，作上限保护）
const useDispersion = t > 0.5 && I > 0 && !(I >= 1.5 && visibleCardCount > 40);
const filter = `blur(${radialBlur}px) hue-rotate(${hueShift}deg)`
             + (useDispersion
                ? ` drop-shadow(${dispPx}px 0 0 rgba(255,0,0,0.5)) drop-shadow(${-dispPx}px 0 0 rgba(0,255,255,0.5))`
                : '');

// L5/L6 CSS 变量（I=0 时为 0/transparent，伪元素不可见）
// computeCardWarp / cardWarpCss 返回 struct（Issue B），useGravityWarp 负责 el.style.setProperty 落元素
const vars = {
  '--bh-tear': `${lerp(0, 8, s) * I}px`,
  '--bh-tear-op': t > 0.35 && I > 0 ? String(s * I) : '0',
  '--bh-ring': String(lerp(0, 0.6, s) * I),
  // --bh-primary 是颜色，由 useGravityWarp 从 readPrimaryColor() 取后单独写入（不在此 struct 内，因与卡片无关、全局同值）
};

// cardWarpCss 返回：{ transform: string, filter: string, vars: Record<string,string> }
// useGravityWarp 落元素：el.style.transform = r.transform; el.style.filter = r.filter;
//   for (const [k,v] of Object.entries(r.vars)) el.style.setProperty(k, v);
//   el.style.setProperty('--bh-primary', primaryColor);  // 单独写
```

> 注：I=0 时 `transform`/`filter` 输出空或单位值（`rotate(0)`、`scale(1,1)`、`blur(0)`），`gravityActive` 仍可为 true（宇宙背景正常转），仅卡片层不动。clear 不由 intensity 触发。
> `visibleCardCount` 由 `useGravityWarp` 每轮 `querySelectorAll('.bh-card').length` 得到（≈ 网格总卡数，非"可见数"，所有缩略图都带 `.bh-card` 类），用于 L4 降档判断。
> `primaryColor` 来源：`useGravityWarp` 是独立 composable，需自行 `getComputedStyle(document.documentElement).getPropertyValue('--color-primary')` 取（黑洞时 appearance 已钉 1，拿到的是暗色强调色）；与 `BlackHoleBackground.vue` 的 `readPrimary()`（真实存在，`BlackHoleBackground.vue:153-155`）同源逻辑，建议在 `blackHoleMath.ts` 导出 `readPrimaryColor()` 复用。

### 2.4 伪元素 CSS（`app.css`，`.bh-card` 作用域）

```css
.bh-card { position: relative; }  /* 由本规则补 position:relative；Thumbnail 外层根节点无 relative 类（relative 在内部 containerRef），伪元素需本规则建立定位上下文 */

.bh-card::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 1;
  background: repeating-linear-gradient(
    to bottom,
    transparent 0,
    transparent 33%,
    rgba(255,255,255,0.08) 33%,
    rgba(255,255,255,0.08) 34%,
    transparent 34%,
    transparent 66%,
    rgba(0,0,0,0.12) 66%,
    rgba(0,0,0,0.12) 67%
  );
  mix-blend-mode: overlay;
  transform: translateX(var(--bh-tear, 0));
  opacity: var(--bh-tear-op, 0);
  transition: opacity 120ms ease-out;
}

.bh-card::after {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 2;
  border-radius: inherit;
  box-shadow: 0 0 calc(var(--bh-ring, 0) * 20px) var(--bh-primary, transparent);
  opacity: var(--bh-ring, 0);
  transition: opacity 120ms ease-out;
}
```

### 2.5 性能策略（扩展 §6.2）

| 开关 | 规则 |
|---|---|
| L4 阈值 | `t > 0.5` 才上色散 drop-shadow（远区只 L1+L2+L3+L6）；**强档降级**：`I >= 1.5 且 visibleCardCount > 40` 时也跳过 L4（drop-shadow 在强档错位大、近黑洞卡多时开销重） |
| L5 阈值 | `t > 0.35` 才上 `::before` 撕裂切片 |
| 可见卡上限 | 每轮 `querySelectorAll('.bh-card')` 若 >80 张（异常），跳过 L4/L5，只做 L1+L2 |
| 节流 | 仍 120ms/轮；帧间靠 CSS `transition`（与 Thumbnail 现有 transition 协调） |
| will-change | gravity 期间 `will-change: transform, filter`；clear 移除 |
| 滚动 | wheel/scroll → idle=false → 整表 clear（沿用 §6.3） |

### 2.6 clear / 回弹（沿用 §6.3，扩展字段）

任一条件（idle=false / isMaximized=false / 离开黑洞 / inputStack>0 / 库切换 / document.hidden / reduced-motion）触发 clear：
- **不含 intensity=0**（I=0 时 `gravityActive` 仍可 true，宇宙背景正常转，仅卡片扭曲层输出空值，不触发 clear）
- 移除内联 `transform` / `filter` / `will-change`
- 移除 CSS 变量 `--bh-tear` / `--bh-tear-op` / `--bh-ring`（数值变量置 0）；`--bh-primary` 是颜色，置 `transparent` 或移除（置 0 无效）
- 停 orbit 与有效增长
- 依赖 CSS 回到 VirtualScroll 布局位置

---

## 3. 配色与特效解耦

### 3.1 `isBlackHoleTheme` 保持 v1.4 现状（不改签名）

```ts
// 保持 v1.4：读 appearance 查活动槽（utils.ts:46-53 现有逻辑不动）
export function isBlackHoleTheme(
  appearance: number,
  lightTheme: number,
  darkTheme: number,
): boolean {
  const id = appearance === 0 ? lightTheme : darkTheme;
  return clampThemeId(id) === THEME_ID.BLACK_HOLE;
}
```

**为何不改成「任一=3」**：原稿曾想用「任一槽=3 即黑洞」防「切 appearance 时黑洞消失」，但 §3.4 已把 appearance 在黑洞主题下 `disabled`——appearance 根本切不了，这个防护是多余的，反而引入**残留槽 bug**：选黑洞（双钉两槽=3）→ 切「默认」（appearance=0 → 只写 lightTheme=0，darkTheme 残留 3）→ `isBlackHoleTheme` 若用「任一=3」会仍返回 true → **黑洞关不掉**。

**黑洞持久靠组合，不靠检测签名**：
- §3.3 选黑洞时双钉 `lightTheme=3` 和 `darkTheme=3`（让活动槽检测也稳）
- §3.4 appearance select 在黑洞下 `disabled`（appearance 不会变）
- 这两点已足够让「黑洞期间 appearance 切换不会让特效消失」；离开黑洞时写活动槽=新值即可正常关闭。

**调用点**：`Home.vue:275` 现有调用 `isBlackHoleTheme(appearance, light, dark)` 不变。

**残留槽清理（必做，见 §3.3）**：单靠「活动槽检测 + 双钉 + appearance 置灰」仍有一条复活路径——选黑洞→切默认（darkTheme 残留 3）→ 翻 appearance 到 dark → 黑洞复活。§3.3 的 setter else 分支必须清掉非活动槽的残留 3，本节标此为**必做**（非可选加固）。

### 3.2 `setTheme` 黑洞钉暗底座

```ts
export function setTheme(appearance: number, themeId: number) {
  const id = clampThemeId(themeId);
  if (id === THEME_ID.BLACK_HOLE) {
    document.documentElement.setAttribute('data-theme', 'dark');
    return;
  }
  // 其余 3 项（默认/复古/CMYK）保持原逻辑，按 appearance 选
  const theme = appearance === 0
    ? LIGHT_THEMES[id] || 'light'
    : DARK_THEMES[id] || 'dark';
  document.documentElement.setAttribute('data-theme', theme);
}
```

### 3.3 Settings 主题切换双钉

用户从 Settings 选「黑洞」时，同时写两个槽；**切走时必须清残留槽**（必做，非可选）。改写现有 `currentTheme` computed 的 set（真实名 `currentTheme`，非 `themeModel`）：

```ts
// Settings.vue 现有 currentTheme computed（Settings.vue:2223-2230）的 set 扩展
const currentTheme = computed({
  get() {
    return config.settings.appearance === 0
      ? config.settings.lightTheme
      : config.settings.darkTheme;
  },
  set(value) {
    if (value === THEME_ID.BLACK_HOLE) {
      config.settings.lightTheme = THEME_ID.BLACK_HOLE;
      config.settings.darkTheme = THEME_ID.BLACK_HOLE;
    } else {
      // 写活动槽；若非活动槽残留 3（曾选黑洞），一并清掉，防翻转 appearance 复活
      if (config.settings.appearance === 0) {
        config.settings.lightTheme = value;
        if (config.settings.darkTheme === THEME_ID.BLACK_HOLE) config.settings.darkTheme = value;
      } else {
        config.settings.darkTheme = value;
        if (config.settings.lightTheme === THEME_ID.BLACK_HOLE) config.settings.lightTheme = value;
      }
    }
  }
});
```

**为何逻辑能跑通**（顺 3 个 watch 推演）：选黑洞 → set(3) 写 lightTheme=3+darkTheme=3，各自触发 watch(5476/5480) → `setTheme(appearance,3)` → `data-theme='dark'`；切默认(appearance=0) → 写 lightTheme=0 且把残留 darkTheme:3→0，两个 watch 都走 `setTheme(0,0)='light'`，黑洞关；之后再翻 appearance 到 dark → `setTheme(1, darkTheme=0)` → `'dark'`，不复活（残留已清）。算法与真实代码机制严丝合缝。

**为何必做（残留槽复活 bug）**：若不清残留槽，复现路径——选黑洞（lightTheme=3, darkTheme=3）→ 切「默认」（appearance=0 → lightTheme=0，darkTheme 残留 3）→ 此时 `isBlackHole=false`、黑洞关（看似正常）→ 用户在默认主题下翻 appearance 到 dark（select 已解禁）→ 活动槽变 darkTheme=3 → `isBlackHoleTheme` 又返回 true → **黑洞复活**。清残留槽后两槽都干净，翻 appearance 不会复活；且只清残留 3，不破坏 v1.4 的 light/dark 独立主题（正常 retro/cmyk 互不影响）。

### 3.4 Settings UI：appearance 置灰

`isBlackHoleTheme` 是 `utils.ts` 的**函数**，Settings 模板里不能直接当布尔用，需 `computed` 包装（建议 6）：

```ts
// Settings.vue <script setup>
import { isBlackHoleTheme } from '@/common/utils';
const isBlackHole = computed(() => isBlackHoleTheme(
  Number(config.settings.appearance),
  Number(config.settings.lightTheme),
  Number(config.settings.darkTheme),
));
// 本期动态主题 === 黑洞主题（未来加 || isAuroraTheme || ...）
const isDynamicTheme = isBlackHole;
```

```vue
<select
  class="select select-bordered select-sm min-w-32"
  v-model="config.settings.appearance"
  :disabled="isBlackHole"
  :class="{ 'opacity-50 cursor-not-allowed': isBlackHole }"
>
  <option v-for="(item, index) in appearanceOptions" :key="index" :value="item.value">{{ item.label }}</option>
</select>
<span v-if="isBlackHole" class="text-xs opacity-60 mt-1">
  {{ $t('settings.general.black_hole_appearance_locked') }}
</span>
```

- `config.settings.appearance` 值仍保留（黑洞期间被冻结但不丢失），切回非黑洞时立即生效。
- 边界：用户在黑洞主题下，appearance select 置灰但值不变；切回默认/复古/CMYK → `isBlackHole=false` → 立即解禁、`setTheme` 恢复读 appearance、宇宙层卸载、扭曲 clear。

### 3.5 背景层不调 shader

- `BlackHoleBackground.vue` shader 源码不改（避免回归）。
- `Home.vue` 在黑洞主题下传 `appearance=1`（暗），等价于 shader 走暗调色固定。
- `watch(() => props.appearance, refreshPrimary)` 保留（默认/复古/CMYK 主题下不挂本组件，无副作用）。

---

## 4. 动态主题强度（为多动态主题预留）

### 4.1 新增设置字段

```js
// configStore.js
settings: {
  // ...existing...
  dynamicThemeIntensity: 1,  // 0=关 0.5=弱 1=标准 1.5=强（动态主题共用）
}
```

- 命名 `dynamicThemeIntensity`（非 `blackHoleIntensity`），为后续「极光」「赛博」等动态主题复用。
- 枚举 `[0, 0.5, 1, 1.5]`，UI 用 **select**（4 项：关/弱/标准/强），与 appearance/theme 风格一致。
- 默认 `1`（标准）；老配置无该字段 → hydrate 填 `1`。

### 4.2 作用域

- 仅当当前主题为「动态主题」时，Settings 显示该行；非动态主题隐藏。
- 派生：`isDynamicTheme`（**computed**，定义见 §3.4；本期 `=== isBlackHole`；未来 `|| isAuroraTheme || ...`）。模板里 `v-if="isDynamicTheme"` 引用该 computed。

### 4.3 对扭曲层的影响

强度 `I` 乘到 §2 每层公式（见 §2.3）：
- `I=0`：仅宇宙背景慢转，卡片不扭曲、不位移（「关」档，但仍属动态主题，宇宙层在）。
- `I=1`：标准（§2 公式基准）。
- `I=1.5`：撕裂/色散更强。
- `I=0.5`：温和扭曲。

### 4.4 不影响黑洞本体

- 强度只影响**卡片扭曲层**（L1-L6）。
- **不影响** `R_event`/`R_inf` 增长曲线、宇宙背景、吸积盘/光子环。
- 理由：半径增长代表「黑洞变大」，与「卡片扭曲强度」是两件事；强度=0 时黑洞仍正常存在（避免黑洞消失反而奇怪）。

### 4.5 Settings UI

```vue
<div v-if="isDynamicTheme" class="form-control">
  <label class="label">
    <span class="label-text">{{ $t('settings.general.dynamic_theme_intensity') }}</span>
  </label>
  <select class="select select-bordered select-sm" v-model.number="config.settings.dynamicThemeIntensity">
    <option v-for="(item, index) in intensityOptions" :key="index" :value="item.value">{{ item.label }}</option>
  </select>
</div>
```

外观区顺序：appearance → theme → (dynamic_theme_intensity if dynamic) → black_hole hint if black hole

**`intensityOptions` JS 常量**：`<select>` 的 `v-for="item in intensityOptions"` 需要 `{value, label}[]`，不能直接用纯字符串数组。仿 `appearanceOptions` 模式在 `Settings.vue` 新增：

```ts
const intensityOptions = computed(() => {
  const labels = localeMsg.value.settings.general.intensity_options; // ["关","弱","标准","强"]
  const values = [0, 0.5, 1, 1.5];
  return labels.map((label: string, i: number) => ({ label, value: values[i] }));
});
```

### 4.6 i18n 新增

```text
settings.general.black_hole_appearance_locked
  zh: 黑洞主题下配色模式不生效
  en: Color mode is locked under Black hole theme

settings.general.dynamic_theme_intensity
  zh: 动态主题强度
  en: Dynamic theme intensity

settings.general.intensity_options
  zh: ["关", "弱", "标准", "强"]
  en: ["Off", "Subtle", "Standard", "Intense"]
```

---

## 5. 接入点清单

| 文件 | 改动 |
|---|---|
| `src-vite/src/common/utils.ts` | `setTheme` 黑洞钉 `data-theme=dark`；`isBlackHoleTheme` **签名不变**（沿用 v1.4 读 appearance 查活动槽） |
| `src-vite/src/common/blackHoleMath.ts` | `CardWarp` 扩展 6 层字段；`computeCardWarp(cx,cy,HX,HY,R_event,R_inf,orbitPhase,swirl=12,intensity)`（Issue C：补 swirl 参数）；`cardWarpCss` 返回 struct `{ transform, filter, vars }`（Issue B：CSS 变量由 useGravityWarp 写入元素，非字符串内带）；新增 `readPrimaryColor()` 导出 |
| `src-vite/src/composables/useGravityWarp.ts` | 消费 `intensity`；写 CSS 变量；L4/L5 阈值开关；>80 张降级 |
| `src-vite/src/components/BlackHoleBackground.vue` | 不改 shader；props 不变（appearance 由 Home 控） |
| `src-vite/src/views/Home.vue` | `isBlackHoleTheme` 调用不变（沿用 v1.4）；黑洞时传 `appearance=1`；provide `intensity` |
| `src-vite/src/views/Settings.vue` | appearance `:disabled="isBlackHole"` + hint；改写 `currentTheme` computed set（黑洞双钉+清残留）；新增 intensity select（仅动态主题） |
| `src-vite/src/stores/configStore.js` | `dynamicThemeIntensity` 字段 + hydrate 默认 1 |
| `src-vite/src/assets/app.css` | `.bh-card::before` / `::after` 伪元素 |
| `src-vite/src/locales/en.json` / `zh.json` | `black_hole_appearance_locked`、`dynamic_theme_intensity`、`intensity_options` |
| `src-vite/src/main.js` | 无（settings 监听已有） |
| `Thumbnail.vue` / `GridView.vue` | 无（伪元素在 CSS，不动模板） |

---

## 6. 性能预算

| 场景 | CPU | GPU/合成 | 内存 | 说明 |
|---|---|---|---|---|
| 主题≠黑洞 / reduced-motion | 0 | 0 | 0 | 不挂宇宙、不扭曲 |
| 黑洞背景（WebGL） | 极低 | 低~中 | 低 | 不改 |
| 黑洞背景（Canvas2D 降级） | 极低 | 低 | 忽略 | 不改 |
| 引力扭曲（L1-L6, I=1） | 低~中（120ms/轮） | 中 | 低 | 可见卡通常 <60 张 |
| 引力扭曲（I=0） | 极低 | 低（仅背景） | 低 | 卡片不动 |
| 输入/隐藏/换主题 | 0 | 0 | 0 | clear |

- 新依赖：无
- 包体增量：≪ 0.1 MB gzip（CSS + 公式扩展）
- Rust / exe：不变

---

## 7. 自测清单（扩展 v1.4 §11）

| # | 步骤 | 期望 |
|---|---|---|
| 1 | 主题=默认/复古/CMYK | 无宇宙层、无扭曲、appearance 正常切换 chrome |
| 2 | 选黑洞 | appearance select 置灰 + hint；data-theme=dark；宇宙+黑洞 |
| 3 | 黑洞下切 appearance（若强制启用） | UI chrome 不变、宇宙调色不变、特效不消失 |
| 4 | 黑洞+最大化静止 ≥15s, I=1 | 卡片朝黑洞方向拉伸+色散+撕裂切片+透镜环；opacity 1 |
| 5 | 同上, I=0 | 卡片不动（含不旋转、不位移、不拉伸）；宇宙背景在 |
| 6 | 同上, I=1.5 | 撕裂/色散明显更强 |
| 7 | 同上, I=0.5 | 温和扭曲（拉伸>1 非<1，确认无「压缩」回归） |
| 8 | 引力中输入 | 立即回弹（transform/filter/CSS 变量全清） |
| 9 | 引力中切强度 | 实时生效（下一轮 120ms） |
| 10 | 切回默认/复古/CMYK | 立即卸宇宙 + clear warp + appearance 解禁 |
| 10b | 选黑洞→切「默认」(appearance=0) | lightTheme=0、darkTheme 被清成 0（不残留 3）；isBlackHole=false；黑洞彻底关（Bug 1 回归） |
| 10c | 选黑洞→切默认→翻 appearance 到 dark | isBlackHole 仍 false、宇宙不重载、特效不复活（验证残留槽已清，复活路径堵死） |
| 11 | 卡片可见数 >80（异常） | L4/L5 跳过，只 L1+L2，不卡顿 |
| 12 | reduced-motion | 无宇宙动效层、无扭曲 |
| 13 | 旧配置无 dynamicThemeIntensity | hydrate 填 1，不报错 |
| 14 | 切换主题后 appearance 值 | 仍保留原值（黑洞期间冻结不丢失） |

---

## 8. 非目标（本期不做）

- 卡片级 WebGL 真透镜扭曲（逐像素位移）
- SVG `feDisplacementMap` 真透镜
- 强度影响黑洞本体半径增长
- 新增除黑洞外的其他动态主题（仅预留 `dynamicThemeIntensity` 字段与 `isDynamicTheme` 派生点）
- 改 Rust / IPC / DB / AI / 包体模型
- range 滑块（用 select 四档）

---

## 9. 未来可选（非本期）

- 卡片级 WebGL 真透镜扭曲（替代/增强 L2 各向异性拉伸）
- 强度档更细分（如 0/0.25/0.5/0.75/1/1.25/1.5）
- 新动态主题（极光/赛博等）接入 `isDynamicTheme` 与 `dynamicThemeIntensity`
- 强度也影响宇宙背景（如 I=0 时背景也静态）—需另议
