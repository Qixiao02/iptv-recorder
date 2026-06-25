# 修复计划总览

> 编制时间：2026-06-02
> 配套审计：`docs/deep-analysis-2026-06-02.md`
> 路径：`docs/fix/`

## 文件清单

| # | 文件 | 范围 | 优先级 | 预计工时 | 推荐执行人 |
| --- | --- | --- | --- | --- | --- |
| 00 | `00-overview.md`（本文件） | — | — | — | — |
| 01 | `01-backend-01-concurrency-and-reliability.md` | SQLite WAL / kill_on_drop / 原子 INSERT / event_sender / 超时 / 优雅停机 | **P0+P1** | 3-5 天 | `rust-backend-engineer` |
| 02 | `01-backend-02-security-hardening.md` | M3U SSRF / 默认密码 / CORS / 登录限流 / refresh token | **P0+P1+P2** | 3-4 天 | `rust-backend-engineer` |
| 03 | `01-backend-03-quality-and-cleanup.md` | sysinfo / 转码可配置 / Channel 死字段 / 录制备恢复 / observability | P1+P2 | 5-7 天 | `rust-backend-engineer` |
| 04 | `02-frontend-01-quick-wins.md` | Settings 比较 / Channels 并行测试 / 死代码删除 / `as any` | **P0** | 1-2 天 | `frontend-engineer` |
| 05 | `02-frontend-02-websocket-and-state.md` | WS 退避 / token 改 subprotocol / URL 解析 bug / useWebSocketBridge hook | **P0+P1** | 2-3 天 | `frontend-engineer` |
| 06 | `02-frontend-03-components-and-i18n.md` | 公共 Modal / 公共 format lib / Layout selector / Settings 拆分 / 虚拟列表 / i18n 200+ 处替换 | P1+P2 | 7-10 天 | `frontend-engineer` |
| 07 | `03-testing-and-ci.md` | GitHub Actions CI / 3 个集成测试 / 组件测试 / E2E（Playwright） | **P0+P1+P2** | 4-6 天 | `qa-engineer` 牵头 |
| 08 | `04-documentation-alignment.md` | Ant Design 文档/实现漂移 | P1 | 0.5 天 | `tech-lead` 拍板 + 任意 dev |

## 执行顺序建议（4 阶段）

### 阶段 0：先扫雷（1-2 天，4 人并行）

| 谁 | 做什么 | 文件 |
| --- | --- | --- |
| `rust-backend-engineer` | 修 SQLite WAL + kill_on_drop + 默认密码 + M3U SSRF | `01-01`, `01-02` |
| `frontend-engineer` | 修 WS URL bug + 一键测试并行 + Settings 比较 + 删死代码 | `02-01`, `02-02` |
| `qa-engineer` | 起 GitHub Actions CI 骨架（先跑通 lint+build，不管 test 覆盖） | `03` |
| `tech-lead` | 拍板 Ant Design 文档/实现方向（删文档 or 引入 antd），同步 README/CLAUDE.md | `04` |

**P0 不做完不进入 P1**。这一阶段做完后系统能上"灰度"但还缺人保。

### 阶段 1：质量底盘（5-7 天，3-4 人并行）

| 谁 | 做什么 | 文件 |
| --- | --- | --- |
| `rust-backend-engineer` | 原子 INSERT、event_sender 注入、超时 enforce、CORS 收紧、sysinfo、登录限流 | `01-01`, `01-02`, `01-03` |
| `frontend-engineer` | WS 退避 + token subprotocol + useWebSocketBridge + 公共 Modal + 公共 format | `02-02`, `02-03` |
| `qa-engineer` | 补 3 个核心集成测试（scheduler / recording 终态 / process 进程清理） | `03` |
| `frontend-engineer`（i18n 子任务）| 替换 200+ 处硬编码中文（按 P1-12 表格） | `02-03` |

### 阶段 2：可维护性（5-7 天，2-3 人）

| 谁 | 做什么 | 文件 |
| --- | --- | --- |
| `rust-backend-engineer` | Channel 死字段清理、转码可配置、Channel health check 实现 | `01-03` |
| `frontend-engineer` | Settings 922 行按 section 拆、引入 `tanstack-virtual`、Layout selector 拆分 | `02-03` |
| `qa-engineer` | 组件测试 + 第一个 E2E 流程（登录→导入→建计划） | `03` |

### 阶段 3：长期投资（持续）

| 谁 | 做什么 | 文件 |
| --- | --- | --- |
| `rust-backend-engineer` | 录制备恢复、优雅停机、Prometheus metrics、refresh token、DEPRECATED 字段清理 | `01-01`, `01-02`, `01-03` |
| `frontend-engineer` | 补 EPG 源管理 / M3U 源管理 / 录制文件库独立页 | `02-03` |
| `qa-engineer` | 覆盖剩余 E2E（录制→取消、并发任务、超时断连） | `03` |

## 依赖关系（关键路径）

```
阶段 0 ──► 阶段 1 ──► 阶段 2 ──► 阶段 3
  │           │           │
  │           ├─► [CI 必须就绪] 才能跑集成测试
  │           └─► [WS bug 修了] 才能做 WS 退避的负载测试
  └─► [tech-lead 拍板 Ant Design 方向] 才能动 frontend-design.md
```

**最关键依赖**：
- `03-testing-and-ci.md` 的 P0-1（CI 骨架）是 P1 集成测试的前置——CI 没起来前集成测试白写
- `02-frontend-02-websocket-and-state.md` 的 P0-1（URL 解析）应在阶段 0 立刻修，不然 P1-2 退避无从验证

## 不要做的事

- **不要重写 `frontend/src/pages/Settings/index.tsx` 整页**——按 section 拆，别动其他功能
- **不要大改 cron 解析逻辑**——只补 7 种简写到后端，先用前端的"高级模式"绕过 4 种
- **不要引入 TypeORM / Diesel**——项目已用 sqlx，没必要换
- **不要从零搭 monorepo / Turborepo**——Rust 和 TS 分仓管理
- **不要把 `N_m3u8DL-RE` 改成 Rust 内置**——外部依赖管理比"重写"重要

## 验收标准（每阶段结束必须达成）

**阶段 0 结束**：
- [ ] `cargo build --release` + `cargo test` 绿
- [ ] `pnpm build` + `pnpm lint` 绿
- [ ] GitHub Actions 能跑（哪怕只跑 build）
- [ ] 系统能在 1 个并发录制下稳定运行 24h

**阶段 1 结束**：
- [ ] 5 个并发录制 24h 不出 SQLITE_BUSY
- [ ] kill 服务时所有 N_m3u8DL-RE 子进程被收走（用 pgrep 验证）
- [ ] M3U 导入本地路径被拒（返回 400）
- [ ] CI 跑 5+ 个集成测试全绿
- [ ] 英文用户能完整使用（i18n 100% 替换完）

**阶段 2 结束**：
- [ ] 20 个并发录制压测 1h 不出现 race
- [ ] Channels 页 1 万条数据滚动不卡（FPS > 30）
- [ ] 前端任意组件测试覆盖率 > 30%

**阶段 3 结束**：
- [ ] 服务崩溃后重启能续录未完成任务
- [ ] SIGTERM 后 < 5s 干净退出
- [ ] Prometheus metrics 暴露 /metrics 端点

## 跟踪方式

每个修复文件执行完后：
1. 提交到 git（**不要 squash**，每个 P0/P1 一个 commit 方便 review）
2. 更新 `docs/fix/README.md` 的 checklist（加个 md 跟踪状态）
3. 在 PR 描述里引用对应的 fix 文件名（如 `Fixes docs/fix/01-backend-01-concurrency-and-reliability.md P0-1`）

---

*下一步：告诉我"做阶段 0"或"做 01-backend-01 P0-1"，我直接派单。*
