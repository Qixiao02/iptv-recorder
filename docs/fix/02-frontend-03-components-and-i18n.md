# 前端修复 03：组件化与国际化

> 优先级：**P1 + P2**
> 预计工时：7-10 天（部分可以分多 sprint）
> 推荐执行人：`frontend-engineer`
> 配套审计章节：`deep-analysis-2026-06-02.md` §3.2 I8, I9, §3.3 S5, S6, S7, S8, §5 P1-9, P1-12, P1-13, P2-1, P2-2, P2-3

## 范围与背景

本文件覆盖前端的**可维护性债务**。共 5 个子任务：i18n 硬编码 200+ 处替换、公共 Modal 组件、Settings 922 行拆分、虚拟列表、Layout selector 拆分。**P1 全部做完**后才考虑 P2。

## 子任务清单

### 子任务 6.1：i18n 硬编码批量替换（**P1**）

**审计引用**：§3.3 S5（详单见 `docs/fix/` 配套的 `frontend-i18n-extract.md` 或前端扫描报告 §5.2）、§3.2 I8

**问题**：前端 200+ 处硬编码中文，英文用户**完全无法使用**产品。

**执行方法（**不能一行行手改**）**：

#### 6.1a：自动提取硬编码
写个简单脚本扫所有 `.tsx` / `.ts` 找含中文的字符串字面量：
```ts
// scripts/extract-chinese.ts
import { readFileSync, readdirSync, statSync } from 'fs';
import { join } from 'path';

const CHINESE_REGEX = /[\u4e00-\u9fff]+/g;

function walk(dir: string): string[] {
  const results: string[] = [];
  for (const entry of readdirSync(dir)) {
    const fullPath = join(dir, entry);
    const stat = statSync(fullPath);
    if (stat.isDirectory()) {
      results.push(...walk(fullPath));
    } else if (/\.(tsx?|jsx?)$/.test(entry)) {
      const content = readFileSync(fullPath, 'utf-8');
      const matches = content.match(CHINESE_REGEX);
      if (matches) {
        results.push(`${fullPath}: ${matches.length} 处中文`);
      }
    }
  }
  return results;
}

console.log(walk('src').join('\n'));
```

跑 `pnpm tsx scripts/extract-chinese.ts | sort -t: -k2 -n -r` 得到按数量排序的列表。

#### 6.1b：批量替换
- 在 `src/locales/zh-CN.ts` 和 `en-US.ts` 加键（按 `module.section.context` 命名）
- 改代码用 `t('module.section.context')`
- 每次改完跑 `pnpm tsc --noEmit` + 浏览器抽检 1-2 个 page

**建议顺序**（按"用户高频接触"排序）：
1. Layout (15+ 处) — 全局可见
2. Dashboard (20+) — 首页
3. Channels (30+) — 高频操作
4. Schedules (25+)
5. Tasks (25+)
6. Settings (50+)
7. 各 Modal (Schedule/Channel/TaskDetail/Player/Import...)

**验收**：
- [ ] 切到 en-US，5 个 page 全部英文（除 i18n 键尚未覆盖的极少数角落）
- [ ] 上述脚本跑出来 `src/` 下中文字面量 = 0
- [ ] `pnpm build` 通过

**风险**：中。**所有现有用户已习惯中文**——切换后要保证上下文自然。`en-US.ts` 的翻译**用 AI 翻译会失真**——建议核心 UI 找真人翻一遍。

---

### 子任务 6.2：抽公共 `<Modal>` 组件（**P1**）

**审计引用**：`frontend/src/components/{Channel,Schedule,ImportM3U,EpgImport,EpgPrograms,TaskDetail}Modal.tsx`、§3.3 S6

**问题**：6 个 modal 各自写 `if (!isOpen) return null` + `.modal-overlay` + `.modal-content`，每个 60+ 行重复。

**修复方案**：
```tsx
// frontend/src/components/Modal.tsx
import { ReactNode, useEffect } from 'react';

interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title?: string;
  size?: 'sm' | 'md' | 'lg' | 'xl';
  closeOnEscape?: boolean;
  closeOnOverlay?: boolean;
  children: ReactNode;
  footer?: ReactNode;
}

export function Modal({
  isOpen, onClose, title, size = 'md',
  closeOnEscape = true, closeOnOverlay = true,
  children, footer,
}: ModalProps) {
  useEffect(() => {
    if (!isOpen || !closeOnEscape) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [isOpen, closeOnEscape, onClose]);

  useEffect(() => {
    if (isOpen) {
      // 锁 body 滚动
      document.body.style.overflow = 'hidden';
      return () => { document.body.style.overflow = ''; };
    }
  }, [isOpen]);

  if (!isOpen) return null;

  return (
    <div
      className="modal-overlay"
      onClick={closeOnOverlay ? onClose : undefined}
      role="dialog"
      aria-modal="true"
    >
      <div
        className={`modal-content modal-${size}`}
        onClick={(e) => e.stopPropagation()}
      >
        {title && <div className="modal-header">{title}</div>}
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-footer">{footer}</div>}
      </div>
    </div>
  );
}
```

**统一改造 6 个 modal**：
- ChannelModal / ScheduleModal / TaskDetailModal / ImportM3UModal / EpgImportModal / EpgProgramsModal
- 删各自的 `if (!isOpen) return null` + ESC 处理 + 滚动锁
- 用 `<Modal>` 包

**验收**：
- [ ] 6 个 modal 都能正常打开/关闭
- [ ] ESC 键关闭
- [ ] 点击遮罩关闭
- [ ] body 滚动锁定生效
- [ ] 单元测试：Modal 组件 prop 行为

**风险**：低。

---

### 子任务 6.3：抽公共 `format` lib（**P1**）

**审计引用**：§3.3 S7

**问题**：`formatDuration` 在 `Tasks/index.tsx:109` 和 `Schedules/index.tsx:144` 各自实现（输出还不一致）；`formatFileSize` 三处各写。

**修复方案**：
```ts
// frontend/src/lib/format.ts
export function formatDuration(seconds: number): string {
  if (seconds < 0) return '00:00:00';
  if (seconds < 3600) {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let i = -1;
  let size = bytes;
  do {
    size /= 1024;
    i++;
  } while (size >= 1024 && i < units.length - 1);
  return `${size.toFixed(2)} ${units[i]}`;
}

export function formatDateTime(iso: string | Date, format: 'datetime' | 'date' | 'time' = 'datetime'): string {
  const d = typeof iso === 'string' ? new Date(iso) : iso;
  // 已有 dayjs 依赖，直接用
  return dayjs(d).format(
    format === 'datetime' ? 'YYYY-MM-DD HH:mm:ss' :
    format === 'date' ? 'YYYY-MM-DD' : 'HH:mm:ss'
  );
}

export function formatSpeed(bytesPerSec: number): string {
  return `${formatFileSize(bytesPerSec)}/s`;
}
```

**统一替换**：
- `src/pages/Tasks/index.tsx:109` `formatDuration` → import
- `src/pages/Schedules/index.tsx:144` `formatDuration` → import
- `src/pages/Dashboard/index.tsx:127` `formatFileSize` → import
- `src/pages/Tasks/index.tsx:118` `formatFileSize` → import
- `src/components/TaskDetailModal.tsx:69` `formatFileSize` → import
- 等等

**验收**：
- [ ] `pnpm tsc --noEmit` 通过
- [ ] 5 个 page 全部显示一致的时长/大小
- [ ] 单元测试：边界值（0, 1, 60, 3600, 86400, 1023, 1024）

**风险**：低。

---

### 子任务 6.4：Layout 拆组件 + selector 拆分（**P1**）

**审计引用**：`frontend/src/components/Layout/index.tsx`、§3.2 I9

**问题**：
- Layout 290 行，5 个 useState + 5 个 useEffect
- `useUIStore` 一次性解构 4 字段，`alerts` 变更触发 Layout 全重渲染

**修复方案**：

#### 6.4a：selector 拆分
```ts
// frontend/src/components/Layout/index.tsx
const sidebarCollapsed = useUIStore(s => s.sidebarCollapsed);
const setSidebarCollapsed = useUIStore(s => s.setSidebarCollapsed);
const alerts = useUIStore(s => s.alerts);
const markAllAlertsRead = useUIStore(s => s.markAllAlertsRead);
const dismissAlert = useUIStore(s => s.dismissAlert);
```

#### 6.4b：拆 `<AlertDropdown>`
```tsx
// frontend/src/components/Layout/AlertDropdown.tsx
export function AlertDropdown() {
  const alerts = useUIStore(s => s.alerts);
  const markAllAlertsRead = useUIStore(s => s.markAllAlertsRead);
  // 整个 ~80 行从 Layout 移过来
}
```

#### 6.4c：拆 `<UserMenu>` + `<LanguageSwitcher>` + `<ThemeToggle>`
类似的 4 个小组件。

**验收**：
- [ ] `Layout/index.tsx` 行数 < 100
- [ ] alerts 变更不再触发 sidebar 重渲染（用 React DevTools Profiler 验证）
- [ ] 所有交互行为不变

**风险**：低。

---

### 子任务 6.5：Settings 922 行按 section 拆（**P1**）

**审计引用**：`frontend/src/pages/Settings/index.tsx:51`、§3.3 S8

**问题**：单文件 922 行，7 个 section + 4 个 mutation + 2 个 useEffect + 表单 + 密码 + 审计日志全堆一起。

**修复方案**：
```
src/pages/Settings/
├── index.tsx                    # 容器组件
├── settings.css
├── sections/
│   ├── BasicSection.tsx         # 基础设置
│   ├── StorageSection.tsx       # 存储设置
│   ├── RecordingSection.tsx     # 录制设置
│   ├── NotificationSection.tsx  # 通知设置
│   ├── AccountSection.tsx       # 账号（含密码表单）
│   ├── OperationsSection.tsx    # 运维（仅 admin）
│   └── AboutSection.tsx         # 关于
└── hooks/
    └── useConfigForm.ts         # 配置表单 state 管理
```

每个 section 文件 100-200 行。`useConfigForm` 抽公共的 `localConfig` / `hasChanges` / `save` 逻辑。

**验收**：
- [ ] Settings 主页 < 100 行（容器 + 7 个 section 引用）
- [ ] 各 section 文件 100-200 行
- [ ] 行为完全不变

**风险**：低。需要仔细保持所有 useState 行为。

---

### 子任务 6.6：引入 `tanstack-virtual`（**P2**）

**审计引用**：`frontend/src/pages/Channels/index.tsx:432`、`Tasks/index.tsx:212`、§3.2 I10

**问题**：Channels 100+ 条 / Tasks 几千条 → 表格/列表渲染卡。

**修复方案**：
```tsx
// frontend/src/pages/Channels/index.tsx
import { useVirtualizer } from '@tanstack/react-virtual';

// 在 Channels 表格里
const parentRef = useRef<HTMLDivElement>(null);
const virtualizer = useVirtualizer({
  count: filteredChannels.length,
  getScrollElement: () => parentRef.current,
  estimateSize: () => 60,
  overscan: 10,
});

return (
  <div ref={parentRef} style={{ height: '600px', overflow: 'auto' }}>
    <div style={{ height: `${virtualizer.getTotalSize()}px`, position: 'relative' }}>
      {virtualizer.getVirtualItems().map((virtualRow) => {
        const channel = filteredChannels[virtualRow.index];
        return (
          <div
            key={virtualRow.key}
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              right: 0,
              height: `${virtualRow.size}px`,
              transform: `translateY(${virtualRow.start}px)`,
            }}
          >
            <ChannelRow channel={channel} />
          </div>
        );
      })}
    </div>
  </div>
);
```

**验收**：
- [ ] 1 万条 channels 滚动 FPS > 30
- [ ] Tasks 同样改造
- [ ] React DevTools Profiler 验证：滚动时只渲染可见行

**风险**：低。但**需要先稳住 section 拆分**——Settings 拆分可能引发动。

---

## 测试要求

| 子任务 | 测试 |
| --- | --- |
| 6.1 | 手动 + i18n 切换 |
| 6.2 | Modal 单元测试 |
| 6.3 | format lib 单元测试（边界值）|
| 6.4 | Layout 渲染测试 + 性能验证 |
| 6.5 | 手动 |
| 6.6 | 性能测试（FPS / Profiler）|

## 提交策略

- **6.1 是大变更**：拆 4-5 个 PR（按文件分批）
- **6.2 / 6.3 短小**：可合并一个 PR
- **6.4 / 6.5 各自独立**
- **6.6 单独 PR**（涉及依赖添加）

## 风险汇总

| 子任务 | 风险 | 缓解 |
| --- | --- | --- |
| 6.1 | 翻译失真 | 找真人翻核心 UI |
| 6.2 | 行为差异 | 完整手动验收 |
| 6.3 | 输出格式变化用户不适应 | 选与现有 80% 场景一致的格式 |
| 6.4 | 拆组件 props 复杂 | 用 Context 或 store |
| 6.5 | 状态管理混乱 | 抽 `useConfigForm` hook |
| 6.6 | 性能回退 | Profiler 验证 |

---

*执行入口：6.3 → 6.2 → 6.4 → 6.5 → 6.1（分批）→ 6.6。*
