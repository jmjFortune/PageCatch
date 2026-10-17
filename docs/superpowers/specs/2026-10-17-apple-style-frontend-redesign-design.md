# PageCatch Apple (macOS HIG) 风格前端界面重塑设计规范

- **状态**: Approved
- **日期**: 2026-10-17
- **主题**: Apple macOS HIG 风格前端界面重构 (Apple Sonoma / Sequoia Aesthetic)
- **目标**: 将 PageCatch 现有纯静态前端打造为具备原生 macOS 质感的专业级视频下载管理控制台。

---

## 1. 目标与设计定位

### 1.1 背景
PageCatch 是一款基于 Rust 后端与 Docker 部署的轻量视频下载工具，具备 HLS / DASH 解析、并发控制、断点校验与批量 TXT/CSV 导入等强大底层能力。原前端界面功能健全，但在视觉细节、光影质感、交互反馈与品牌辨识度上具备极大提升空间。

### 1.2 核心目标
1. **视觉全面重塑**：严谨遵循 Apple Human Interface Guidelines (macOS Sonoma / Sequoia)，运用毛玻璃（Liquid / Frosted Glass）、高精微边框、Apple 语义色彩与 SF Pro 风格排版。
2. **主题自由度**：默认自动跟随系统 `prefers-color-scheme` 切换明暗外观，同时在侧边栏提供快速切换控件（自动 / 浅色 / 深色）。
3. **卡片式布局 (Inset Cards)**：将任务列表由平面条目升级为带平滑动态进度条、状态指示胶囊与浮起微动效的 macOS 悬浮卡片。
4. **国际化双语支持 (i18n)**：提供中文与英文双语一键无缝热切换，中文深度本地化（贴合 macOS 中文环境用语）。
5. **零运行时依赖 & 100% 后端兼容**：不引入额外构建工具链（如 Node.js / Vite / Webpack），保持纯 HTML5 + 现代化 CSS3 + 原生 ES6 架构，完全兼容既有 Rust 静态文件服务及 Docker 构建流水线。

---

## 2. 视觉设计系统 (Design System Tokens)

### 2.1 色彩与语义 Tokens
通过 CSS 自定义属性 (`:root`, `[data-theme="light"]`, `[data-theme="dark"]`) 定义完整色彩阶梯：

| Token 名称 | 浅色模式 (Light) | 深色模式 (Dark) | 用途说明 |
| :--- | :--- | :--- | :--- |
| `--mac-accent` | `#0071e3` (System Blue) | `#0a84ff` (Pro Blue) | 主操作按钮、高亮项、进度条主色 |
| `--mac-accent-hover` | `#0077ed` | `#409cff` | 悬停状态 |
| `--mac-accent-soft` | `rgba(0, 113, 227, 0.12)` | `rgba(10, 132, 255, 0.18)` | 浅色高亮背景、选中项背景 |
| `--mac-bg-canvas` | `#f5f5f7` | `#1c1c1e` | 整个视口主背景 |
| `--mac-bg-sidebar` | `rgba(238, 240, 246, 0.78)` | `rgba(32, 32, 34, 0.78)` | 磨砂毛玻璃侧边栏背景 |
| `--mac-bg-card` | `#ffffff` | `#2c2c2e` | 任务卡片、弹窗与面板主体背景 |
| `--mac-text-primary`| `#1d1d1f` | `#f5f5f7` | 主标题、重要文字 |
| `--mac-text-secondary`| `#86868b` | `#98989d` | 副标题、URL、提示辅助文本 |
| `--mac-border` | `rgba(0, 0, 0, 0.08)` | `rgba(255, 255, 255, 0.1)` | 1px 细微光泽边框 |
| `--mac-border-subtle` | `rgba(0, 0, 0, 0.04)` | `rgba(255, 255, 255, 0.05)` | 卡片内弱分隔线 |
| `--mac-success` | `#34c759` | `#30d158` | 下载完成、连接正常状态 |
| `--mac-success-soft`| `rgba(52, 199, 89, 0.12)` | `rgba(48, 209, 88, 0.18)` | 成功状态徽标背景 |
| `--mac-danger` | `#ff3b30` | `#ff453a` | 错误、失败、危险操作 |
| `--mac-danger-soft` | `rgba(255, 59, 48, 0.12)` | `rgba(255, 69, 58, 0.18)` | 失败状态徽标背景 |
| `--mac-warning` | `#ff9500` | `#ff9f0a` | 重试提示与警告 |

### 2.2 材料与毛玻璃质感 (Materials & Frosted Glass)
- **侧边栏与悬浮遮罩**：
  ```css
  backdrop-filter: blur(24px) saturate(180%);
  -webkit-backdrop-filter: blur(24px) saturate(180%);
  ```
- **窗口顶边缘高光微倒角**：
  ```css
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.6); /* 浅色 */
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.12); /* 深色 */
  ```
- **弥散环境阴影**：
  ```css
  --mac-shadow-card: 0 2px 8px rgba(0, 0, 0, 0.04), 0 1px 2px rgba(0, 0, 0, 0.02);
  --mac-shadow-card-hover: 0 8px 24px rgba(0, 0, 0, 0.08), 0 2px 6px rgba(0, 0, 0, 0.04);
  --mac-shadow-modal: 0 24px 72px rgba(0, 0, 0, 0.24), 0 4px 16px rgba(0, 0, 0, 0.08);
  ```

### 2.3 字体排版系统 (Typography)
- **字体族栈**：`-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif`；
- **等宽代码与日志**：`ui-monospace, "SF Mono", "JetBrains Mono", Menlo, Consolas, monospace`；
- **渲染平滑**：全局应用 `-webkit-font-smoothing: antialiased; -moz-osx-font-smoothing: grayscale;`；
- **数值与时间**：关键时间与大小信息使用 `font-variant-numeric: tabular-nums;` 防止跳动。

---

## 3. 组件重塑与交互规格

### 3.1 macOS 应用壳层 (App Shell)
- **顶部红黄绿窗口微装饰 (Traffic Lights)**：在侧边栏最顶部加入 macOS 经典的三色点（红 `#ff5f56`、黄 `#ffbd2e`、绿 `#27c93f`），并带有边框质感；
- **导航菜单**：
  - 任务列表项（带动态数字胶囊）与偏好设置项；
  - 激活项采用原生圆角胶囊背景（`border-radius: 8px`）及 Accent Blue 高亮；
  - 侧边栏折叠支持（自适应 72px 图标 Dock 模式）。
- **侧边栏底部控制中枢**：
  - **服务连通状态指示器**：带呼吸绿点与文字说明（Connected / 服务正常）；
  - **外观切换器 (Appearance Switcher)**：三段式胶囊控制器（自动 🔄 / 浅色 ☀️ / 深色 🌙）；
  - **语言切换器 (Locale Switcher)**：紧凑胶囊（中文 / EN）；
  - **版本信息**：显示当前 PageCatch 引擎版本。

### 3.2 顶部工具栏与分段控制器 (Toolbar & Segmented Control)
- **标题区**：大字号 `h1`，配以柔和的副标题（任务计数、实时状态）；
- **主功能按钮**：`+ 新建任务`，Apple 经典药丸形胶囊按钮（`border-radius: 20px`），带轻微阴影与按压缩放反馈；
- **分段控制器 (Segmented Control)**：
  - 凹槽式灰阶底座，内嵌滑块状白色/深灰色卡片（全部 / 进行中 / 已完成 / 失败）；
  - 点击时平滑滑向对应 Tab，并更新筛选器。

### 3.3 悬浮任务卡片 (Inset Cards)
- 每个下载任务呈现为一个独立的精致圆角立体卡片（`border-radius: 12px`）；
- **卡片头部**：
  - 左侧：高清拟物化视频图标与文件名（加粗、溢出截断提示）；
  - 右侧：状态指示胶囊（下载中带脉冲微呼吸灯、完成绿胶囊、失败红胶囊）。
- **平滑进度条**：
  - 高度 6px，圆角轨道槽；
  - 进度条填充使用 Apple Blue 柔和线性渐变，支持 CSS `transition: width 0.3s ease-out` 平滑过渡，杜绝突兀跳跃。
- **任务元数据与快捷动作栏**：
  - 底部左侧：任务序号、字节大小（已载入/总量）、下载速度、重试次数、完成时间（UTC+8）；
  - 底部右侧：轻量操作图标组（复制链接、查看日志、复制输出路径、取消任务、重试、删除记录），统一为 macOS 微按钮样式。

### 3.4 模态窗口与表单 (Modals & Forms)
- **对话框升级为 macOS Floating Sheet**：
  - 背景采用深度毛玻璃全屏遮罩；
  - 对话框采用 `border-radius: 14px`，顶层加微弱高光边；
  - 动画采用 Apple 风格缩放与淡入（`scale(0.96) -> scale(1)`）；
- **输入框与文本域**：
  - 聚焦光圈：`outline: none; box-shadow: 0 0 0 3px rgba(0, 113, 227, 0.25); border-color: var(--mac-accent);`；
- **批量导入区 (Drag & Drop)**：
  - 支持拖拽 `.txt` / `.csv` 文件直接释放导入，拖拽经过时展现呼吸动画虚线边框与高亮背景提示。

---

## 4. 国际化引擎设计 (i18n Engine)

### 4.1 字典结构
在前端定义结构化字典映射 `i18n`：
```javascript
const i18n = {
  zh: {
    nav: { downloads: '下载任务', settings: '偏好设置' },
    toolbar: { all: '全部', active: '进行中', done: '已完成', fail: '失败', newTask: '新建任务', refresh: '刷新' },
    status: { pending: '排队等待', resolving: '解析中', downloading: '下载中', verifying: '校验中', ok: '已完成', duplicate: '已存在跳过', fail: '失败', cancelled: '已取消' },
    // 更多表单、设置、弹窗文案...
  },
  en: { ... }
};
```

### 4.2 响应式文本绑定
- HTML 元素通过 `data-i18n="toolbar.newTask"` 声明多语言词条；
- 通过 `applyLocale(lang)` 统一遍历更新 `textContent` 与 `title` / `placeholder`；
- 语言设置记忆在 `localStorage.getItem('pagecatch.locale')`，默认自动检测 `navigator.language`。

---

## 5. 兼容性与架构稳定性

1. **现有功能 100% 保留**：
   - 保持原有的并发控制、重试、取消、删除、日志查看；
   - 保持现有的 `import.js` 文件解析（GB18030、UTF-16、BOM 识别与去重）；
   - 保持现有的后端 API 接口不变。
2. **移动端与响应式自适应**：
   - 在窄屏（<700px 手机与平板竖屏）自动转换为流式卡片，隐藏次要边栏，底部保留安全交互区。
3. **性能保证**：
   - CSS 动效全部采用 `transform` 与 `opacity`，避免触发页面重排（Reflow）；
   - 静态资源压缩且无任何外部 CDN 强依赖，局域网/NAS/离线内网部署均可完美加载。

---

## 6. 测试与验证计划

1. **外观与主题测试**：
   - 浅色模式各元素对比度与阴影测试；
   - 深色模式各元素明暗度与文字清晰度测试；
   - 自动跟随系统（切换 OS 深浅色）即时生效测试。
2. **组件交互测试**：
   - 分段控制器标签过滤切换是否精准；
   - 任务卡片 hover 状态、按键操作（复制、日志、取消、删除）事件冒泡与弹窗呼出测试；
   - 新建任务弹窗导入 TXT/CSV 文件拖拽与粘贴测试。
3. **多语言切换测试**：
   - 中英双语切换后页面各区域文案是否完整覆盖；
   - 刷新页面后语言持久化记忆是否生效。
4. **移动端适配测试**：
   - 在 375px / 420px / 768px 多种视口宽度下的排版与无横向滚动条验证。
