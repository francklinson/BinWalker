# BinWalker 构建指南

## 项目结构

```
BinWalker/
├── src/                          # Vue 前端源码
│   ├── main.ts                  # Vue 入口
│   ├── App.vue                  # 主组件
│   └── vite-env.d.ts            # 类型声明
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs              # Tauri 入口
│   │   └── commands.rs          # binwalk 命令封装
│   ├── Cargo.toml               # Rust 依赖 (含 binwalk 3.1.0)
│   ├── tauri.conf.json          # Tauri 配置
│   └── build.rs                 # 构建脚本
├── package.json                  # 前端依赖
├── vite.config.ts                # Vite 配置
├── tsconfig.json                 # TypeScript 配置
├── build.ps1                     # PowerShell 构建脚本
└── build.bat                     # CMD 构建脚本
```

## 环境要求

| 工具 | 版本要求 | 安装命令 |
|------|----------|----------|
| Node.js | 18+ | `winget install OpenJS.NodeJS.LTS` |
| Rust | 1.70+ | `winget install Rustlang.Rustup` |
| Visual Studio Build Tools | 2022 | `winget install Microsoft.VisualStudio.2022.BuildTools` |
| WebView2 Runtime | - | Windows 10/11 通常已预装 |

## 快速开始

### 1. 检查环境

```powershell
# PowerShell
.\build.ps1 check

# 或 CMD
build.bat check
```

### 2. 开发模式

```powershell
.\build.ps1 dev
```

这会启动 Vite 开发服务器和 Tauri 应用，支持热重载。

### 3. 生产构建

```powershell
.\build.ps1 build
```

构建完成后：
- 可执行文件: `src-tauri\target\release\BinWalker.exe`
- 安装包: `src-tauri\target\release\bundle\nsis\`

### 4. 运行程序

```powershell
.\build.ps1 run
```

## 手动构建步骤

如果脚本有问题，可以手动执行：

```powershell
# 1. 安装前端依赖
npm install

# 2. 构建前端
npm run build

# 3. 构建 Rust 后端
cd src-tauri
cargo build --release

# 4. 运行
.\target\release\BinWalker.exe
```

## 核心功能

### binwalk 集成

binwalk 3.1.0 已通过 Cargo 依赖集成：

```toml
# src-tauri/Cargo.toml
[dependencies]
binwalk = "3.1.0"
```

### 可用命令

| 命令 | 功能 | 参数 |
|------|------|------|
| `scan_file` | 扫描固件签名 | `path: String` |
| `get_entropy` | 计算文件熵值 | `path: String, block_size: usize` |
| `extract_file` | 提取嵌入文件 | `path: String, output_dir: String` |

### 前端调用示例

```typescript
import { invoke } from "@tauri-apps/api/core";

// 扫描文件
const results = await invoke("scan_file", { path: "/path/to/firmware.bin" });

// 获取熵值
const entropy = await invoke("get_entropy", { 
  path: "/path/to/firmware.bin", 
  blockSize: 1024 
});

// 提取文件
const message = await invoke("extract_file", { 
  path: "/path/to/firmware.bin", 
  outputDir: "/output/dir" 
});
```

## 常见问题

### 1. 构建时提示 "link.exe not found"

需要安装 Visual Studio Build Tools：
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

### 2. Cargo 下载依赖慢

配置国内镜像，编辑 `.cargo/config.toml`：
```toml
[source.crates-io]
replace-with = 'rsproxy-sparse'

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/crates.io-index/"

[net]
git-fetch-with-cli = true
```

### 3. npm 安装慢

配置淘宝镜像：
```powershell
npm config set registry https://registry.npmmirror.com
```

### 4. WebView2 未安装

下载地址：https://developer.microsoft.com/en-us/microsoft-edge/webview2/

## 技术栈

| 组件 | 技术 |
|------|------|
| 前端框架 | Vue 3 + TypeScript |
| 构建工具 | Vite |
| 桌面框架 | Tauri 2.x |
| 固件分析 | binwalk 3.1.0 (Rust) |
| 打包工具 | NSIS |

## 开发计划

- [x] 基础项目结构
- [x] binwalk 集成
- [x] 文件扫描功能
- [x] 熵值分析功能
- [x] 文件提取功能
- [ ] 熵值可视化图表
- [ ] Hex 查看器
- [ ] 进度条显示
- [ ] 深色模式
- [ ] 固件对比功能

## 许可证

MIT License
