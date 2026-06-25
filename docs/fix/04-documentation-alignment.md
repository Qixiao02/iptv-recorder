# 文档对齐修复

> 优先级：**P1**
> 预计工时：0.5 天
> 推荐执行人：`tech-lead` 拍板 + 任意 dev 执行
> 配套审计章节：`deep-analysis-2026-06-02.md` §2.4, §3.3 S9, §5 P1-8

## 范围与背景

3 份文档都说用 Ant Design 6.x，但代码完全不用——前端是 **Tailwind 4 + 自定义 CSS**。下一个维护者按文档走会浪费 30 分钟发现"文档错了"。

## 决策（tech-lead 拍板）

**两条路**：

### 选项 A：文档向代码对齐（推荐）
- 删掉 3 份文档和 CLAUDE.md 里所有 Ant Design 描述
- 把"自研 Tailwind 4 + 设计 token"写实
- 工作量小（纯文本修改）
- 风险：未来若想引 antd，要再改一次

### 选项 B：代码向文档对齐
- 引入 antd + @ant-design/icons + 替换所有自定义组件
- 工作量巨大（2-3 周）
- 风险：组件库与现有 `index.css` 设计 token 冲突

**推荐选 A**。理由：
- 项目已经成型，UI 风格有自己的设计语言（design token 在 `index.css:1-690`）
- antd 是企业级通用 UI 库，本项目的"卡片/进度条/徽章"已有自研且一致
- 切到 antd 收益有限

## 子任务清单

### 子任务 8.1：删 Ant Design 描述（**P1**）

**审计引用**：
- `docs/frontend-design.md:34-37, 71, 92`
- `docs/frontend-prompt.md:18`
- `docs/ui-design-prompt.md` 全文（已经不提 antd，但还要再确认无残留）
- `CLAUDE.md:143`

**修复方案**：

#### `CLAUDE.md:143`
```diff
- **Frontend**: React 19, TypeScript, Vite, Ant Design, TanStack Query, Zustand, React Router, Axios, i18next
+ **Frontend**: React 19, TypeScript, Vite, Tailwind CSS 4 + 自定义 design tokens (index.css), TanStack Query, Zustand, React Router, Axios, i18next
```

#### `docs/frontend-design.md:34-37`
```diff
- ## 技术栈选型
- - **UI**: Ant Design 6.x + Tailwind CSS 4.x
+ ## 技术栈选型
+ - **UI**: 自研设计系统 (基于 Tailwind 4 + CSS 变量) + Lucide Icons
+ - 详见 `docs/ui-design-prompt.md` 的设计语言章节
```

#### `docs/frontend-design.md:71, 92`
全文搜 "Ant Design" 替换为"自研 UI"或对应真实技术。

#### `docs/frontend-prompt.md:18`
```diff
- UI 组件库: Ant Design 6.x
+ UI 组件库: 自研组件（基于 Tailwind 4 + CSS 变量），图标用 lucide-react
```

#### `docs/ui-design-prompt.md`
确认无 "Ant Design" 残留。如果有，全文替换。

**验收**：
- [ ] `grep -ri "ant design" docs/ CLAUDE.md` 返回 0 行（`antd` 也算）
- [ ] README/CLAUDE.md 描述与实际 `package.json` 一致
- [ ] tech-lead 在 PR 描述里写"决策：选项 A，理由 X"

**风险**：极低。纯文本修改。

---

### 子任务 8.2：补一个"前端技术栈实际清单"段（**P1**）

**问题**：删完 antd 描述后，新人想知道"那到底用什么"，散落在 4 份文档里。

**修复方案**：在 `docs/frontend-design.md` 顶部加：
```markdown
## 前端实际技术栈（2026-06 更新）

| 类别 | 选型 | 备注 |
| --- | --- | --- |
| 框架 | React 19 | concurrent features 启用 |
| 语言 | TypeScript 5.9 | `strict: true` + 多 no* 选项 |
| 构建 | Vite 7 | `tsc -b && vite build` |
| 样式 | Tailwind 4 + 自定义 CSS | 设计 token 在 `src/index.css` |
| 图标 | lucide-react | 按需 tree-shake |
| 路由 | React Router 7 | 全部 lazy import |
| 状态 | Zustand 5 | 仅 4 个全局 store（auth/theme/ui/setting）|
| 服务端状态 | TanStack Query 5 | 频道/任务/计划/EPG 都走 Query |
| 国际化 | i18next + react-i18next | zh-CN / en-US |
| HTTP | axios | 拦截器统一处理 401/错误归一化 |
| 视频 | hls.js (light 子路径) | 仅 PlayerModal 用 |
| 时间 | dayjs | 替代 moment |
| 测试 | vitest + @testing-library/react | 覆盖率 < 5%，待补 |

> **历史说明**：本项目曾计划使用 Ant Design 6.x，但实际未引入。  
> 原因：项目已有自研 design system，与 antd 风格不兼容。
```

**验收**：
- [ ] `docs/frontend-design.md` 顶部有此表
- [ ] 与 `frontend/package.json` 字段一致

**风险**：低。

---

## 提交策略

- 一个 commit：
  ```
  docs: align Ant Design 文档/实现漂移 + 加实际技术栈表
  ```
- PR 描述：
  > 决策：保留自研设计系统，删除 Ant Design 文档描述。
  > 理由：项目已有成熟的 design token（index.css:1-690），切到 antd 收益小、风险大。

## 风险汇总

| 子任务 | 风险 | 缓解 |
| --- | --- | --- |
| 8.1 | 漏改某处 | grep 全 project |
| 8.2 | 表格与代码漂移 | 加 lint 检查（`scripts/check-deps.sh` 比对 package.json） |

---

*执行入口：8.1 → 8.2。半天搞定。*
