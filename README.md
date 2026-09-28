# ModelMeter

Windows 桌面端 **DeepSeek / MiMo** API 用量监控工具，用于查看账户余额、消费统计、模型 Token 用量与历史趋势。

> 郑重声明：本项目**不是** DeepSeek 官方产品，也**不是**小米 MiMo 官方产品。

## 功能特性

### 双平台支持
- **DeepSeek**：网页登录同步，查询账户余额（总余额 / 充值余额 / 赠送余额）、平台用量 Token、V4.1 Flash（deepseek-flash）/ V4 Pro（deepseek-v4-pro）模型统计；V4 Flash 等旧名请求已路由至 V4.1 Flash 计入同一模型
- **MiMo**：小米账号登录，余额查询、用量数据展示，支持 **Token（按量计费）/ 套餐（订阅）** 两种付费模式切换
- 主面板顶部 Provider 一键切换 / 记忆上次使用平台

### 用量监控
- 今日费用、本月消费实时展示
- 按模型维度展示 Token 总量、缓存命中 / 未命中、输出 Token、请求数
- 最近 7 天消费趋势柱状图，支持按周翻页浏览历史
- 模型详情页：每日 Token 明细、缓存命中率、平均单价

### 系统托盘 & 小组件
- 托盘常驻，左键单击切换面板，右键菜单：显示面板、显示小组件、刷新、设置、退出
- 小组件（Widget）精简模式：屏幕角落常驻，顶部悬停即可展开完整面板
- 后台定时自动刷新（仅窗口可见时刷新，DeepSeek / MiMo 均支持）

### 交互与显示
- **贴边隐藏**：窗口贴到屏幕顶部自动收起为精简条，鼠标悬停自动展开
- **液态玻璃 UI**：`backdrop-filter: blur()` 动态高斯模糊 + 半透明渐变，无边框圆角，支持 Windows 10 / 11 圆角适配
- 浅色 / 深色 / 跟随系统 三态主题

### 设置项
- DeepSeek / MiMo 平台登录与退出
- MiMo 付费模式（Token / 套餐）切换
- 主题切换、刷新间隔自定义、贴边隐藏开关、开机自启

## 系统要求

- Windows 10 / 11
- Microsoft Edge WebView2 Runtime（Windows 11 通常已内置）
- Node.js 18+ 与 npm
- Rust 1.77.2+（MSVC 工具链）
- Visual Studio Build Tools（含 Desktop development with C++）

## 安装与开发

依赖就绪后，可二选一：

**调试构建并启动**

```bat
dev.bat
```

**发布构建（生成 NSIS 安装包）**

```bat
build.bat
```

产物：
- EXE：`src\target\release\ModelMeter.exe`
- NSIS：`src\target\release\bundle\nsis\`

也可手动执行：

```powershell
cd frontend
npm install
npm run build          # 构建前端（vue-tsc + vite）

cd ../src
cargo build            # 调试构建
cargo tauri build      # 发布构建并打包
```

## 使用方式

### DeepSeek（方式一：网页登录自动同步）
1. 在设置页点击"打开登录页"，弹出 DeepSeek 平台登录窗口
2. 完成登录后，应用自动从登录窗口提取平台用量 Token 与 Cookie，并刷新用量统计
3. 登录完成后登录窗口自动关闭

### DeepSeek（方式二：手动粘贴 Token）
用量查询失败或无法网页登录时，可从浏览器控制台执行 `JSON.parse(localStorage.userToken).value` 获取 Token 并手动同步。

> Token 可能过期，用量查询失败时重新登录即可。

### MiMo
1. 切换到 MiMo 平台，或设置页点击"打开即 MiMo 登录"
2. 在弹出的 MiMo 平台窗口完成小米账号登录
3. 应用从 WebView2 缓存提取登录态，成功后自动关闭登录窗口并展示余额与用量

### 快捷操作
- DeepSeek 面板"充值"：跳转 DeepSeek 充值页
- MiMo 面板"续费"：跳转 MiMo 套餐管理

## 数据存储

应用数据默认位于：

```text
%LOCALAPPDATA%\ModelMeter\
```

| 文件 | 说明 |
| --- | --- |
| `settings.json` | 通用设置（主题、刷新间隔、开机自启、窗口位置等） |
| `platform_token.json` | DeepSeek 平台用量 Token |
| `platform_cookies.json` | DeepSeek 平台 Cookie |
| `mimo_platform_cookies.json` | MiMo 平台 Cookie |

旧版本数据目录 `%LOCALAPPDATA%\DeepSeekDesktopAssistant\` 会在首次启动时自动迁移到 `ModelMeter`。

> **安全说明**：所有敏感凭据（用量 Token、各平台 Cookie）均通过 Windows DPAPI（`CryptProtectData`）以当前 Windows 用户身份加密后落盘，文件中以 `enc1:<hex>` 前缀标记；无前缀的值按明文读取，用于兼容旧版本数据。加密失败时会降级为明文并写入调试日志，仅限当前用户可解密——更换 Windows 用户后需重新登录。请勿提交、公开或截图这些文件的内容。

## 项目结构

```text
ModelMeter/
├── frontend/                    # Vue 3 + TypeScript 前端
│   ├── src/
│   │   ├── views/               # Dashboard / Settings / Widget / TrendDetail（DS 与 MiMo 共用，按 Provider 分支）
│   │   ├── components/          # 面板、图表、登录、图标等组件（含 Mimo* 变体）
│   │   ├── stores/              # Pinia 状态管理（DS / MiMo / 主题 / 设置 / Provider）
│   │   ├── types/               # 类型定义
│   │   ├── App.vue / main.ts    # 入口与路由
│   │   └── style.css            # 全局样式（液态玻璃 UI）
│   ├── public/                  # 图标与静态资源
│   └── package.json
├── src/                         # Tauri + Rust 后端
│   ├── src/
│   │   ├── ds/                  # DeepSeek 模块（api / commands / login / models / top_up）
│   │   ├── mimo/                # MiMo 模块（api / commands / login / models）
│   │   ├── crypto.rs            # Windows DPAPI 凭据加解密（enc1:<hex>）
│   │   ├── error.rs             # 统一结构化错误类型 AppError
│   │   ├── storage.rs           # 本地设置与凭据存储（统一加密）
│   │   ├── tray.rs              # 系统托盘、菜单与共享命令
│   │   ├── windows.rs           # 窗口管理（贴边、圆角、位置记忆、小组件、Cookie 提取）
│   │   ├── lib.rs               # 入口、命令注册、后台定时刷新
│   │   └── main.rs
│   ├── Cargo.toml               # Rust 依赖与版本
│   └── tauri.conf.json          # Tauri 窗口 / 打包 / 安全配置
├── dev.bat                      # 调试构建并启动
├── build.bat                    # 发布构建（NSIS 打包）
└── README.md
```

## 测试

```powershell
cd frontend
npm test               # 前端单元测试（Vitest，Pinia store）

cd ../src
cargo test             # Rust 后端单元测试
```

CI 在 `windows-latest` 上运行完整流程：前后端测试 → 前端编译 → `tauri build` 打包 NSIS 安装包，并上传安装包为构建产物（见 `.github/workflows/ci.yml`）。推送 `main-v2` 或手动触发时执行打包，PR 只跑测试。

## 参考与致谢

本项目的架构路线沿用作者早期的两个单平台应用 **DeepSeekDesktopAssistant** 与 **MimoDesktopAssistant**（Vue 3 + Tauri 2），合并为当前的双平台 ModelMeter；DeepSeek 模块与 MiMo 模块分别由这两套源码重构而来。

部分实现技术参考了同类的 React + Tauri 项目 **DeepSeekMonitorWindows**，主要包括：Windows DPAPI 凭据加密、WebView2 登录态 / Cookie 提取、统一结构化错误类型，以及 `WebView2Loader.dll` 随包分发。这些参考只用于具体技术点，未采用其 React 前端与后端架构。

上述参考项目的源码副本统一放在仓库根目录的 `参考项目/` 下，仅作本地参照，已被 `.gitignore` 排除，不参与构建。

上游来源（按时间顺序）：

- [JayHome137/DeepSeekMonitor](https://github.com/JayHome137/DeepSeekMonitor) — 创意与最初实现
- [Joyi-code/DeepSeekMonitorWindows](https://github.com/Joyi-code/DeepSeekMonitorWindows) — Windows 移植
- [HaoyueQin/DeepSeekMonitorWindows](https://github.com/HaoyueQin/DeepSeekMonitorWindows) — MiMo 支持与工程化（错误类型、更新器、测试）
- [KerryChia/DeepSeek_Monitor_for_Windows](https://github.com/KerryChia/DeepSeek_Monitor_for_Windows) — DPAPI 加密实现参考

## 技术栈

**前端**：Vue 3、TypeScript、Vite 5、Pinia、Vue Router、Chart.js（vue-chartjs）、Tailwind CSS、Tauri JS API 2

**后端**：Tauri 2、Rust（reqwest 0.12、tokio、chrono、webview2-com、winreg、windows、Windows DPAPI 凭据加密）

## 免责声明

本项目仅用于学习和研究目的。请遵守 DeepSeek 与小米的使用条款，合理使用相关接口，避免频繁请求。

DeepSeek / MiMo 平台页面结构、登录状态、WebView 缓存和内部用量接口都可能随时变化，本项目不保证长期可用。API Key、用量 Token、Cookie 均为敏感凭据，使用者需自行承担本机存储、账号安全、网络请求与数据展示带来的风险。