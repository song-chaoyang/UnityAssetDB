<p align="center">
  <img src="docs/images/banner.svg" alt="UnityAssetDB" width="600">
</p>

<p align="center">
  <strong>🗄️ 整个 Unity 项目的资产引用，尽在一个可查询的数据库</strong>
</p>

<p align="center">
  <a href="https://github.com/song-chaoyang/UnityAssetDB/actions/workflows/ci.yml"><img src="https://github.com/song-chaoyang/UnityAssetDB/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/song-chaoyang/UnityAssetDB/actions/workflows/release.yml"><img src="https://github.com/song-chaoyang/UnityAssetDB/actions/workflows/release.yml/badge.svg" alt="Release"></a>
  <img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust 1.75+">
  <img src="https://img.shields.io/badge/平台-macOS%20%7C%20Linux%20%7C%20Windows-blue.svg" alt="Platforms">
  <a href="LICENSE"><img src="https://img.shields.io/badge/许可证-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="README.md">🌐 English</a>
</p>

---

## 为什么选择 UnityAssetDB？

Unity 项目中的资产依赖关系会逐渐演变成错综复杂的网络。**"谁引用了这个材质？" "这个纹理能安全删除吗？" "哪些场景用了这个预制体？"** — 手动回答这些问题意味着要在数千个 YAML 文件中搜索。

UnityAssetDB 将整个 Unity 项目索引到 **SQLite 数据库** 中，在 **1 毫秒以内** 回答这些问题。

```
$ unityassetdb refs Assets/Materials/Enemy.mat

📂 Assets/Materials/Enemy.mat (2 referenced by)
────────────────────────────────────────────────────────────────
  ← Assets/Scenes/Game.unity [scene] via depends_on (guid-file)
  ← Assets/Scenes/Game.unity [scene] via instance_of
2 reference(s) total
```

### ✨ 核心特性

| 特性 | 说明 |
|------|------|
| 🔗 **引用追踪** | 通过 GUID 解析追踪任意资产的入向/出向依赖 |
| 🌳 **层级浏览器** | 导航 GameObject 树、组件绑定、Transform 父子关系 |
| 🔬 **C# 脚本分析** | 基于 tree-sitter 的类/方法提取 + MonoBehaviour 绑定 |
| 📁 **虚拟文件系统** | 以虚拟路径查询资产，支持 glob、grep 和树形浏览 |
| 🌐 **交互式 Web UI** | vis.js 依赖图，点击探索，REST API |
| 🔌 **MCP 服务器** | JSON-RPC over stdio — 集成任何 MCP 兼容工具 |
| ⚡ **极致性能** | Rust + SQLite + rayon 并行 — 每秒索引 10,000+ 文件 |
| 📦 **零依赖** | 单一静态二进制，无需运行时环境 |

---

## 快速上手

### 安装

```bash
# 下载预编译二进制（macOS / Linux / Windows）
# → https://github.com/song-chaoyang/UnityAssetDB/releases

# 或从源码构建
cargo install --git https://github.com/song-chaoyang/UnityAssetDB
```

### 三步搞定

```bash
# 1️⃣  为 Unity 项目建立索引
unityassetdb index build /path/to/your/unity/project

# 2️⃣  查询引用
unityassetdb refs Assets/Materials/Enemy.mat -p /path/to/project

# 3️⃣  启动 Web UI
unityassetdb serve /path/to/project --port 8089
```

### 实时进度反馈

8 阶段流水线渲染多行实时视图——加权总百分比 + 预计剩余时间、带吞吐量和实时提取统计的阶段进度条、以及当前正在处理的文件：

```
⠸ [00:00:12] ████████████░░░░░░░░  48% 3/8 extract · ETA 08s
⠹ ████████████████░░░░░  extract 31,422/54,120 · 2,615/s · 892K objs · 1.2M refs · 2.3 GB
  📄 Assets/Scenes/MainMenu.unity
```

结束后输出各阶段耗时分解，时间花在哪里一目了然：

```
  phase breakdown:
    scan          1.8s  ████████████████████
    register      0.6s  ██████
    extract      14.2s  ████████████████████████████
    assets        0.9s  ██
    graph         2.7s  █████
    bind          0.4s  █
    vfs           1.1s  ██
    publish       0.2s
✅ Index built in 21.4s: .unityassetdb/index.db (54,120 files, 12,830 assets, 8,221 scripts bound, 2,529 files/s)
```

### 演示：引用查询

<p align="center">
  <img src="docs/images/demo-refs.svg" alt="引用查询演示" width="800">
</p>

---

## 命令行参考

```
unityassetdb <COMMAND>

命令:
  index   索引管理 (build 构建 / sync 同步 / status 状态)
  refs    查询 VFS 路径的引用关系
  ls      列出 VFS 条目
  glob    通配符搜索
  grep    内容搜索
  read    读取 VFS 条目
  serve   启动 Web 服务器（交互式图谱 UI）
  mcp     启动 MCP 服务器（JSON-RPC over stdio）
  help    打印帮助信息
```

### 常用查询

```bash
# 谁引用了这个材质？（入向）
unityassetdb refs Assets/Materials/Enemy.mat -p /project

# 这个场景依赖什么？（出向）
unityassetdb refs Assets/Scenes/Game.unity -p /project --direction out

# 仅返回文件级结果
unityassetdb refs Assets/Materials/Enemy.mat -p /project --filter File

# 浏览资产树
unityassetdb ls Assets -p /project --depth 2

# 查找所有预制体
unityassetdb glob "*.prefab" -p /project

# 在索引内容中搜索类名
unityassetdb grep "PlayerController" -p /project

# 查看索引统计
unityassetdb index status /project
```

---

## 架构

<p align="center">
  <img src="docs/images/architecture.svg" alt="架构图" width="800">
</p>

### 工作原理

UnityAssetDB 运行 **7 阶段索引流水线**：

| 阶段 | 做什么 |
|------|--------|
| 1. **发现** | 用 `walkdir` 遍历 `Assets/`、`Packages/`、`ProjectSettings/`，解析 `.meta` 文件获取 GUID |
| 2. **提取** | 用 `serde_yaml` 解析 Unity YAML（`.unity`、`.prefab`、`.mat`、`.asset`），用 `tree-sitter` 解析 C#，提取对象、引用、声明 |
| 3. **解析** | 构建实体图：GameObject→Component 边、Transform 层级、预制体实例链接、脚本到符号的绑定 |
| 4. **物化** | 将实体投影到虚拟文件系统（VFS），生成可查询的路径和边 |
| 5. **校验** | 完整性检查 — 确认无断裂的 VFS 边 |
| 6. **发布** | 原子性 SQLite 数据库切换 |
| 7. **同步** | 增量更新 — 基于内容哈希仅重新索引变更文件 |

### SQLite 模式 — 19 张表

| 层 | 表 | 用途 |
|----|-----|------|
| 文件 | `files`, `assemblies`, `assembly_references` | 跟踪每个文件 + .meta GUID |
| YAML | `yaml_objects`, `yaml_references` | 解析后的 Unity YAML 结构 + 引用 |
| C# | `cs_declarations`, `cs_mentions`, `symbols`, `symbol_edges` | tree-sitter 提取的代码结构 |
| 图 | `assets`, `entities`, `entity_edges`, `entity_symbol_edges` | 实体关系图 |
| VFS | `vfs_entries`, `vfs_edges` | 虚拟文件系统 |
| 元数据 | `projects`, `index_diagnostics`, `rebuild_summary` | 索引元信息 |

### VFS 边类型

| 边 | 含义 |
|----|------|
| `child_of` | 目录→文件，父节点→子节点 |
| `defined_in` | 节点→定义它的文件 |
| `depends_on` | 文件→被引用文件（通过 GUID） |
| `instance_of` | 预制体实例→源预制体 |
| `binds_to` | 组件→C# 脚本类 |
| `refs` | 通用引用边 |

---

## REST API

运行 `serve` 时，以下接口可用：

| 接口 | 说明 |
|------|------|
| `GET /api/status` | 索引统计信息 |
| `GET /api/ls?path=...&depth=N` | 列出 VFS 子条目 |
| `GET /api/refs?path=...&direction=in&filter=ALL` | 引用查询 |
| `GET /api/glob?pattern=...&entry_type=ALL` | 通配符搜索 |
| `GET /api/grep?pattern=...&path=...` | 内容搜索 |
| `GET /api/read?path=...` | 读取 VFS 条目内容 |
| `GET /api/graph?path=...` | 可视化子图数据（节点 + 边） |

---

## MCP 服务器

UnityAssetDB 可作为 MCP（模型上下文协议）服务器运行，通过 JSON-RPC over stdio 暴露 7 个工具。

```bash
unityassetdb mcp
```

### 配置

添加到 MCP 客户端配置文件中：

```json
{
  "mcpServers": {
    "unityassetdb": {
      "command": "/path/to/unityassetdb",
      "args": ["mcp"]
    }
  }
}
```

### 可用工具

| 工具 | 说明 |
|------|------|
| `index_build` | 为 Unity 项目构建索引 |
| `index_status` | 显示索引统计 |
| `refs` | 查询引用图（入/出，可过滤） |
| `ls` | 列出 VFS 条目 |
| `glob` | 通配符搜索 |
| `grep` | 内容搜索 |
| `read` | 读取 VFS 条目内容 |

---

## 支持的文件类型

| 扩展名 | 类型 | 解析器 |
|--------|------|--------|
| `.unity`, `.scene` | 场景 | Unity YAML |
| `.prefab` | 预制体 | Unity YAML |
| `.mat` | 材质 | Unity YAML |
| `.asset` | ScriptableObject | Unity YAML |
| `.controller`, `.overrideController` | 动画控制器 | Unity YAML |
| `.anim` | 动画 | Unity YAML |
| `.vfx` | 视觉效果 | Unity YAML |
| `.cs` | C# 脚本 | tree-sitter |
| `.asmdef`, `.asmref` | 程序集定义 | JSON |
| `.shader`, `.hlsl` | 着色器 | 文本 |
| `.fbx`, `.png`, `.wav`, ... | 二进制 | 仅哈希 |

---

## 技术栈

| 组件 | 技术 | 原因 |
|------|------|------|
| 语言 | **Rust** | 零开销抽象、内存安全、单一二进制 |
| 数据库 | **rusqlite** (bundled) | 递归 CTE 查询、WAL 模式、零配置 |
| C# 解析 | **tree-sitter** | 增量解析、AST 提取 |
| YAML 解析 | **serde_yaml** | Unity 多文档 YAML 支持 |
| Web 服务器 | **axum** + **tokio** | 异步、类型安全路由 |
| 并行 | **rayon** | 数据并行文件提取 |
| 图可视化 | **vis.js** | 交互式网络可视化 |

---

## 性能基准

在真实 Unity 项目（URP + TextMeshPro，Apple Silicon MacBook）上实测：

| 项目规模 | 文件数 | 索引时间 | 吞吐量 | 数据库大小 |
|---------|--------|---------|--------|----------|
| 测试项目 | 10 | <0.1s | — | 48 KB |
| 真实项目（Assets + PackageCache） | 12,969 | 13.6s | ~950 files/s | 185 MB |

> 索引范围覆盖 `Assets/`、`Packages/`、`ProjectSettings/` 和 `Library/PackageCache/`（内置包脚本也能解析）。包含供 `grep`/`read` 使用的文件正文索引。

---

## 开发

```bash
cargo test      # 运行全部 32 个测试
cargo clippy    # 代码检查
cargo fmt       # 格式化
```

### 项目结构

```
src/
├── model/          # 数据类型
├── discovery/       # 文件系统扫描 + .meta 解析
├── extract/         # Unity YAML + C# tree-sitter 解析器
├── resolve/         # 实体图构建 + 脚本绑定
├── materialize/     # VFS 投影
├── db/              # SQLite 模式 + 连接管理
├── query/           # refs, ls, glob, grep, read
├── cli/             # CLI + MCP 命令处理
├── server/          # axum REST API
└── web/             # 内嵌 HTML/JS/CSS

tests/
├── fixtures/        # 最小 Unity 测试项目
└── integration_test # 32 个测试：单元 + 集成
```

---

## 下载

预编译二进制可在 [Releases](https://github.com/song-chaoyang/UnityAssetDB/releases) 页面下载：

| 平台 | 下载 |
|------|------|
| macOS (Apple Silicon) | `unityassetdb-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `unityassetdb-x86_64-apple-darwin.tar.gz` |
| Linux (x86_64) | `unityassetdb-x86_64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64) | `unityassetdb-x86_64-pc-windows-msvc.zip` |

---

## 贡献

欢迎贡献！请参阅 [CONTRIBUTING.md](CONTRIBUTING.md)。

## Star 趋势

[![Star History Chart](https://api.star-history.com/svg?repos=song-chaoyang/UnityAssetDB&type=Date)](https://www.star-history.com/#song-chaoyang/UnityAssetDB&Date)

## 许可证

[MIT](LICENSE) © 2026
