# BinWalker

**一款基于 Binwalk 的现代化固件安全分析工具**


## 📖 简介

BinWalker 是一款专为安全研究人员和嵌入式开发者设计的固件分析工具。它基于强大的 Binwalk 引擎，提供直观的图形界面，帮助用户快速识别固件中的安全风险、文件系统和可执行文件。

## ✨ 核心特性

### 🔍 智能扫描
- **深度递归扫描** - 支持多层嵌套的压缩文件和解包分析
- **多格式支持** - 识别 100+ 种固件组件和文件格式
- **风险等级分类** - 自动将检测结果分为 5 个风险等级（严重/高/中/低/信息）

### 📦 自动提取
- **一键提取** - 自动解压和提取固件中的所有组件
- **递归解包** - 支持 SquashFS、JFFS2、CramFS 等文件系统
- **多压缩格式** - Gzip、Bzip2、XZ、LZMA、Tar 等

### 🎯 可视化分析
- **统计概览** - 实时显示扫描结果统计数据
- **风险高亮** - 颜色编码的风险等级标识
- **详情展开** - 点击行项查看完整元数据
- **排序筛选** - 支持按偏移量、大小、风险等级等多维度排序

### 🚀 便捷功能
- **快速复制** - 一键复制偏移量、路径等信息
- **数据导出** - 支持 CSV 和 JSON 格式导出
- **文件定位** - 直接打开提取文件所在目录
- **悬浮提示** - 风险等级按钮显示详细说明

## 🖥️ 界面预览

### 主界面
- 统计卡片展示扫描概况
- 风险等级快速筛选
- 层级标签过滤嵌套内容

### 扫描结果
- 可展开的详情行
- 置信度可视化进度条
- 风险等级颜色标识

### 提取结果
- 文件列表表格
- 一键打开文件位置
- 支持排序和筛选

## 📥 安装

### 预编译版本

从 [Releases](https://github.com/francklinson/BinWalker/releases) 页面下载对应平台的安装包：

- **Windows**: `BinWalker_1.0.0_x64-setup.exe` (NSIS 安装包)
- **macOS**: `BinWalker_1.0.0_x64.dmg`
- **Linux**: `binwalker_1.0.0_amd64.deb`

### 从源码构建

#### 环境要求

- Node.js 18+ 
- Rust 1.70+
- Tauri 2.x

#### 构建步骤

```bash
# 1. 克隆项目
git clone https://github.com/francklinson/BinWalker.git
cd binwalker

# 2. 安装依赖
npm install

# 3. 开发模式运行
npm run tauri dev

# 4. 构建生产版本
npm run tauri build
```

构建产物位于：
- 可执行文件: `src-tauri/target/release/binwalker.exe`
- 安装包: `src-tauri/target/release/bundle/nsis/`

## 🚀 快速开始

### 1. 选择固件文件

点击"选择文件"按钮，支持以下格式：
- 固件镜像: `.bin`, `.img`, `.fw`, `.rom`, `.flash`
- 压缩文件: `.tar`, `.gz`, `.bz2`, `.xz`, `.lzma`
- 文件系统: `.squashfs`, `.jffs2`, `.cramfs`, `.ubi`, `.ubifs`
- 其他: `.hex`, `.dfu`, `.elf`, `.firmware`

### 2. 开始分析

点击"开始分析"按钮，工具将：
1. 扫描固件中的所有组件
2. 识别文件系统和可执行文件
3. 检测加密密钥和敏感信息
4. 自动提取所有组件

### 3. 查看结果

- **统计卡片**: 查看总检测项、高风险项、文件系统数量
- **风险筛选**: 点击风险等级按钮快速过滤
- **详情展开**: 点击任意行查看完整信息
- **导出数据**: 使用 CSV/JSON 按钮导出扫描结果

## 📊 风险等级说明

| 等级 | 颜色 | 说明 | 示例 |
|------|------|------|------|
| **严重** | 🔴 红色 | 加密密钥、证书、敏感凭证 | PEM、RSA、OpenSSL、AES、GPG、LUKS |
| **高** | 🟠 橙色 | 可执行文件、固件组件 | ELF、PE、UEFI |
| **中** | 🟡 黄色 | 文件系统 | SquashFS、JFFS2、CramFS、UBI |
| **低** | 🔵 蓝色 | 压缩格式 | Gzip、Bzip2、XZ、LZMA、Tar |
| **信息** | ⚪ 灰色 | 其他签名、元数据 | PNG、JPEG、PDF、Copyright |

## 🔧 技术栈

### 后端
- **Rust** - 高性能、安全的系统编程语言
- **Tauri 2.x** - 跨平台桌面应用框架
- **Binwalk** - 固件分析引擎（本地集成版本）
- **backhand** - SquashFS 文件系统解析
- **flate2** - Gzip 压缩/解压
- **bzip2** - Bzip2 压缩/解压
- **lzma-rs / oxiarc-lzma** - LZMA 压缩/解压

### 前端
- **Vue 3** - 渐进式 JavaScript 框架
- **TypeScript** - 类型安全的 JavaScript 超集
- **Vite** - 下一代前端构建工具
- **Tauri API** - 系统级功能调用

## 📁 项目结构

```
binwalker/
├── src/                    # 前端源码
│   ├── App.vue            # 主界面组件
│   ├── main.ts            # 前端入口
│   └── assets/            # 静态资源
├── src-tauri/             # 后端源码
│   ├── src/
│   │   ├── main.rs        # 后端入口
│   │   └── commands.rs    # Tauri 命令实现
│   ├── Cargo.toml         # Rust 依赖配置
│   └── tauri.conf.json    # Tauri 配置
├── local-binwalk/         # 本地 Binwalk 源码
├── package.json           # Node.js 依赖
└── README.md              # 项目文档
```

## 🎯 使用场景

### 安全研究
- 识别固件中的硬编码密钥和证书
- 发现隐藏的后门和可执行文件
- 分析固件文件系统和目录结构

### 逆向工程
- 提取固件中的文件系统和配置
- 解包压缩的固件组件
- 分析嵌入式设备的软件架构

### 漏洞挖掘
- 快速定位敏感信息（密钥、证书）
- 识别可执行文件进行静态分析
- 发现固件中的安全风险

## 🐛 已知问题

- Windows 系统需要 WebView2 Runtime（安装包会自动检测并提示安装）
- 某些非标准 LZMA 格式的 SquashFS 可能需要手动调整解压参数
- 大型固件（>1GB）扫描可能需要较长时间

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

### 开发指南

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启 Pull Request

## 📄 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件

## 🙏 致谢

- [Binwalk](https://github.com/ReFirmLabs/binwalk) - 强大的固件分析工具
- [Tauri](https://tauri.app/) - 优秀的跨平台桌面应用框架
- [Vue.js](https://vuejs.org/) - 优雅的前端框架
