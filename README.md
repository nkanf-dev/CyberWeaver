# CyberWeaver

CyberWeaver 是一个面向安全分析场景的桌面取证画布，使用 **Tauri + React + tldraw + SQLite** 构建。

它的目标不是“再造一个白板”，而是把零散线索（IP、进程、文件、事件）沉淀成结构化数据，并在无限画布里可视化串联。

## 核心能力

- 结构化线索模型（`geo` / `text` / `note`）
- 画布加载安全校验，避免历史脏数据触发 `ValidationError`
- 画布与 SQLite 的稳定双向同步
  - 增量 upsert
  - 删除同步
  - 去抖合并写入
  - 后端事务处理
- SQLite schema 自动迁移（兼容旧表结构）
- 浏览器模式持久化回退（便于 Web 调试与 e2e）

## 技术栈

- Frontend: React 19, TypeScript, tldraw, Vite
- Desktop shell: Tauri 2
- Backend: Rust, SeaORM, SQLite
- Testing: Vitest, Playwright

## 架构概览

```text
src/
  domain/
    clueNode.ts        # 线索领域模型与校验
    shapeMapper.ts     # tldraw shape <-> 线索实体映射
    storeChanges.ts    # store diff -> 持久化增量
  hooks/
    useCanvasSync.ts   # 画布同步控制器（加载、监听、写回）
  infrastructure/
    nodeGateway.ts     # tauri 命令网关 + 浏览器回退存储

src-tauri/src/
  lib.rs               # tauri 命令、数据库初始化、迁移、同步写入
  main.rs              # tauri 入口
```

## 快速开始

### 1. 环境要求

- Node.js >= 20
- npm >= 10
- Rust stable
- Tauri 2 官方依赖（见 [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)）

### 2. 安装依赖

```bash
npm install
```

### 3. 启动开发环境

```bash
npm run tauri dev
```

仅启动前端（浏览器模式）：

```bash
npm run dev
```

## 测试

单元测试：

```bash
npm run test:unit
```

E2E 测试：

```bash
npm run test:e2e
```

构建检查：

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

## 脚本

- `npm run dev`: 启动 Vite
- `npm run build`: TypeScript 检查 + 前端构建
- `npm run test:unit`: 运行 Vitest 单元测试
- `npm run test:e2e`: 运行 Playwright e2e
- `npm run tauri dev`: 启动 Tauri 开发环境

## Roadmap

### Phase 1: 基础框架（已完成）

- [x] Tauri 桌面客户端骨架
- [x] tldraw 无限画布集成
- [x] 基础 UI 与交互搭建

### Phase 2: 核心数据层与持久化（已完成）

- [x] SQLite 初始化与 SeaORM 接入
- [x] 结构化线索实体定义（矩形 / 文本 / 便签）
- [x] 修复线索加载时的 `ValidationError`
- [x] 实现画布与 SQLite 双向稳定同步

### Phase 3: 同构旁路与可视化增强（计划中）

- [ ] Axum + WebSocket 实时双向通信
- [ ] 后端反向驱动画布操作
- [ ] 自动布局（如 Elkjs）

### Phase 4: 智能体与自动化取证（计划中）

- [ ] Python 分析脚本 / Agent 插件接口
- [ ] 攻击路径推理与威胁评分
- [ ] 取证报告导出

## 提交规范

请使用 Conventional Commits：

- `feat: ...`
- `fix: ...`
- `refactor: ...`
- `test: ...`
- `docs: ...`

## 贡献

欢迎提交 Issue 和 PR。建议在提交前本地执行：

```bash
npm run build
npm run test:unit
npm run test:e2e
cargo check --manifest-path src-tauri/Cargo.toml
```
