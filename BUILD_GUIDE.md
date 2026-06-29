# BinWalker 构建指南

## 项目概述

BinWalker 是一个基于 Tauri 2 + Vue 3 + binwalk 3 的固件分析工具。

## 环境要求

- **Node.js**: 18+ (已安装 v24.14.0)
- **Rust**: 1.70+ (已安装 1.96.0)
- **Visual Studio Build Tools**: 2022 (已安装)
- **WebView2 Runtime**: Windows 10/11 通常已预装

## 构建步骤

### 1. 检查环境

```powershell
.\build.ps1 check
```

### 2. 构建项目

```powershell
.\build.ps1 build
```

或者手动执行：

```powershell
# 安装前端依赖
npm install

# 构建前端
npm run build

# 构建后端
cd src-tauri
cargo build --release
```

### 3. 运行应用

```powershell
.\build.ps1 run
```

或直接运行：

```powershell
src-tauri\target\release\BinWalker.exe
```

## 开发模式

启动开发服务器（支持热重载）：

```powershell
.\build.ps1 dev
```

## 项目结构

```
BinWalker/
├── src/                          # Vue 前端
│   ├── main.ts                   # 入口文件
│   ├── App.vue                   # 主组件
│   └── vite-env.d.ts             # 类型声明
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs               # Tauri 入口
│   │   └── commands.rs           # binwalk 命令封装
│   ├── Cargo.toml                # Rust 依赖
│   ├── tauri.conf.json           # Tauri 配置
│   └── icons/
│       └── icon.ico              # 应用图标
├── package.json                  # 前端依赖
├── vite.config.ts                # Vite 配置
├── tsconfig.json                 # TypeScript 配置
└── build.ps1                     # 构建脚本
```

## 核心功能

### 1. 固件扫描 (`scan_file`)

使用 binwalk 扫描固件文件，识别嵌入的文件和数据结构。

**调用示例**:
```typescript
import { invoke } from "@tauri-apps/api/core";

const results = await invoke("scan_file", { 
  path: "C:\\firmware.bin" 
});
```

**返回格式**:
```typescript
interface ScanResult {
  offset: number;      // 偏移量
  size: number;        // 大小
  name: string;        // 名称
  description: string; // 描述
  confidence: number;  // 置信度
}
```

### 2. 熵值分析 (`get_entropy`)

计算文件的熵值分布，用于识别加密或压缩区域。

**调用示例**:
```typescript
const entropy = await invoke("get_entropy", { 
  path: "C:\\firmware.bin",
  blockSize: 1024
});
```

**返回格式**:
```typescript
interface EntropyPoint {
  offset: number;   // 偏移量
  entropy: number;  // 熵值 (0-8)
}
```

### 3. 文件提取 (`extract_file`)

根据扫描结果提取嵌入的文件。

**调用示例**:
```typescript
const message = await invoke("extract_file", { 
  path: "C:\\firmware.bin",
  outputDir: "C:\\extracted"
});
```

## 技术栈

| 组件 | 技术 | 版本 |
|------|------|------|
| 前端框架 | Vue 3 | 3.5.13 |
| 构建工具 | Vite | 6.0.3 |
| 桌面框架 | Tauri | 2.x |
| 固件分析 | binwalk | 3.1.0 |
| 类型系统 | TypeScript | 5.7.2 |

## 常见问题

### Q: 构建时提示 "link.exe not found"

**A**: 需要安装 Visual Studio Build Tools：

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

### Q: Cargo 下载依赖很慢

**A**: 配置国内镜像源，编辑 `.cargo/config.toml`：

```toml
[source.crates-io]
replace-with = 'rsproxy-sparse'

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/crates.io-index/"

[net]
git-fetch-with-cli = true
```

### Q: npm 安装依赖很慢

**A**: 配置淘宝镜像：

```powershell
npm config set registry https://registry.npmmirror.com
```

### Q: WebView2 未安装

**A**: 下载地址：https://developer.microsoft.com/en-us/microsoft-edge/webview2/

## 许可证

MIT License
