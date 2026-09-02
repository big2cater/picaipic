---
layout: page
title: PicAiPic
description: Local-first, AI-powered photo manager with a black hole for a theme.
---

<div class="bh-home">
  <section class="bh-hero">
    <canvas ref="bhCanvas" class="bh-canvas" aria-hidden="true"></canvas>

    <div class="bh-hero-inner">
      <p class="bh-eyebrow">PIC A I · PIC A PIC</p>
      <h1 class="bh-title">把整片星空，<br />收进你的相册。</h1>
      <p class="bh-tagline">
        本地优先的照片管理器 —— 浏览、搜索、整理都以你自己的磁盘为家。
        连黑洞主题都是认真的：照片会被吸进事件视界。
      </p>
      <div class="bh-cta">
        <a class="bh-btn bh-btn-primary" href="https://github.com/big2cater/picaipic/releases/latest">下载 PicAiPic</a>
        <a class="bh-btn bh-btn-ghost" href="/guide/introduction">了解 PicAiPic</a>
        <a class="bh-btn bh-btn-ghost" href="https://github.com/big2cater/picaipic">GitHub ↗</a>
      </div>
      <p class="bh-hint">← 试试移动鼠标，别让黑洞发现你</p>
    </div>
  </section>

  <section class="bh-features">
    <h2 class="bh-section-title">本地优先，<span>不交付你的回忆</span></h2>

    <div class="bh-grid">
      <article class="bh-card">
        <div class="bh-card-icon">📁</div>
        <h3>文件夹即图库</h3>
        <p>直接用你现有的照片文件夹，没有导入锁、没有专有格式绑架。卸载应用，照片原样还在。</p>
      </article>

      <article class="bh-card">
        <div class="bh-card-icon">🔒</div>
        <h3>隐私是默认值</h3>
        <p>索引、缩略图、语义搜索、人脸处理全部在本机完成。不需要云账号，也不会上传一张照片。</p>
      </article>

      <article class="bh-card">
        <div class="bh-card-icon">🧠</div>
        <h3>本地 AI 搜索</h3>
        <p>中英文语义搜索、找相似图、智能标签、人脸聚类，都跑在你自己的 CPU 上，10 万张图也跟得上。</p>
      </article>

      <article class="bh-card">
        <div class="bh-card-icon">🎨</div>
        <h3>接住你的 ComfyUI</h3>
        <p>把你保存的 ComfyUI 工作流直接跑在选中的照片上：导入、批量、取消、结果自动回到相册。</p>
      </article>

      <article class="bh-card">
        <div class="bh-card-icon">🖌️</div>
        <h3>内置创作工具</h3>
        <p>裁剪预设、拼图、批处理、冲印排版、相框、追色与 LUT 风格，全部离线可用。</p>
      </article>

      <article class="bh-card">
        <div class="bh-card-icon">⚡</div>
        <h3>为大图库而生</h3>
        <p>虚拟化网格、增量扫描、批量化推理与精确向量搜索，为 1 万到 10 万+ 文件的图库设计。</p>
      </article>

      <article class="bh-card">
        <div class="bh-card-icon">💸</div>
        <h3>免费，无订阅</h3>
        <p>GPL-3.0 开源，零付费墙。应用与数据都在你的掌控之下。</p>
      </article>

      <article class="bh-card">
        <div class="bh-card-icon">🕳️</div>
        <h3>五套主题，含黑洞</h3>
        <p>默认、复古、CMYK、赛博朋克与黑洞。空闲时照片真的会被吸进去 —— 这就是我们的品味。</p>
      </article>
    </div>
  </section>
</div>

<script setup>
import { onMounted, onBeforeUnmount, ref } from 'vue'

const bhCanvas = ref(null)
let raf = 0
let cleanup = () => {}

onMounted(() => {
  const canvas = bhCanvas.value
  if (!canvas) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
  const dpr = Math.min(window.devicePixelRatio || 1, 1.5)

  const stars = []
  const STAR_COUNT = reduceMotion ? 90 : 260
  const mouse = { x: 0.5, y: 0.42, active: false }

  let w = 0
  let h = 0

  function resize() {
    const rect = canvas.parentElement.getBoundingClientRect()
    w = Math.max(320, rect.width)
    h = Math.max(240, rect.height)
    canvas.width = Math.round(w * dpr)
    canvas.height = Math.round(h * dpr)
    canvas.style.width = `${w}px`
    canvas.style.height = `${h}px`
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  }

  function spawnStar(init = false) {
    // 粒子按高斯分布在黑洞核心附近形成吸积盘，少量远星
    const near = Math.random() < 0.72
    const r = near
      ? Math.abs((Math.random() + Math.random() + Math.random()) / 3 - 0.42) * 0.62
      : 0.55 + Math.random() * 0.45
    const a = Math.random() * Math.PI * 2
    const speed = near ? 0.12 + Math.random() * 0.9 : 0.02 + Math.random() * 0.07
    return {
      // 半径方向围绕中心 0.5/0.45，旋转
      baseR: r,
      baseA: a,
      speed,
      drift: 0.04 + Math.random() * 0.2,
      twinkle: Math.random() * Math.PI * 2,
      twinkleSpeed: 0.5 + Math.random() * 2,
      size: near ? 0.6 + Math.random() * 1.9 : 0.4 + Math.random() * 1.1,
      hue: Math.random() < 0.82 ? 200 : Math.random() < 0.5 ? 45 : 320, // 蓝白为主，暖金点缀
      init,
    }
  }

  for (let i = 0; i < STAR_COUNT; i += 1) stars.push(spawnStar(true))

  const cxT = { x: 0.5, y: 0.42 }
  function draw(now) {
    const t = now / 1000
    // 黑洞核心跟随鼠标，但保持在一个小范围内缓慢移动
    cxT.x += ((mouse.active ? mouse.x : 0.5) - cxT.x) * 0.035
    cxT.y += ((mouse.active ? mouse.y : 0.42) - cxT.y) * 0.035
    const cx = cxT.x * w
    const cy = cxT.y * h

    // 星轨背景
    const bg = ctx.createRadialGradient(cx, cy, 0, cx, cy, Math.max(w, h) * 0.75)
    bg.addColorStop(0, 'rgba(10, 12, 28, 1)')
    bg.addColorStop(0.55, 'rgba(8, 10, 24, 1)')
    bg.addColorStop(1, 'rgba(4, 6, 16, 1)')
    ctx.fillStyle = bg
    ctx.fillRect(0, 0, w, h)

    // 粒子
    for (const s of stars) {
      s.baseA += s.speed * 0.0016
      s.twinkle += s.twinkleSpeed * 0.016
      const pull = mouse.active ? 0.9 : 1
      const r = s.baseR * Math.min(w, h) * 0.62 * pull
      const x = cx + Math.cos(s.baseA) * r + Math.sin(s.baseA + s.drift) * 2
      const y = cy + Math.sin(s.baseA) * r * 0.72 + Math.cos(s.baseA + s.drift) * 2
      const alpha = 0.22 + 0.55 * (0.5 + 0.5 * Math.sin(s.twinkle))
      const size = s.size * (0.8 + 0.4 * Math.sin(s.twinkle * 1.3))
      ctx.beginPath()
      ctx.fillStyle = `hsla(${s.hue}, 85%, ${72 + 18 * Math.sin(s.twinkle)}%, ${alpha})`
      ctx.arc(x, y, size, 0, Math.PI * 2)
      ctx.fill()

      // 光子拖尾：少数近轨粒子画出小弧线
      if (s.speed > 0.7 && Math.random() < 0.08) {
        ctx.beginPath()
        ctx.strokeStyle = `hsla(${s.hue}, 90%, 78%, 0.16)`
        ctx.lineWidth = 1
        ctx.arc(cx, cy, r, s.baseA - 0.22, s.baseA)
        ctx.stroke()
      }
    }

    // 黑洞事件视界
    const pulse = reduceMotion ? 0 : Math.sin(t * 1.1) * 0.03
    const er = Math.min(w, h) * (0.085 + pulse) // event horizon
    const glow1 = ctx.createRadialGradient(cx, cy, er * 0.2, cx, cy, er * 3.2)
    glow1.addColorStop(0, 'rgba(190, 220, 255, 0.5)')
    glow1.addColorStop(0.25, 'rgba(90, 140, 255, 0.22)')
    glow1.addColorStop(0.6, 'rgba(60, 90, 200, 0.08)')
    glow1.addColorStop(1, 'rgba(0, 0, 0, 0)')
    ctx.fillStyle = glow1
    ctx.beginPath()
    ctx.arc(cx, cy, er * 3.2, 0, Math.PI * 2)
    ctx.fill()

    // 光子环：黑洞最迷人的部分
    ctx.beginPath()
    ctx.arc(cx, cy, er * 1.28, 0, Math.PI * 2)
    ctx.strokeStyle = `rgba(255, 214, 150, ${0.35 + 0.2 * Math.sin(t * 2.2)})`
    ctx.lineWidth = 2.4
    ctx.stroke()
    ctx.beginPath()
    ctx.arc(cx, cy, er * 1.28, 0, Math.PI * 2)
    ctx.strokeStyle = 'rgba(200, 235, 255, 0.16)'
    ctx.lineWidth = 9
    ctx.stroke()

    // 纯黑核心（事件视界）
    ctx.beginPath()
    ctx.arc(cx, cy, er, 0, Math.PI * 2)
    ctx.fillStyle = '#000'
    ctx.fill()
  }

  function tick(now) {
    draw(now)
    raf = requestAnimationFrame(tick)
  }

  function onPointer(e) {
    const rect = canvas.getBoundingClientRect()
    mouse.x = (e.clientX - rect.left) / rect.width
    mouse.y = (e.clientY - rect.top) / rect.height
    mouse.active = true
  }
  function onLeave() {
    mouse.active = false
  }

  resize()
  window.addEventListener('resize', resize)
  window.addEventListener('pointermove', onPointer, { passive: true })
  window.addEventListener('pointerleave', onLeave)
  if (!reduceMotion) {
    raf = requestAnimationFrame(tick)
  } else {
    draw(performance.now())
  }

  cleanup = () => {
    cancelAnimationFrame(raf)
    window.removeEventListener('resize', resize)
    window.removeEventListener('pointermove', onPointer)
    window.removeEventListener('pointerleave', onLeave)
  }
})

onBeforeUnmount(cleanup)
</script>

<style>
.bh-home {
  --bh-ink: #e8edff;
  --bh-dim: rgba(232, 237, 255, 0.62);
  --bh-line: rgba(255, 255, 255, 0.1);
}

.bh-hero {
  position: relative;
  border-radius: 28px;
  overflow: hidden;
  min-height: 560px;
  margin: 0 auto 4.5rem;
  max-width: 1180px;
  border: 1px solid var(--bh-line);
  box-shadow: 0 40px 120px rgba(0, 0, 0, 0.45);
}

.bh-canvas {
  position: absolute;
  inset: 0;
  display: block;
}

.bh-hero-inner {
  position: relative;
  z-index: 2;
  padding: clamp(2.4rem, 6vw, 5rem);
  max-width: 60%;
  pointer-events: none;
}

.bh-eyebrow {
  font-size: 0.72rem;
  letter-spacing: 0.42em;
  color: rgba(190, 220, 255, 0.75);
  margin-bottom: 1.2rem;
}

.bh-title {
  font-size: clamp(2.2rem, 5vw, 3.6rem);
  line-height: 1.12;
  font-weight: 800;
  letter-spacing: -0.03em;
  color: var(--bh-ink);
  margin: 0 0 1.1rem;
  text-shadow: 0 6px 30px rgba(0, 0, 0, 0.55);
}

.bh-tagline {
  color: var(--bh-dim);
  font-size: 1.04rem;
  line-height: 1.8;
  margin: 0 0 2rem;
  max-width: 34rem;
}

.bh-cta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.8rem;
  pointer-events: auto;
}

.bh-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  padding: 0.7rem 1.35rem;
  font-size: 0.95rem;
  font-weight: 600;
  text-decoration: none;
  transition: transform 0.2s ease, box-shadow 0.25s ease, background 0.25s ease, border-color 0.25s ease;
}

.bh-btn-primary {
  background: linear-gradient(135deg, #6aa8ff, #3b6ef0);
  color: #fff;
  box-shadow: 0 14px 40px rgba(59, 110, 240, 0.45);
}

.bh-btn-primary:hover {
  transform: translateY(-2px);
  box-shadow: 0 20px 52px rgba(59, 110, 240, 0.55);
}

.bh-btn-ghost {
  border: 1px solid rgba(255, 255, 255, 0.18);
  background: rgba(255, 255, 255, 0.05);
  backdrop-filter: blur(10px);
  color: var(--bh-ink);
}

.bh-btn-ghost:hover {
  transform: translateY(-2px);
  border-color: rgba(140, 190, 255, 0.45);
}

.bh-hint {
  margin-top: 1.6rem;
  font-size: 0.82rem;
  color: rgba(200, 225, 255, 0.5);
  letter-spacing: 0.06em;
}

.bh-features {
  max-width: 1180px;
  margin: 0 auto;
}

.bh-section-title {
  font-size: clamp(1.5rem, 3vw, 2.1rem);
  font-weight: 800;
  letter-spacing: -0.02em;
  margin: 0 0 2.2rem;
}

.bh-section-title span {
  background: linear-gradient(90deg, #6aa8ff, #f5b76a);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}

.bh-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.9rem;
}

.bh-card {
  border-radius: 20px;
  padding: 1.3rem 1.2rem 1.2rem;
  border: 1px solid rgba(255, 255, 255, 0.07);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.055), rgba(255, 255, 255, 0.02)),
    rgba(13, 15, 32, 0.55);
  transition: transform 0.22s ease, border-color 0.22s ease, background 0.22s ease;
}

.bh-card:hover {
  transform: translateY(-6px);
  border-color: rgba(120, 180, 255, 0.35);
  background:
    linear-gradient(180deg, rgba(106, 168, 255, 0.1), rgba(255, 255, 255, 0.03)),
    rgba(13, 15, 32, 0.6);
}

.bh-card-icon {
  font-size: 1.6rem;
  margin-bottom: 0.8rem;
}

.bh-card h3 {
  font-size: 1rem;
  font-weight: 700;
  margin: 0 0 0.5rem;
}

.bh-card p {
  font-size: 0.88rem;
  line-height: 1.6;
  color: var(--bh-dim);
  margin: 0;
}

@media (max-width: 1024px) {
  .bh-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}

@media (max-width: 700px) {
  .bh-hero { min-height: 480px; }
  .bh-hero-inner { max-width: 88%; padding: 2rem 1.4rem; }
  .bh-grid { grid-template-columns: 1fr; }
}
</style>