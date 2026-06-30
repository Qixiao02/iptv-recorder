/**
 * 集中式 React Query 缓存键工厂。
 *
 * 此前 queryKey 字面量散落 13+ 文件(如 ['tasks']、['channels','all']),一处拼写错误
 * 会让缓存失效/实时更新静默失效。本工厂集中管理,保证:
 *  - 读查询用带参数的具体键(精确匹配);
 *  - 失效/批量更新用根键(prefix 匹配,React Query 对 queryKey 数组做前缀匹配)。
 *
 * 关键约定:React Query 对 `invalidateQueries({ queryKey: [...] })` 和
 * `setQueriesData({ queryKey: [...] })` 做前缀匹配,而 `setQueryData` 做精确匹配。
 * 因此根键(如 `channelKeys.root`、`notificationKeys.root`)用于前缀失效,
 * 具体键(如 `channelKeys.all()`、`taskKeys.all()`)用于读/精确写。
 */

/** 任务列表。当前为不分页的精确键(WS 实时补丁直接 setQueryData 此键)。 */
export const taskKeys = {
  /** 根键,前缀失效用 */
  root: ['tasks'] as const,
  /** 任务列表精确键(当前唯一列表查询) */
  all: () => ['tasks'] as const,
};

/** 频道。列表查询是分页的,全量/计数/分组是独立精确键;root 用于前缀失效。 */
export const channelKeys = {
  root: ['channels'] as const,
  /** 分页列表(参数化,与 Channels 页过滤条件绑定) */
  list: (params: readonly unknown[]) => ['channels', ...params] as const,
  /** 全量频道(getAllChannels,用于频道选择器等) */
  all: () => ['channels', 'all'] as const,
  /** 频道总数(Dashboard 等) */
  count: () => ['channels', 'count'] as const,
  /** 频道分组(distinct group_name) */
  groups: () => ['channels', 'groups'] as const,
};

export const configKeys = {
  root: ['config'] as const,
  /** 系统配置 */
  config: () => ['config'] as const,
  /** 系统健康状态 */
  health: () => ['system', 'health'] as const,
};

export const notificationKeys = {
  root: ['notifications'] as const,
  /** 通知分页列表 */
  list: (page: number) => ['notifications', page] as const,
};

export const auditKeys = {
  root: ['audit'] as const,
  /** 审计日志分页 */
  logs: (page: number, pageSize: number) => ['audit', 'logs', page, pageSize] as const,
};

export const scheduleKeys = {
  root: ['schedules'] as const,
  /** 计划列表 */
  all: () => ['schedules'] as const,
};

export const upcomingKeys = {
  /** 即将到来的计划任务(调度器) */
  upcoming: () => ['upcoming'] as const,
};

export const epgKeys = {
  root: ['epg'] as const,
  /** EPG 源列表 */
  sources: () => ['epg', 'sources'] as const,
  /** 某频道的节目单 */
  programs: (channelRef: string) => ['epg', 'programs', channelRef] as const,
};
