---
name: git-workflow
description: IPTV Recorder 项目的 git 分支、提交与推送/发版规范。当用户要在本仓库做任何 git 操作——提交代码、推送分支、发版本、打 tag、创建/切换分支、合并代码，或问"怎么提交/怎么发版/怎么推送"时，必须先加载本 skill 并严格按其流程执行。涵盖 dev/main 双分支模型、conventional commits 提交规范、功能级提交粒度、发版合并打 tag 流程。
---

# IPTV Recorder Git 工作流规范

本仓库采用 **dev / main 双分支模型**。所有 git 操作必须遵循本规范，**严格模式**：每个关卡有硬性检查，不满足则停止并向用户报告问题、等待修正，绝不绕过。

## 分支模型（铁律）

| 分支 | 角色 | 谁在上面写代码 |
|---|---|---|
| `dev` | **开发主干**，所有日常开发在此进行 | ✅ 是 |
| `main` | **发版主干**，仅发版时从 dev 合并过来，打 tag | ❌ 否，绝不直接在 main 上提交 |

**核心原则**：
- 所有新代码先落在 `dev`。**绝不在 `main` 上直接 commit/开发。**
- `main` 只接受来自 `dev` 的发版合并（`--no-ff`，保留合并节点）+ 发版 tag。
- 发版 = 把 `dev` 合并到 `main` 并打 tag，不是别的。

## 提交规范（conventional commits）

每个提交信息必须符合格式：

```
<type>(<scope>): <简述>

<可选正文，解释为什么/做了什么>
```

**type 白名单**（只能用这些，小写）：
- `feat` — 新功能
- `fix` — bug 修复
- `docs` — 文档（README、CHANGELOG 文案）
- `style` — 格式（不影响代码逻辑）
- `refactor` — 重构（非 feat/fix）
- `perf` — 性能优化
- `test` — 测试相关
- `chore` — 构建/工具/依赖
- `release` — 发版相关（版本号、CHANGELOG、发版合并）

**scope 建议**（对应本仓库模块，可按实际调整）：`channels`、`schedule`、`recording`、`player`、`transcode`、`security`、`settings`、`channels`、`config`、`i18n`、`migration` 等。

**简述要求**：
- 中文或英文均可，但**一句话说清这次改了什么**，不要空泛（❌"修改了一些文件" ✅"频道管理新增来源筛选"）。
- 不超过 50 字；正文换行后再写。
- 结尾**不加句号**。

**示例**：
```
feat(channels): 频道管理新增「来源」筛选(公网源/私有源)

后端 PaginationParams 增加 source_visibility 字段，list_paginated 支持按来源过滤；
前端工具栏新增筛选下拉。补充回归测试。
```

## 提交粒度：功能级

**一个功能/修复 = 一个提交**，不要把不相关的改动塞进同一个 commit，也不要把一个功能拆成碎片提交。

判定方法：
- 能用一句话（一个 type+scope+简述）概括的改动，归为一个提交。
- 一次会话里做了多个独立功能（如"加筛选"+"加画中画"），必须拆成**多个提交**，分别提交。
- 跨功能的共用文件（如 i18n、类型定义），归到与该改动**最相关**的那个功能提交里。

**如何拆分**（工作区有多个功能改动时）：
1. 先 `git status` 看全部改动。
2. 按功能分组，确定每个提交包含哪些文件。
3. 用 `git add <文件>` 逐组暂存 + `git commit`，**不要** `git add .` 一把梭。
4. 若某个文件同时含多个功能的改动且无法按文件拆分，用 `git add -p` 交互式分块暂存（非交互环境则向用户说明，按最相关功能归类）。

## 关卡检查（严格模式，逐步执行）

下面每个阶段的【必须满足】是硬性条件。**任一不满足，立即停止、向用户报告具体问题、等待指示，不要自动绕过或降级。**

### 阶段 1：开发前

**必须满足**：
- 当前在 `dev` 分支（`git branch --show-current` 显示 `dev`）。
  - 若不在 dev：切换到 dev（`git checkout dev`）。
  - 若本地没有 dev：从 main 创建（`git checkout main && git pull && git checkout -b dev`），并提示用户首次需 `git push -u origin dev`。
- dev 是最新的（`git pull --ff-only`；若提示需合并，停止并报告，不要自动 merge/rebase）。

### 阶段 2：提交前（质量门）

**必须满足**（任一失败则停止报告，不要提交）：
- 后端有改动 → `cargo check`（manifest-path 指向 backend/Cargo.toml）通过。
- 后端有改动 → `cargo test` 通过（至少不新增失败）。
- 前端有改动 → `pnpm build`（在 frontend 目录）通过。
- 前端有改动 → `pnpm lint`（在 frontend 目录）无错误。
- 工作区没有**与本任务无关**的遗留改动混入（如发现历史遗留未提交改动，先向用户确认归属，不要默认归并）。

> Rust 格式：改动的 .rs 文件用 `cargo fmt` 或 `rustfmt --edition 2021 --check <file>` 确认干净；不干净先格式化。

### 阶段 3：提交

按"功能级"拆分，逐个 `git add <文件>` + `git commit -m "<规范信息>"`。
- 提交后 `git status` 应为干净（除非有意保留部分改动）。
- 多个提交按逻辑顺序排列（底层/依赖项在前，上层功能在后）。

### 阶段 4：推送

**日常开发推送 dev**：
```
git push origin dev        # 或 git push（dev 已跟踪 origin/dev）
```
- 推送被拒（远程有新提交）→ 停止，提示用户 `git pull --rebase origin dev`，**不要自动 force push**。

**绝不**对 `main` 做日常推送。main 只在发版（阶段 5）时推送。

### 阶段 5：发版（仅当用户明确要求发版时执行）

发版 = dev → main 合并 + 打 tag。**逐步执行，每步确认。**

1. **确认 dev 干净且是最新**：
   - `git checkout dev && git status`（干净）
   - `git pull --ff-only`
2. **合并到 main**：
   - `git checkout main && git pull --ff-only`
   - `git merge --no-ff dev -m "release: vX.Y.Z 合并 dev 到 main\n\n<本次发版要点列表>"`
   - `--no-ff` 是必须的，保留合并节点，让发版历史清晰。
3. **更新版本号**（若尚未统一）：
   - `backend/Cargo.toml` 的 `version`
   - `frontend/package.json` 的 `version`
   - `frontend/src/i18n/modules/settings/{zh-CN,en-US}.ts` 的 `about.version`
   - 三处必须一致。
4. **打 tag**：
   - `git tag -a vX.Y.Z -m "IPTV Recorder vX.Y.Z\n\n<发版亮点>"`
   - tag 必须打在**合并后的 main** 上，不是 dev。
5. **推送 main + tag**：
   - `git push origin main`
   - `git push origin vX.Y.Z`
6. **GitHub Release**（可选）：提示用户在 GitHub Releases 基于 tag 发 release，把 CHANGELOG 对应段落贴进去。若装了 `gh`，可 `gh release create vX.Y.Z --notes-file ...`。
7. **切回 dev 继续开发**：`git checkout dev`

### 发版后的 CHANGELOG 维护

每次发版前，把本次所有改动整理进 `CHANGELOG.md` 顶部新增一个版本段（格式见现有 CHANGELOG）：
- 按 `### ✨ 新功能 / 🐛 修复 / 🔒 安全 / 🛠 重构 / 🧪 测试` 分类。
- 每条一句话，说清用户能感知的变化。
- CHANGELOG 是单一数据源，前端"更新日志"页通过 `prebuild` 脚本从根目录同步。

## 常见场景速查

| 场景 | 正确做法 |
|---|---|
| 日常写代码 | `dev` 分支 → 提交 → `git push origin dev` |
| 紧急修复 | 也在 `dev` 上做（除非用户明确要求热修到 main，需额外确认） |
| 发新版本 | dev 合并 main（--no-ff）+ 打 tag + push main & tag |
| 同步 main 的热修到 dev | `git checkout dev && git merge main` |
| 不确定当前该在哪 | 默认在 dev；main 仅发版时碰 |
| 想看发版历史 | `git log --oneline --graph main` 看合并节点 + `git tag -l` |

## 红线（绝不自动执行，需用户明确授权）

- `git push --force` / `--force-with-lease` 到任何分支。
- `git rebase` 已推送的提交。
- 删除分支（`git push origin --delete` / `git branch -D`）。
- 修改/删除已打的 tag。
- 直接在 main 上 commit（非发版合并）。
- `git add .` 一把梭提交（除非改动确实只属于单一功能且已确认）。

遇到这些操作，**先暂停，向用户说明影响并请求确认**。
