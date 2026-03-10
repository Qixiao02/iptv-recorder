# IPTV Recorder 前端生产提示词文档

> 版本: 1.0 | 更新日期: 2026-02-19

本文档用于指导 AI 或开发者快速生成高质量的前端代码。

---

## 一、项目概述

### 1.1 项目简介
IPTV Recorder 是一个 IPTV 节目录制管理系统，提供 M3U 播放源管理、定时录制、直播预览、录制文件管理等功能。

### 1.2 技术栈
```
前端框架: React 19.x + TypeScript 5.x
构建工具: Vite 7.x
UI 组件库: Ant Design 6.x
样式方案: Tailwind CSS 4.x
状态管理: Zustand 5.x
服务端状态: TanStack Query 5.x
路由管理: React Router 7.x
国际化: react-i18next 16.x
HTTP 客户端: axios 1.x
实时通信: WebSocket
后端 API: Rust + Axum
```

### 1.3 目录结构
```
frontend/
├── src/
│   ├── api/              # API 请求封装
│   │   ├── request.ts    # axios 实例配置
│   │   ├── channels.ts   # 频道相关 API
│   │   ├── tasks.ts      # 任务相关 API
│   │   ├── recordings.ts # 录制文件 API
│   │   ├── m3u.ts        # M3U 源管理 API
│   │   ├── settings.ts   # 系统设置 API
│   │   └── websocket.ts  # WebSocket 客户端
│   ├── components/       # 通用组件
│   │   ├── Layout/       # 布局组件
│   │   ├── VideoPlayer/  # 视频播放器
│   │   ├── TaskStatus/   # 任务状态组件
│   │   └── ...
│   ├── hooks/            # 自定义 Hooks
│   │   ├── useWebSocket.ts
│   │   ├── useTasks.ts
│   │   └── ...
│   ├── locales/          # 国际化资源
│   │   ├── zh-CN.json
│   │   └── en-US.json
│   ├── pages/            # 页面组件
│   │   ├── Dashboard/    # 仪表盘
│   │   ├── Channels/     # 频道管理
│   │   ├── Tasks/        # 任务管理
│   │   ├── Recordings/   # 录制管理
│   │   ├── M3USources/   # M3U 源管理
│   │   ├── Live/         # 直播预览
│   │   └── Settings/     # 系统设置
│   ├── stores/           # Zustand 状态仓库
│   │   ├── useAppStore.ts
│   │   ├── useTaskStore.ts
│   │   └── ...
│   ├── types/            # TypeScript 类型定义
│   │   ├── channel.ts
│   │   ├── task.ts
│   │   ├── recording.ts
│   │   └── ...
│   ├── utils/            # 工具函数
│   │   ├── format.ts
│   │   ├── time.ts
│   │   └── ...
│   ├── App.tsx           # 应用入口
│   ├── main.tsx          # 渲染入口
│   ├── router.tsx        # 路由配置
│   └── index.css         # 全局样式
├── package.json
├── vite.config.ts
├── tailwind.config.js
└── tsconfig.json
```

---

## 二、设计规范

### 2.1 配色方案
```css
/* 主色调 - 深蓝色系 */
--primary-color: #1890ff;        /* 主色 */
--primary-hover: #40a9ff;        /* 主色悬停 */
--primary-active: #096dd9;       /* 主色激活 */

/* 功能色 */
--success-color: #52c41a;        /* 成功 */
--warning-color: #faad14;        /* 警告 */
--error-color: #ff4d4f;          /* 错误 */
--info-color: #1890ff;           /* 信息 */

/* 中性色 */
--text-primary: rgba(0, 0, 0, 0.88);    /* 主要文字 */
--text-secondary: rgba(0, 0, 0, 0.65);  /* 次要文字 */
--text-disabled: rgba(0, 0, 0, 0.25);   /* 禁用文字 */
--border-color: #d9d9d9;                 /* 边框色 */
--background-color: #f5f5f5;             /* 背景色 */

/* 暗色模式 */
--dark-bg: #141414;
--dark-card-bg: #1f1f1f;
--dark-text: rgba(255, 255, 255, 0.85);
```

### 2.2 布局规范
```
页面最小宽度: 1200px
侧边栏宽度: 200px (折叠: 80px)
顶部导航高度: 64px
内容区域内边距: 24px
卡片圆角: 8px
表格行高: 54px
```

### 2.3 字体规范
```css
font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto,
  'Helvetica Neue', Arial, 'Noto Sans', sans-serif;

/* 字号 */
--font-size-xs: 12px;
--font-size-sm: 14px;
--font-size-base: 16px;
--font-size-lg: 18px;
--font-size-xl: 20px;
--font-size-2xl: 24px;
--font-size-3xl: 30px;
```

### 2.4 间距规范
```
xs: 4px
sm: 8px
md: 16px
lg: 24px
xl: 32px
2xl: 48px
```

---

## 三、组件设计指南

### 3.1 通用原则
1. **组件命名**: 使用 PascalCase，如 `ChannelList`, `TaskCard`
2. **Props 定义**: 必须使用 TypeScript 接口定义
3. **样式隔离**: 优先使用 Tailwind 类名，复杂样式使用 CSS Modules
4. **国际化**: 所有用户可见文本必须使用 `t()` 函数
5. **可访问性**: 添加适当的 aria 属性

### 3.2 状态管理规范

#### Zustand Store 模板
```typescript
// stores/useExampleStore.ts
import { create } from 'zustand';
import { devtools, persist } from 'zustand/middleware';

interface ExampleState {
  // 状态
  data: SomeType[];
  loading: boolean;

  // 操作
  setData: (data: SomeType[]) => void;
  fetchData: () => Promise<void>;
}

export const useExampleStore = create<ExampleState>()(
  devtools(
    persist(
      (set, get) => ({
        data: [],
        loading: false,

        setData: (data) => set({ data }),

        fetchData: async () => {
          set({ loading: true });
          try {
            const res = await api.getData();
            set({ data: res.data });
          } finally {
            set({ loading: false });
          }
        },
      }),
      { name: 'example-store' }
    )
  )
);
```

### 3.3 API 请求规范

#### Axios 实例配置
```typescript
// api/request.ts
import axios from 'axios';
import { message } from 'antd';

const request = axios.create({
  baseURL: '/api',
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
});

// 请求拦截器
request.interceptors.request.use(
  (config) => {
    // 可添加 token 等
    return config;
  },
  (error) => Promise.reject(error)
);

// 响应拦截器
request.interceptors.response.use(
  (response) => response.data,
  (error) => {
    const msg = error.response?.data?.message || '请求失败';
    message.error(msg);
    return Promise.reject(error);
  }
);

export default request;
```

#### API 模块模板
```typescript
// api/channels.ts
import request from './request';
import type { Channel, ChannelListParams, ChannelListResponse } from '@/types/channel';

export const channelApi = {
  // 获取频道列表
  getList: (params: ChannelListParams) =>
    request.get<ChannelListResponse>('/channels', { params }),

  // 获取单个频道
  getById: (id: number) =>
    request.get<Channel>(`/channels/${id}`),

  // 创建频道
  create: (data: Partial<Channel>) =>
    request.post<Channel>('/channels', data),

  // 更新频道
  update: (id: number, data: Partial<Channel>) =>
    request.put<Channel>(`/channels/${id}`, data),

  // 删除频道
  delete: (id: number) =>
    request.delete(`/channels/${id}`),
};
```

### 3.4 页面组件模板

```typescript
// pages/Example/index.tsx
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Card, Table, Button, Space } from 'antd';
import { PlusOutlined } from '@ant-design/icons';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { exampleApi } from '@/api/example';
import type { ExampleItem } from '@/types/example';
import type { ColumnsType } from 'antd/es/table';

const ExamplePage: React.FC = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  // 获取列表数据
  const { data, isLoading } = useQuery({
    queryKey: ['examples'],
    queryFn: () => exampleApi.getList(),
  });

  // 删除操作
  const deleteMutation = useMutation({
    mutationFn: (id: number) => exampleApi.delete(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['examples'] });
    },
  });

  // 表格列定义
  const columns: ColumnsType<ExampleItem> = [
    {
      title: t('example.name'),
      dataIndex: 'name',
      key: 'name',
    },
    {
      title: t('common.actions'),
      key: 'actions',
      render: (_, record) => (
        <Space>
          <Button type="link" onClick={() => handleEdit(record)}>
            {t('common.edit')}
          </Button>
          <Button type="link" danger onClick={() => deleteMutation.mutate(record.id)}>
            {t('common.delete')}
          </Button>
        </Space>
      ),
    },
  ];

  return (
    <div className="p-6">
      <Card
        title={t('example.title')}
        extra={
          <Button type="primary" icon={<PlusOutlined />}>
            {t('common.add')}
          </Button>
        }
      >
        <Table
          columns={columns}
          dataSource={data}
          loading={isLoading}
          rowKey="id"
        />
      </Card>
    </div>
  );
};

export default ExamplePage;
```

---

## 四、功能模块详细设计

### 4.1 仪表盘 (Dashboard)

**功能要点:**
- 统计卡片：总频道数、今日录制、运行任务、存储使用
- 运行中任务列表（实时更新）
- 即将开始的任务
- 最近录制的文件
- 系统状态指示器

**提示词:**
```
创建一个仪表盘页面，包含以下元素：

1. 顶部统计卡片区（4列网格布局）:
   - 总频道数：图标 + 数值 + 同比变化
   - 今日录制：已完成/总数，带进度条
   - 运行中任务：实时数量，带动画效果
   - 存储使用：已用/总量，带圆形进度

2. 中部内容区（两列布局）:
   - 左侧：运行中任务列表
     - 显示：频道名称、开始时间、已录制时长、文件大小
     - 状态：带颜色标识（录制中、暂停、错误）
     - 操作：停止按钮
   - 右侧：即将开始的任务
     - 显示：频道名称、计划时间、时长
     - 倒计时显示

3. 底部区域:
   - 最近录制文件表格
   - 列：文件名、频道、时长、大小、创建时间、操作

使用 WebSocket 实现运行中任务的实时更新。
支持暗色模式。
```

### 4.2 频道管理 (Channels)

**功能要点:**
- 频道列表（表格/卡片视图切换）
- 搜索、筛选（按分组、源）
- 新增/编辑/删除频道
- 批量操作
- 频道状态检测
- 快速创建录制任务

**提示词:**
```
创建频道管理页面，功能需求：

1. 顶部工具栏:
   - 搜索框（支持频道名称模糊搜索）
   - 分组筛选下拉
   - M3U 源筛选下拉
   - 视图切换按钮（表格/卡片）
   - 新增频道按钮
   - 批量操作按钮（启用/禁用/删除）

2. 表格视图列:
   - 缩略图（可选显示）
   - 频道名称 + 分组标签
   - M3U 来源
   - 流地址（部分隐藏）
   - 状态（在线/离线/未知）
   - 最后检测时间
   - 操作按钮

3. 卡片视图:
   - 频道封面图
   - 频道名称
   - 分组标签
   - 状态指示器
   - 快捷操作按钮

4. 新增/编辑弹窗:
   - 频道名称（必填）
   - 分组选择
   - 流地址（必填，URL 验证）
   - M3U 来源（可选）
   - Logo URL（可选）
   - 启用状态

5. 交互特性:
   - 表格行拖拽排序
   - 双击行打开编辑
   - 右键菜单
   - 行选择批量操作
```

### 4.3 任务管理 (Tasks)

**功能要点:**
- 任务列表（计划任务、历史任务 Tab）
- 创建定时录制任务
- Cron 表达式配置
- 任务状态管理
- 任务执行日志

**提示词:**
```
创建任务管理页面：

1. Tab 切换:
   - 计划任务（待执行的任务）
   - 历史任务（已完成的任务）

2. 计划任务列表:
   - 频道名称
   - 录制时间（Cron 表达式 + 人类可读描述）
   - 录制时长
   - 输出路径
   - 状态（启用/禁用）
   - 下次执行时间
   - 操作：立即执行、编辑、禁用/启用、删除

3. 创建任务表单:
   - 频道选择（下拉搜索）
   - 录制模式：单次 / 重复
   - 单次模式：日期时间选择器
   - 重复模式：
     - Cron 表达式输入
     - 可视化 Cron 编辑器（秒/分/时/日/月/周）
     - 预设模板（每天、每周、工作日等）
   - 录制时长
   - 输出文件名模板
   - 输出目录选择

4. 历史任务列表:
   - 任务名称
   - 执行时间
   - 执行结果（成功/失败）
   - 文件大小
   - 错误信息（如有）
   - 查看日志按钮

5. 任务详情抽屉:
   - 基本信息
   - 执行历史时间线
   - 完整日志输出
```

### 4.4 录制管理 (Recordings)

**功能要点:**
- 录制文件列表
- 文件预览/播放
- 下载/删除
- 批量管理
- 存储统计

**提示词:**
```
创建录制文件管理页面：

1. 顶部统计栏:
   - 总文件数
   - 总占用空间
   - 今日新增

2. 筛选工具栏:
   - 时间范围选择
   - 频道筛选
   - 文件大小范围
   - 搜索框（文件名）
   - 排序方式

3. 文件列表（表格 + 缩略图模式）:
   - 缩略图（视频第一帧）
   - 文件名
   - 频道来源
   - 时长
   - 文件大小
   - 创建时间
   - 操作：播放、下载、重命名、删除

4. 视频预览弹窗:
   - 视频播放器（支持 HLS/MP4）
   - 文件信息面板
   - 下载按钮

5. 批量操作:
   - 全选/反选
   - 批量下载（打包 ZIP）
   - 批量删除（二次确认）

6. 存储管理:
   - 存储空间进度条
   - 自动清理设置入口
```

### 4.5 M3U 源管理 (M3U Sources)

**功能要点:**
- M3U 源列表
- 添加/编辑/删除源
- 手动刷新/自动刷新
- 解析状态
- 频道预览

**提示词:**
```
创建 M3U 源管理页面：

1. 源列表卡片:
   - 源名称
   - 源类型（URL/文件）
   - 地址/路径
   - 更新间隔
   - 最后更新时间
   - 频道数量
   - 状态（正常/错误/更新中）
   - 操作：刷新、编辑、删除、查看频道

2. 添加源表单:
   - 源名称
   - 源类型选择（URL/本地文件）
   - URL 输入（URL 类型）
   - 文件上传（文件类型）
   - 自动更新间隔（下拉：禁用/1小时/6小时/12小时/24小时）
   - EPG 源 URL（可选）

3. 刷新操作:
   - 手动刷新按钮
   - 刷新进度显示
   - 刷新结果提示（新增/更新/删除数量）

4. 频道预览弹窗:
   - 从该源解析的所有频道
   - 分组筛选
   - 搜索
   - 频道状态检测

5. 定时刷新:
   - 显示下次刷新时间
   - 最后刷新结果
```

### 4.6 直播预览 (Live)

**功能要点:**
- 频道列表侧边栏
- 视频播放器
- 快速录制按钮
- 播放历史

**提示词:**
```
创建直播预览页面：

1. 左侧频道列表（可折叠）:
   - 搜索框
   - 分组树形结构
   - 频道项：Logo + 名称 + 状态指示
   - 当前播放高亮
   - 收藏频道置顶

2. 主播放区域:
   - 视频播放器（全屏支持）
   - 频道信息覆盖层（名称、分组）
   - 控制栏：播放/暂停、音量、全屏

3. 右侧操作面板:
   - 快速录制按钮
   - 录制时长选择（15分钟/30分钟/1小时/自定义）
   - 画质选择
   - 频道详细信息

4. 底部历史记录:
   - 最近观看的频道
   - 快速切换按钮

5. 播放器要求:
   - 支持 HLS (.m3u8) 和 HTTP FLV
   - 低延迟播放
   - 断线重连
   - 错误提示
```

### 4.7 系统设置 (Settings)

**功能要点:**
- 基础设置
- 存储设置
- 录制设置
- 通知设置
- 外观设置
- 关于

**提示词:**
```
创建系统设置页面（左侧菜单 + 右侧内容布局）：

1. 基础设置:
   - 语言切换（中文/英文）
   - 时区设置
   - 开机自启动
   - 系统日志级别

2. 存储设置:
   - 录制文件保存路径（文件夹选择）
   - 临时文件路径
   - 自动清理开关
   - 保留天数
   - 最小剩余空间
   - 存储空间显示

3. 录制设置:
   - 默认录制格式（MP4/TS/MKV）
   - 默认录制时长
   - 文件命名模板
   - 同时录制任务数上限
   - 断线重连次数
   - 分段录制大小

4. 通知设置:
   - 录制完成通知
   - 录制失败通知
   - 存储空间警告
   - 通知方式（系统/Webhook）

5. 外观设置:
   - 主题切换（亮色/暗色/跟随系统）
   - 主题色选择
   - 紧凑模式

6. 关于:
   - 版本信息
   - 检查更新
   - 开源协议
   - GitHub 链接

所有设置项需要有「保存」和「重置」按钮。
保存成功后显示提示。
```

---

## 五、通用组件设计

### 5.1 视频播放器组件

**提示词:**
```
创建 VideoPlayer 组件：

Props:
- src: 视频地址
- poster: 封面图
- autoPlay: 是否自动播放
- muted: 是否静音
- onError: 错误回调
- onEnded: 播放结束回调

功能要求:
1. 支持 HLS 和普通视频格式
2. 自定义控制栏（播放、进度、音量、全屏）
3. 加载状态指示
4. 错误状态显示
5. 全屏模式
6. 画中画支持
7. 快捷键支持（空格暂停、方向键快进）

样式要求:
1. 响应式宽高
2. 暗色控制栏
3. 平滑过渡动画
4. 悬浮显示控制栏
```

### 5.2 任务状态组件

**提示词:**
```
创建 TaskStatus 组件：

Props:
- status: 任务状态枚举 (pending/running/completed/failed/cancelled)
- progress: 进度百分比 (0-100)
- size: 组件尺寸 (small/default/large)

功能要求:
1. 根据状态显示不同图标和颜色
2. Running 状态显示进度条
3. Failed 状态支持展开显示错误信息
4. 支持动画效果（Running 脉冲动画）
```

### 5.3 Cron 编辑器组件

**提示词:**
```
创建 CronEditor 组件：

Props:
- value: Cron 表达式
- onChange: 值变化回调

功能要求:
1. 可视化选择器（秒/分/时/日/月/周）
2. 直接输入 Cron 表达式
3. 人类可读描述
4. 预设模板按钮
5. 下次执行时间预览（显示最近 5 次）
6. 表达式验证

预设模板:
- 每分钟
- 每小时
- 每天（指定时间）
- 每周（指定星期和时间）
- 工作日
- 每月（指定日期）
```

---

## 六、WebSocket 实时更新

### 6.1 消息类型定义

```typescript
// types/websocket.ts
interface WsMessage<T = unknown> {
  type: string;
  payload: T;
  timestamp: number;
}

// 任务状态更新
interface TaskStatusUpdate {
  taskId: number;
  status: 'running' | 'completed' | 'failed';
  progress?: number;
  message?: string;
}

// 系统通知
interface SystemNotification {
  level: 'info' | 'warning' | 'error';
  title: string;
  message: string;
}

// 存储空间更新
interface StorageUpdate {
  used: number;
  total: number;
  percentage: number;
}
```

### 6.2 WebSocket Hook

**提示词:**
```
创建 useWebSocket Hook：

功能要求:
1. 自动连接和重连
2. 心跳检测
3. 事件订阅/取消订阅
4. 消息类型分发
5. 连接状态管理
6. 错误处理

返回值:
- connected: 连接状态
- subscribe: 订阅事件
- unsubscribe: 取消订阅
- send: 发送消息
```

---

## 七、国际化配置

### 7.1 语言资源结构

```json
// locales/zh-CN.json
{
  "common": {
    "add": "新增",
    "edit": "编辑",
    "delete": "删除",
    "save": "保存",
    "cancel": "取消",
    "confirm": "确认",
    "search": "搜索",
    "reset": "重置",
    "actions": "操作",
    "status": "状态",
    "loading": "加载中...",
    "success": "操作成功",
    "error": "操作失败"
  },
  "menu": {
    "dashboard": "仪表盘",
    "channels": "频道管理",
    "tasks": "任务管理",
    "recordings": "录制管理",
    "m3uSources": "M3U 源管理",
    "live": "直播预览",
    "settings": "系统设置"
  },
  "dashboard": {
    "title": "仪表盘",
    "totalChannels": "总频道数",
    "todayRecordings": "今日录制",
    "runningTasks": "运行中任务",
    "storageUsed": "存储使用"
  },
  "channel": {
    "title": "频道管理",
    "name": "频道名称",
    "group": "分组",
    "url": "流地址",
    "source": "来源",
    "logo": "Logo",
    "online": "在线",
    "offline": "离线"
  },
  "task": {
    "title": "任务管理",
    "scheduled": "计划任务",
    "history": "历史任务",
    "cron": "执行时间",
    "duration": "录制时长",
    "outputPath": "输出路径",
    "runNow": "立即执行",
    "nextRun": "下次执行"
  },
  "recording": {
    "title": "录制管理",
    "fileName": "文件名",
    "channel": "频道",
    "size": "大小",
    "createdAt": "创建时间"
  },
  "m3uSource": {
    "title": "M3U 源管理",
    "name": "源名称",
    "type": "源类型",
    "url": "URL 地址",
    "refreshInterval": "更新间隔",
    "lastUpdate": "最后更新",
    "refresh": "刷新"
  },
  "settings": {
    "title": "系统设置",
    "basic": "基础设置",
    "storage": "存储设置",
    "recording": "录制设置",
    "notification": "通知设置",
    "appearance": "外观设置",
    "about": "关于"
  }
}
```

---

## 八、API 接口规范

### 8.1 统一响应格式

```typescript
// 成功响应
interface ApiResponse<T> {
  code: 0;
  data: T;
  message: string;
}

// 错误响应
interface ApiError {
  code: number;
  message: string;
  details?: Record<string, string>;
}
```

### 8.2 分页请求

```typescript
interface PaginationParams {
  page: number;      // 页码，从 1 开始
  pageSize: number;  // 每页数量
  sortBy?: string;   // 排序字段
  sortOrder?: 'asc' | 'desc';
}

interface PaginationResponse<T> {
  list: T[];
  total: number;
  page: number;
  pageSize: number;
  totalPages: number;
}
```

### 8.3 核心 API 列表

| 模块 | 方法 | 路径 | 描述 |
|------|------|------|------|
| 频道 | GET | /api/channels | 获取频道列表 |
| 频道 | GET | /api/channels/:id | 获取单个频道 |
| 频道 | POST | /api/channels | 创建频道 |
| 频道 | PUT | /api/channels/:id | 更新频道 |
| 频道 | DELETE | /api/channels/:id | 删除频道 |
| 任务 | GET | /api/tasks | 获取任务列表 |
| 任务 | POST | /api/tasks | 创建任务 |
| 任务 | PUT | /api/tasks/:id | 更新任务 |
| 任务 | DELETE | /api/tasks/:id | 删除任务 |
| 任务 | POST | /api/tasks/:id/run | 立即执行任务 |
| 录制 | GET | /api/recordings | 获取录制列表 |
| 录制 | GET | /api/recordings/:id | 获取录制详情 |
| 录制 | DELETE | /api/recordings/:id | 删除录制 |
| 录制 | GET | /api/recordings/:id/download | 下载录制 |
| M3U源 | GET | /api/m3u-sources | 获取源列表 |
| M3U源 | POST | /api/m3u-sources | 创建源 |
| M3U源 | POST | /api/m3u-sources/:id/refresh | 刷新源 |
| 设置 | GET | /api/settings | 获取设置 |
| 设置 | PUT | /api/settings | 更新设置 |
| 系统 | GET | /api/system/status | 系统状态 |
| 系统 | GET | /api/system/storage | 存储信息 |

---

## 九、快速生成提示词模板

### 9.1 生成页面组件

```
请为 IPTV Recorder 项目创建 [页面名称] 页面：

技术栈：React 19 + TypeScript + Ant Design 6 + Tailwind CSS + TanStack Query

功能需求：
1. [功能点1]
2. [功能点2]
3. [功能点3]

组件要求：
- 使用 react-i18next 国际化
- 使用 TanStack Query 管理数据
- 响应式布局
- 支持暗色模式

请生成完整的 TypeScript 代码。
```

### 9.2 生成 API 模块

```
请为 [模块名] 创建 API 请求模块：

基础路径: /api/[module]
请求库: axios

接口列表：
1. GET / - 获取列表（支持分页）
2. GET /:id - 获取详情
3. POST / - 创建
4. PUT /:id - 更新
5. DELETE /:id - 删除

请生成完整的 TypeScript 代码，包含类型定义。
```

### 9.3 生成 Zustand Store

```
请创建 [功能名] 的 Zustand Store：

状态：
- [状态1]: [类型]
- [状态2]: [类型]

操作：
- [操作1]: [描述]
- [操作2]: [描述]

要求：
- 使用 devtools 中间件
- 使用 persist 中间件（如需持久化）
- TypeScript 类型完整
```

---

## 十、开发检查清单

### 10.1 页面开发检查
- [ ] 组件使用 TypeScript 类型定义
- [ ] 所有文本使用 i18n
- [ ] 表单验证完整
- [ ] 错误处理和提示
- [ ] 加载状态显示
- [ ] 空状态显示
- [ ] 响应式适配
- [ ] 暗色模式适配
- [ ] 无障碍属性

### 10.2 性能优化检查
- [ ] 列表使用虚拟滚动（大数据量）
- [ ] 图片懒加载
- [ ] 组件懒加载
- [ ] 避免不必要的重渲染
- [ ] 防抖/节流处理

### 10.3 代码质量检查
- [ ] ESLint 无警告
- [ ] TypeScript 无错误
- [ ] 无 console.log
- [ ] 无硬编码文本
- [ ] 无未使用的导入

---

## 附录：常用代码片段

### A. 表格列定义
```typescript
const columns: ColumnsType<DataType> = [
  {
    title: t('common.name'),
    dataIndex: 'name',
    key: 'name',
    sorter: true,
    ellipsis: true,
  },
  {
    title: t('common.actions'),
    key: 'actions',
    width: 150,
    render: (_, record) => (
      <Space>
        <Button type="link" size="small">{t('common.edit')}</Button>
        <Popconfirm title={t('common.confirmDelete')} onConfirm={() => handleDelete(record.id)}>
          <Button type="link" size="small" danger>{t('common.delete')}</Button>
        </Popconfirm>
      </Space>
    ),
  },
];
```

### B. 表单验证规则
```typescript
const rules = {
  required: [{ required: true, message: t('validation.required') }],
  url: [
    { required: true, message: t('validation.required') },
    { type: 'url', message: t('validation.invalidUrl') },
  ],
  number: [
    { required: true, message: t('validation.required') },
    { type: 'number', min: 0, message: t('validation.positiveNumber') },
  ],
};
```

### C. Query 使用示例
```typescript
// 列表查询
const { data, isLoading } = useQuery({
  queryKey: ['items', params],
  queryFn: () => api.getItems(params),
});

// 创建/更新
const mutation = useMutation({
  mutationFn: api.createItem,
  onSuccess: () => {
    queryClient.invalidateQueries({ queryKey: ['items'] });
    message.success(t('common.success'));
  },
  onError: (error) => {
    message.error(error.message);
  },
});
```

---

**文档结束**
