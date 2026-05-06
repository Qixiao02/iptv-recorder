# IPTV Recorder 前端设计文档

> 版本: 3.0
> 更新时间: 2025-02-19
> 架构: 前后端分离 | React + TypeScript
> 状态: ✅ 基础框架已完成

---

## 目录

1. [项目概览](#项目概览)
2. [技术栈](#技术栈)
3. [项目结构](#项目结构)
4. [已实现模块](#已实现模块)
5. [待开发模块](#待开发模块)
6. [运行指南](#运行指南)
7. [开发规范](#开发规范)

---

## 项目概览

### 架构图

```
┌─────────────────────────────────────────────────────────────┐
│                       浏览器                                  │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                    React 前端应用                         │ │
│  │  ┌───────────────────────────────────────────────────┐  │ │
│  │  │  路由: React Router v7                             │  │ │
│  │  │  状态: Zustand + TanStack Query                    │  │ │
│  │  │  UI: Ant Design 6.x + Tailwind CSS 4.x             │  │ │
│  │  │  国际化: react-i18next                             │  │ │
│  │  └───────────────────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
│                          │                                    │
│                    HTTP / WebSocket                          │
│                          ▼                                    │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                   Rust 后端 (Axum)                       │ │
│  │                   SQLite Database                        │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### 开发环境

| 服务 | 地址 | 说明 |
|------|------|------|
| 前端开发服务器 | http://localhost:5173 | Vite HMR |
| 后端 API | http://localhost:3000 | Rust Axum |
| API 代理 | /api | Vite 代理到后端 |
| WebSocket | /ws | Vite 代理到后端 |

---

## 技术栈

### 核心依赖

```json
{
  "dependencies": {
    "react": "^19.2.4",
    "react-dom": "^19.2.4",
    "react-router-dom": "^7.13.0",
    "antd": "^6.3.0",
    "@ant-design/icons": "^6.1.0",
    "@ant-design/charts": "^2.6.7",
    "zustand": "^5.0.11",
    "@tanstack/react-query": "^5.90.21",
    "axios": "^1.13.5",
    "dayjs": "^1.11.19",
    "i18next": "^25.8.11",
    "react-i18next": "^16.5.4"
  }
}
```

### 技术选型详情

| 类别 | 选型 | 版本 | 用途 |
|------|------|------|------|
| 运行时 | React | 19.2.4 | UI 框架 |
| 语言 | TypeScript | 5.9.x | 类型系统 |
| 构建 | Vite | 7.3.x | 开发服务器/打包 |
| 路由 | React Router | 7.13.x | 页面路由 |
| UI 组件 | Ant Design | 6.3.x | 组件库 |
| 样式 | Tailwind CSS | 4.2.x | 原子化 CSS |
| 状态管理 | Zustand | 5.0.x | 全局状态 |
| 服务端状态 | TanStack Query | 5.90.x | API 缓存 |
| 国际化 | react-i18next | 16.5.x | 多语言 |
| HTTP | axios | 1.13.x | API 请求 |
| 图表 | @ant-design/charts | 2.6.x | 数据可视化 |

---

## 项目结构

### 完整目录树

```
frontend/
├── src/
│   ├── api/                      # ✅ API 封装层
│   │   ├── client.ts             # Axios 实例配置
│   │   ├── channels.ts           # 频道 API
│   │   ├── schedules.ts          # 计划 API
│   │   ├── tasks.ts              # 任务 API
│   │   ├── system.ts             # 系统 API
│   │   └── websocket.ts          # WebSocket 客户端
│   │
│   ├── components/               # ✅ 通用组件
│   │   └── Layout/
│   │       ├── index.tsx         # 布局组件
│   │       └── Layout.css        # 布局样式
│   │
│   ├── hooks/                    # 📝 自定义 Hooks (待开发)
│   │   ├── useWebSocket.ts
│   │   ├── useTaskProgress.ts
│   │   └── useConfig.ts
│   │
│   ├── locales/                  # ✅ 国际化
│   │   ├── zh-CN.ts              # 简体中文
│   │   ├── en-US.ts              # 英文
│   │   └── index.ts              # i18next 配置
│   │
│   ├── pages/                    # ✅ 页面组件
│   │   ├── Dashboard/            # 仪表盘 (基础实现)
│   │   ├── Channels/             # 频道管理 (占位)
│   │   ├── Schedules/            # 录制计划 (占位)
│   │   ├── Tasks/                # 录制任务 (占位)
│   │   └── Settings/             # 系统设置 (占位)
│   │
│   ├── stores/                   # ✅ Zustand 状态
│   │   ├── channelStore.ts       # 频道状态
│   │   ├── authStore.ts          # 登录态
│   │   ├── settingStore.ts       # 设置状态
│   │   └── uiStore.ts            # UI 状态
│   │
│   ├── types/                    # ✅ TypeScript 类型
│   │   └── index.ts              # 所有类型定义
│   │
│   ├── utils/                    # 📝 工具函数 (待开发)
│   │   ├── format.ts
│   │   ├── validation.ts
│   │   └── cron.ts
│   │
│   ├── App.tsx                   # ✅ 根组件
│   ├── main.tsx                  # ✅ 入口文件
│   ├── index.css                 # ✅ 全局样式
│   └── vite-env.d.ts             # ✅ Vite 类型
│
├── public/                       # 静态资源
├── index.html                    # HTML 入口
├── package.json                  # 依赖配置
├── pnpm-lock.yaml                # 锁文件
├── vite.config.ts                # Vite 配置
├── tailwind.config.js            # Tailwind 配置
├── postcss.config.js             # PostCSS 配置
└── tsconfig.json                 # TypeScript 配置
```

---

## 已实现模块

### 1. API 封装层 (✅ 完成)

| 文件 | 功能 |
|------|------|
| `client.ts` | Axios 实例、拦截器、错误处理 |
| `channels.ts` | 频道 CRUD、导入 M3U、获取分组 |
| `schedules.ts` | 计划 CRUD、切换启用状态 |
| `tasks.ts` | 任务查询、取消、手动录制 |
| `system.ts` | 系统配置、调度器操作 |
| `websocket.ts` | WebSocket 客户端、事件订阅 |

### 2. 类型定义 (✅ 完成)

```typescript
// types/index.ts 包含所有类型:
- Channel, CreateChannelRequest
- Schedule, CreateScheduleRequest
- Task, ManualRecordRequest, TaskStatus
- UpcomingTask
- ImportM3URequest, ImportM3UResponse
- ErrorResponse, SystemConfig
- WebSocket 消息类型
```

### 3. 状态管理 (✅ 完成)

| Store | 状态 | Actions |
|-------|------|---------|
| `channelStore` | channels, selectedChannelIds, loading | setChannels, addChannel, updateChannel, removeChannel |
| `authStore` | token, user, isAuthenticated | setAuth, logout, updateUser |
| `settingStore` | config, language, loading | setConfig, setLanguage |
| `uiStore` | sidebarCollapsed, currentPath, alerts | setSidebarCollapsed, setCurrentPath, addAlert |

### 4. 国际化 (✅ 完成)

```typescript
// 支持语言
- zh-CN: 简体中文 (默认)
- en-US: English

// 翻译模块
- common: 通用词汇
- menu: 菜单项
- channel: 频道相关
- schedule: 计划相关
- task: 任务相关
- dashboard: 仪表盘
- settings: 系统设置
- websocket: WebSocket
- alerts: 告警信息
```

### 5. 布局组件 (✅ 完成)

```
┌─────────────────────────────────────────────┐
│  Header (顶部栏)                             │
│  - 菜单折叠按钮                              │
│  - 刷新按钮                                  │
│  - 语言切换                                  │
│  - 用户菜单                                  │
├─────────────────────────────────────────────┤
│  Sider (侧边栏)                              │
│  - Logo + 品牌名                             │
│  - 导航菜单 (暗色主题)                       │
│  - 可折叠                                   │
└─────────────────────────────────────────────┘
```

### 6. 仪表盘页面 (🔄 基础实现)

```typescript
// 已实现功能
- 统计卡片: 频道总数、计划总数、今日任务、失败任务
- 正在录制任务列表
- 即将执行的任务列表
- 最近完成的任务列表
- TanStack Query 数据获取
- 自动刷新 (5s/10s)
```

---

## 待开发模块

### 优先级 1 (高)

| 模块 | 功能 | 预估时间 |
|------|------|----------|
| **频道管理页面** | 列表、搜索、新建、编辑、删除、导入 M3U | 4h |
| **录制任务页面** | 列表、状态筛选、手动录制、任务详情 | 3h |
| **录制计划页面** | 列表、Cron 编辑器、启用/禁用 | 3h |

### 优先级 2 (中)

| 模块 | 功能 | 预估时间 |
|------|------|----------|
| **系统设置页面** | 配置面板、表单验证、保存 | 3h |
| **WebSocket 集成** | 实时进度推送、连接状态 | 2h |
| **通用组件** | StatusTag、ConfirmModal、M3UImportModal | 2h |

### 优先级 3 (低)

| 模块 | 功能 | 预估时间 |
|------|------|----------|
| **工具函数** | format.ts、validation.ts、cron.ts | 1h |
| **自定义 Hooks** | useWebSocket、useTaskProgress | 1h |
| **图表组件** | 录制统计、存储空间 | 2h |

---

## 运行指南

### 安装依赖

```bash
cd frontend
pnpm install
```

### 开发模式

```bash
# 启动前端 (http://localhost:5173)
pnpm dev

# 启动后端 (http://localhost:3000)
cd ../backend
cargo run
```

### 生产构建

```bash
# 构建
pnpm build

# 预览构建结果
pnpm preview
```

### 环境变量

```bash
# .env.development
VITE_API_BASE_URL=http://localhost:3000/api

# .env.production
VITE_API_BASE_URL=/api
```

---

## 开发规范

### 命名规范

| 类型 | 规范 | 示例 |
|------|------|------|
| 组件文件 | PascalCase | `ChannelList.tsx` |
| 工具文件 | camelCase | `format.ts` |
| 类型文件 | camelCase | `channel.ts` |
| 接口/类型 | PascalCase | `Channel`, `TaskStatus` |
| 常量 | UPPER_SNAKE_CASE | `API_BASE_URL` |
| React 组件 | PascalCase | `const Dashboard: React.FC` |
| 函数/变量 | camelCase | `getChannels`, `isLoading` |

### 文件组织

```typescript
// 组件文件结构
import { ... } from 'react';           // 1. React 导入
import { ... } from 'antd';              // 2. 第三方库
import { ... } from '@/components/...';  // 3. 内部组件
import { ... } from './xxx';             // 4. 相对导入
import type { ... } from '@/types';      // 5. 类型导入
import './xxx.css';                      // 6. 样式

interface Props { ... }                  // 7. 类型定义

export const Component: React.FC<Props> = ({ ... }) => {
  // Hooks
  // States
  // Effects
  // Handlers
  // Render
};

export default Component;
```

### 样式规范

```tsx
// 优先级: Ant Design 组件 > Tailwind > 内联样式
<Button className="flex items-center gap-2" type="primary">
  {t('common.confirm')}
</Button>
```

### API 调用规范

```tsx
// 使用 TanStack Query
const { data, isLoading, error, refetch } = useQuery({
  queryKey: ['channels'],
  queryFn: getChannels,
  refetchInterval: 5000,
});

// 使用 Mutation
const mutation = useMutation({
  mutationFn: createChannel,
  onSuccess: () => {
    message.success(t('common.success'));
    queryClient.invalidateQueries({ queryKey: ['channels'] });
  },
});
```

---

## API 映射

| 前端调用 | 方法 | 后端路由 |
|----------|------|----------|
| `getChannels()` | GET | `/api/channels` |
| `createChannel(data)` | POST | `/api/channels` |
| `updateChannel(id, data)` | PUT | `/api/channels/{id}` |
| `deleteChannel(id)` | DELETE | `/api/channels/{id}` |
| `getChannelGroups()` | GET | `/api/channels/groups` |
| `importM3UFromUrl(data)` | POST | `/api/channels/import/url` |
| `importM3UFromContent(data)` | POST | `/api/channels/import/content` |
| `getSchedules()` | GET | `/api/schedules` |
| `createSchedule(data)` | POST | `/api/schedules` |
| `toggleSchedule(id)` | POST | `/api/schedules/{id}/toggle` |
| `getTasks()` | GET | `/api/tasks` |
| `startManualRecord(data)` | POST | `/api/tasks/manual` |
| `cancelTask(id)` | DELETE | `/api/tasks/{id}` |
| `getUpcoming()` | GET | `/api/scheduler/upcoming` |
| `reloadScheduler()` | POST | `/api/scheduler/reload` |
| `getConfig()` | GET | `/api/config` |
| `updateConfig(data)` | POST | `/api/config` |
| `wsClient.connect()` | WS | `/ws` |

---

## 路由配置

```typescript
// 当前路由
/                    → 重定向到 /dashboard
/dashboard           → 仪表盘 (✅ 基础实现)
/channels            → 频道管理 (📝 占位)
/schedules           → 录制计划 (📝 占位)
/tasks               → 录制任务 (📝 占位)
/settings            → 系统设置 (📝 占位)
```

---

## 开发任务清单

### Phase 1: 基础页面 (进行中)

- [ ] 频道管理页面
  - [ ] 频道列表组件
  - [ ] 频道表单组件
  - [ ] M3U 导入弹窗
- [ ] 录制计划页面
  - [ ] 计划列表组件
  - [ ] Cron 编辑器组件
  - [ ] 计划表单组件
- [ ] 录制任务页面
  - [ ] 任务列表组件
  - [ ] 任务详情抽屉
  - [ ] 手动录制弹窗

### Phase 2: 功能增强

- [ ] WebSocket 实时更新
- [ ] 通知系统集成
- [ ] 系统设置页面
- [ ] 数据统计图表

### Phase 3: 优化完善

- [ ] 响应式布局优化
- [ ] 加载状态优化
- [ ] 错误处理完善
- [ ] 单元测试

---

*文档版本: 3.0*
*最后更新: 2025-02-19*
