<div align="center">
  <img src="../docs/public/icon.png" alt="PicAiPic Logo" width="120" style="border-radius: 20px">
  <h1>PicAiPic - 私人本地照片管理器</h1>
  <h3>本地优先桌面照片管理器，Windows 为完整验证平台；Linux 构建为实验性、尚未完全验证。</h3>
  <p>
    <a href="https://github.com/big2cater/picaipic/releases"><img src="https://img.shields.io/github/v/release/big2cater/picaipic" alt="GitHub release"></a>
    <a href="https://github.com/big2cater/picaipic/releases"><img src="https://img.shields.io/github/downloads/big2cater/picaipic/total" alt="GitHub all releases"></a>
    <a href="https://github.com/big2cater/picaipic/stargazers"><img src="https://img.shields.io/github/stars/big2cater/picaipic" alt="GitHub stars"></a>
  </p>
</div>

[English](../README.md) | 简体中文

PicAiPic 是一款本地优先的照片管理器，旨在帮助您轻松浏览家庭相册、快速找回旧照片，并离线管理庞大的个人多媒体库。
它直接使用现有文件夹，索引、缩略图、元数据、语义搜索、人脸处理与编辑均在本机完成，不要求云账号或上传媒体。

- 演示视频: [https://youtu.be/RbKqNKhbVUs](https://youtu.be/RbKqNKhbVUs)
- 隐私策略: [PRIVACY.md](../PRIVACY.md)

## 下载 PicAiPic

打开 [最新版本发布页面](https://github.com/big2cater/picaipic/releases/latest)，下载匹配您系统的文件：

| 平台 | 安装包 | 备注 |
| :-- | :-- | :-- |
| **Windows 10/11 (x64 / ARM64)** | `_x64_en-US.msi` / `_arm64_en-US.msi` | 未签名 — 如果 SmartScreen 阻止下载，请点击**仍要保留** |
| **Linux (amd64 / arm64)** | `_amd64.deb` / `_arm64.deb` | 实验性构建，适用于 Debian 系发行版（Ubuntu、Debian、Linux Mint 等）——**尚未完全验证** |

## 界面预览

<p align="center">
  <img src="../docs/public/screenshots/picaipic-v1.1-smart-tags.png" alt="PicAiPic v1.1 智能标签与大图库界面" width="1100">
</p>

<p align="center"><em>智能标签视图：RAW/JPEG 参数徽标、虚拟化浏览与黑洞主题。</em></p>

## 为什么选择 PicAiPic

- **本地优先设计**：照片保存在您自己的硬盘上，无需云账号或强制上传。
- **不锁定媒体库**：直接使用现有文件夹，而不是把所有内容导入封闭数据库。
- **私有 AI 工具**：语义搜索、相似图片、智能标签、embedding 和人脸功能都在本机运行。
- **面向大图库构建**：虚拟化浏览、暖重扫缓存、批量本地推理与精确向量搜索面向 10k-100k+ 文件图库。
- **无订阅或强制生态**：应用与本地数据始终由用户掌控。

## 功能特性

### 浏览与整理

- **流畅虚拟化网格**：缩略图尺寸、拍摄参数徽标、日期分组、多选、排序与滚动条快速跳转。
- **多文件夹图库**：增量扫描、缩略图/embedding 复用、拖放与粘贴导入、文件系统同步。
- **丰富视图**：时间线、日历、文件夹、地点/地图、相机、镜头、标签、收藏、评分、人物与文件类型。
- **智能标签**：人物、宠物、风景、建筑、植物等本地视觉分类。
- **智能相册**：all/any 组合规则、日期、尺寸、人物、相机/镜头与排序。
- **集合与去重**：不移动原文件的手动集合，以及精确哈希/视觉相似去重。

### 本地 AI 与外部工具

- **本机语义搜索**：中英文文本搜索、找相似图、智能标签与 512 维本地图像向量。
- **人脸检测与聚类**：本地 ONNX 模型与大规模近邻支持。
- **ComfyUI 集成**：导入已保存的工作流（API 格式导出，或一键转换的 UI 格式图），对你自己的 ComfyUI 服务器（桌面版或远程机器）运行——选中照片上传、提交、轮询，完成后把结果图下载回图库，以工作流派生的可读文件名导入；支持串行批量、可中断取消和可配置的 VRAM 冷却，第一张结果会自动滚动到视野中。
- **可选第三方 AI 插件**（可在设置中完全隐藏）：签名包、发布者信任、权限、bearer token、运行时/模型绑定、输入 staging、进度取消与受控结果导入。

### 编辑与创作

- **图片编辑器**：裁剪预设、旋转、翻转、缩放、曝光与色彩调整。
- **拼图、批处理和冲印**：等分/杂志/长条/自由画布，批量裁剪、扩边、水印、文字与 EXIF 时间戳，DPI 输出和系统打印。
- **照片格调与 LUT**：本地 `.cube` 库、配方、手动参数和批量应用。
- **追色与相框**：全局 Lab 参考图追色、风格 LUT 导出、EXIF 信息条、浮动/下沉模糊布局与自定义 Logo。

### 媒体体验

- **Live Photo / Motion Photo**：Apple 图+视频配对、Google 内嵌 MP4、HEIC 容器视频；长按预览、导出/转换、确认后的关键帧替换与元数据修复。详见 [Live Photo 指南](../docs/guide/live-photo.md)。
- **60+ 媒体格式**：通过 LibRaw、libheif、libjpeg-turbo、jxl-oxide、FFmpeg 与 Rust codec 支持图片、RAW 和视频。
- **五套主题**：默认、复古、CMYK、黑洞、赛博朋克；动态主题带有受控的最大化窗口空闲特效、GPU 上限感知的 WebGL 渲染，以及集显/高 DPI Windows 设备上的可见 CSS 照片回退。
- **Windows 为完整验证的平台**（x64/ARM64 包）。Linux 包（x86_64/ARM64）已构建，但属于**实验性，尚未完全验证**。

## 当前开发状态

PicAiPic 当前处于 `v1.1.0` 开发线。仓库可能已有私有多架构 draft release，但只有维护者主动发布后才算正式版本。近期已经完成：

- 大图库滚动优化：取消过期视口任务、缩略图/metadata 请求去重、卡片菜单懒挂载、虚拟列表使用稳定的 GPU 定位
- 暖重扫优化：扫描内 folder/file-state cache 与有界时间戳事务；10,343 个未变化文件从 10.164 秒降至 8.786 秒
- 本地 CLIP batch embedding、受限预处理预取、启动矩阵减内存，以及 11 万向量默认精确搜索实机验证
- AI 插件包签名、发布者信任、bearer token、权限/安装流程、运行时冲突门禁与输入文件 staging；整个插件功能可在设置中用开关完全隐藏
- ComfyUI 集成：工作流导入（API/UI 格式识别）、对用户自管服务器串行运行/批量/取消、可读结果文件名与首个结果自动定位
- 裁剪、拼图、批处理、冲印、追色、LUT/照片格调、相框、智能相册、集合和 Live/Motion Photo 等内置流程
- Windows 安装包、Linux/共享 PNG、标题栏、欢迎/关于页和文档站统一使用新版应用图标
- 动态主题跨电脑加固：主窗原生最大化同步、旧强度配置迁移、GPU 纹理/viewport 上限保护，以及黑洞/赛博朋克照片层的 CSS 失败回退
- 发布范围收敛为 Windows/Linux；已删除 Android/iOS 资源和 macOS bundle/native bridge 文件

下一阶段重点是 release 可执行文件回归、代表性冷导入/embedding profiling 与插件签名密钥轮换/撤销设计。详见 [v1.1.0 发布说明](../docs/guide/release-notes/v1.1.0.md)。

## 卸载 PicAiPic

PicAiPic 直接使用您现有的照片文件夹。卸载 PicAiPic 或删除其数据库和缓存文件，**不会**删除您的原始照片。

常规卸载只会移除应用程序。如需彻底删除 PicAiPic，请先退出 PicAiPic，卸载应用程序，然后按照对应平台的命令删除本地数据库、缩略图缓存和配置文件。

### Windows

打开 **设置 > 应用 > 已安装的应用**，找到 **PicAiPic** 并选择 **卸载**。

然后打开 PowerShell，删除所有 PicAiPic 数据库、缓存和配置文件：

```powershell
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "$env:LOCALAPPDATA\com.big2cater.picaipic"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "$env:APPDATA\com.big2cater.picaipic"
```

### Linux

对于基于 Debian 的发行版，请卸载软件包：

```bash
sudo apt remove picaipic
```

然后删除所有 PicAiPic 数据库、缓存和配置文件：

```bash
rm -rf "$HOME/.local/share/com.big2cater.picaipic" \
       "$HOME/.cache/com.big2cater.picaipic" \
       "$HOME/.config/com.big2cater.picaipic"
```

如果您在 PicAiPic 设置中选择了自定义数据库存储目录，请在确认其中仅包含 PicAiPic 数据库文件后，单独删除该目录。

## 源码编译

编译要求: Node.js 20+, pnpm, Rust stable.

```bash
# Linux 系统依赖
# sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev \
#   patchelf nasm clang pkg-config autoconf automake libtool cmake

# 克隆并编译
git clone --recursive https://github.com/big2cater/picaipic.git
cd picaipic
git submodule update --init --recursive
cargo install tauri-cli --version "^2.0.0" --locked
./scripts/download_models.sh            # Windows: .\scripts\download_models.ps1
./scripts/download_ffmpeg_sidecar.sh    # Windows: .\scripts\download_ffmpeg_sidecar.ps1
cd src-vite && pnpm install && cd ..
cargo tauri dev
```

## 支持格式

PicAiPic 支持 60+ 种照片、RAW 和视频格式。

| 类型 | 格式清单 |
| :--- | :--- |
| 常规图片 | JPG/JPEG, PNG, GIF, BMP, TIFF, WebP, HEIC/HEIF/HIF, AVIF, JXL, PSD, EXR, HDR/RGBE, TGA, JPEG 2000 (JP2/J2K/J2C/JPC/JPF/JPX), DDS, DPX, QOI |
| RAW 照片 | CR2, CR3, CRW, NEF, NRW, ARW, SRF, SR2, RAF, RW2, ORF, PEF, DNG, SRW, RWL, MRW, 3FR, MOS, DCR, KDC, ERF, MEF, RAW, MDC |
| 视频 | MP4, MOV, M4V, MKV, AVI, FLV, TS/M2TS, WMV, WebM, 3GP/3G2, F4V, VOB, MPG/MPEG, ASF, DIVX 等。Windows 和 Linux 均支持 H.264 播放；在不支持原生播放时，系统会自动进行兼容性处理。 |

### Linux 视频播放备注

在 Linux Mint/Ubuntu/Debian 上，请安装以下软件包以获得更好的视频播放支持：

```bash
sudo apt install gstreamer1.0-libav gstreamer1.0-plugins-good
```

## 技术架构

- 核心: Tauri + Rust
- 前端: Vue + Vite + Tailwind CSS
- 数据: SQLite

### 关键库

| 库 | 用途 |
| :-- | :-- |
| [LibRaw](https://github.com/LibRaw/LibRaw) | RAW 图像解码与缩略图提取 |
| [libheif](https://github.com/strukturag/libheif) | HEIC/HEIF/HIF 图像解码与预览生成 |
| [libjpeg-turbo](https://libjpeg-turbo.org/) | 快速 JPEG 解码与缩略图生成 |
| [FFmpeg](https://ffmpeg.org/) | 视频处理与缩略图生成 |
| [Video.js](https://videojs.com/) | 跨平台视频播放界面 |
| [ONNX Runtime](https://onnxruntime.ai/) | 本地 AI 模型推理引擎 |
| [CLIP](https://github.com/openai/CLIP) | 图文相似度搜索 |
| [InsightFace](https://github.com/deepinsight/insightface) | 人脸检测与识别 |
| [Leaflet](https://leafletjs.com/) | 用于地理位置照片的交互式地图 |
| [daisyUI](https://daisyui.com/) | 界面 UI 组件库 |

## 开源协议

GPL-3.0-or-later。详情请参阅 [LICENSE](../LICENSE)。

PicAiPic 基于开源照片管理器 [Lap](https://github.com/julyx10/lap)（作者
[julyx10](https://github.com/julyx10)，2024-2026，GPL-3.0-or-later）修改开发。
上游项目启发了本应用所依赖的本地优先、文件夹即图库的产品模型。
