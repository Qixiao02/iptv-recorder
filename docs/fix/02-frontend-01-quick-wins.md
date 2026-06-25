# 前端修复 01：Quick Wins

> 优先级：**全部 P0**
> 预计工时：1-2 天
> 推荐执行人：`frontend-engineer`
> 配套审计章节：`deep-analysis-2026-06-02.md` §3.3 S1-S4, §3.2 I6, 附录 B

## 范围与背景

本文件覆盖前端**纯清理 + 1 个关键性能 bug**，全部是 P0 但工时很短。先做这一组能给后续修复腾出干净的 baseline。共 5 个子任务。

## 子任务清单

### 子任务 4.1：删除死代码（**P0**）

**审计引用**：
- `frontend/src/stores/channelStore.ts:1-41`（§3.3 S2）— 全项目零引用
- `frontend/src/App.css:1-37`（§3.3 S3）— Vite 模板未被引用
- `frontend/src/assets/react.svg`（附录 B）— React 默认 logo 零引用

**修复步骤**：
```bash
# 验证零引用后再删
cd D:\work\Porject\iptv-recorder\frontend
grep -r "channelStore" src/  # 应该只在自身文件
grep -r "App.css" src/
grep -r "react.svg" src/
# 确认零引用后
rm src/stores/channelStore.ts
rm src/App.css
rm src/assets/react.svg
```

**验收**：
- [ ] `pnpm build` 通过
- [ ] `git grep "channelStore"` 仅命中 `package-lock.json`（间接）
- [ ] `pnpm dev` 启动后页面正常

**风险**：极低。

---

### 子任务 4.2：Settings `hasChanges` 改白名单路径比较（**P0**）

**审计引用**：`frontend/src/pages/Settings/index.tsx:221`、§3.2 I11

**问题**：
```ts
const hasChanges = JSON.stringify(localConfig) !== JSON.stringify(config);
```
若后端返回的字段比本地 `defaultConfig` 多，会**永远判为有变更**（false positive）。

**修复方案**：白名单路径比较
```ts
// frontend/src/pages/Settings/index.tsx
const TRACKED_FIELDS: Array<keyof Config> = [
  'server', 'database', 'storage', 'recorder', 'scheduler',
  // ... 列出真正关心变更的字段
] as const;

const isEqualByTrackedFields = (a: Config, b: Config): boolean => {
  for (const key of TRACKED_FIELDS) {
    if (JSON.stringify(a[key]) !== JSON.stringify(b[key])) {
      return false;
    }
  }
  return true;
};

const hasChanges = !isEqualByTrackedFields(localConfig, config);
```

**进一步方案**：用 `lodash.isEqual` 配合 `pick(localConfig, TRACKED_FIELDS)` 选字段再比较。

**验收**：
- [ ] 后端多返回一个无关字段（如 `metadata.diagnostics`）→ `hasChanges` 是 false
- [ ] 改 `recorder.executable` → `hasChanges` 是 true
- [ ] 改 `server.port` → `hasChanges` 是 true
- [ ] 单元测试：3 个 case

**风险**：低。

---

### 子任务 4.3：Channels 一键测试改并行（**P0**）

**审计引用**：`frontend/src/pages/Channels/index.tsx:122-146`、§3.2 I7

**问题**：
```ts
const handleBatchTest = async () => {
  for (const id of selectedIds) {
    await testChannel(id);   // ← 串行
    await queryClient.invalidateQueries({ queryKey: ['channels'] });
  }
};
```
选 100 个频道点"一键测试"要等 100 个 HTTP 串行返回，UI 卡死。

**修复方案**：
```ts
// frontend/src/pages/Channels/index.tsx
const handleBatchTest = async () => {
  const results = await Promise.allSettled(
    selectedIds.map(id => testChannel(id))
  );
  
  const succeeded = results.filter(r => r.status === 'fulfilled').length;
  const failed = results.length - succeeded;
  
  // 一次性 invalidate
  await queryClient.invalidateQueries({ queryKey: ['channels'] });
  
  // 反馈
  toast.success(`测试完成：${succeeded} 个成功，${failed} 个失败`);
};
```

**进阶**：加并发上限（如 5 并发），避免 100 选时一次性 100 个请求打爆后端：
```ts
import pLimit from 'p-limit';
const limit = pLimit(5);
const results = await Promise.allSettled(
  selectedIds.map(id => limit(() => testChannel(id)))
);
```

**验收**：
- [ ] 选 10 个 channel → 全部返回时间 ≈ 单个请求时间（不是 10 倍）
- [ ] 后端故意让 1 个 channel URL 失效 → UI 显示"9 成功 1 失败"
- [ ] 单元测试：`Promise.allSettled` 用例

**风险**：低。但**建议加上并发上限**（p-limit）以保护后端。

---

### 子任务 4.4：移除 `ScheduleModal` 的 6 处 `as any`（**P0**）

**审计引用**：`frontend/src/components/ScheduleModal.tsx:93-98`、§3.3 S4

**问题**：
```ts
const videoQuality = (schedule as any).video_quality;
const audioQuality = (schedule as any).audio_quality;
const maxSpeed = (schedule as any).max_speed;
const threadCount = (schedule as any).thread_count;
const transcodeMode = (schedule as any).transcode_mode;
const transcodePreset = (schedule as any).transcode_preset;
```
`Schedule` 类型在 `frontend/src/types/index.ts:30-50` 已包含所有这些字段，`as any` 是**坏习惯传染**——让人误以为字段不存在。

**修复方案**：
```ts
// frontend/src/components/ScheduleModal.tsx
const videoQuality = schedule.video_quality;
const audioQuality = schedule.audio_quality;
const maxSpeed = schedule.max_speed;
const threadCount = schedule.thread_count;
const transcodeMode = schedule.transcode_mode;
const transcodePreset = schedule.transcode_preset;
```

如果遇到 `TS2548: Property 'x' is optional and may be undefined` 之类的真实类型错误，**不要**用 `as any` 绕过，改用可选链或默认值。

**验收**：
- [ ] `pnpm tsc --noEmit` 通过
- [ ] `grep "as any" frontend/src/components/ScheduleModal.tsx` 返回 0 行

**风险**：低。如果遇到真实类型错误需要单独修。

---

### 子任务 4.5：Layout 切换语言用 `i18n.changeLanguage()`（**P0**）

**审计引用**：`frontend/src/components/Layout/index.tsx:68-73`、§3.2 I13

**问题**：
```ts
const handleLanguageChange = (lang: string) => {
  localStorage.setItem('lang', lang);
  window.location.reload();  // ← 整页重载，状态全丢
};
```
切换语言整页重载——未保存的表单、滚动位置、WS 状态全丢。

**修复方案**：
```ts
// frontend/src/components/Layout/index.tsx
import { useTranslation } from 'react-i18next';
import { useSettingStore } from '@/stores/settingStore';

const handleLanguageChange = (lang: 'zh-CN' | 'en-US') => {
  // 直接调 i18next
  i18n.changeLanguage(lang);
  // 同步 setting store
  useSettingStore.getState().setLanguage(lang);
};
```

**注意**：`settingStore` 和 `i18n` 都在持久化语言——本任务保留两边但**用 i18n 单一驱动**：
- i18n 读 `localStorage('i18nextLng')`（默认）
- settingStore 仍写但仅作"业务状态"展示
- 简化方案：把 settingStore 删了，让 i18n 单一源

**验收**：
- [ ] 切换语言不刷新页面
- [ ] 表单输入中切换语言 → 表单内容保留
- [ ] WS 连接中切换语言 → WS 保持

**风险**：低。

---

## 测试要求

| 子任务 | 测试 |
| --- | --- |
| 4.1 | 手动：build 通过、grep 零引用 |
| 4.2 | 3 个单元测试：3 种场景 |
| 4.3 | 1 个集成测试：批量 + 故意失败 |
| 4.4 | `pnpm tsc --noEmit` |
| 4.5 | 手动：填表单切语言 |

## 提交策略

- 一个 PR 包含 5 个子任务（小变更，单独 PR 太多噪音）
- commit message 分段（`fix(frontend): dead code cleanup + Settings comparison`）

## 风险汇总

| 子任务 | 风险 | 缓解 |
| --- | --- | --- |
| 4.1 | 误删被引用的文件 | grep 确认零引用 |
| 4.2 | 白名单漏字段 | 列全后再做 |
| 4.3 | 100 并发打爆后端 | 加 p-limit 5 |
| 4.4 | 真实类型错误 | 单独处理 |
| 4.5 | i18n 与 settingStore 不同步 | 短期保留双写 |

---

*执行入口：4.1 → 4.3 → 4.2 → 4.4 → 4.5。*
