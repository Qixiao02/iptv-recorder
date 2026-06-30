import type { QueryClient } from '@tanstack/react-query';
import type { PaginatedTasks, Task, TaskProgressData, TaskUpdateData } from '@/types';
import { taskKeys } from '@/lib/queryKeys';

const TERMINAL_TASK_STATUSES = new Set(['completed', 'failed', 'cancelled']);

const mergeTask = (current: Task, next: Task): Task => ({
  ...current,
  ...next,
  progress_percent:
    next.status === 'completed'
      ? 100
      : next.progress_percent ?? current.progress_percent,
});

export const upsertTaskList = (tasks: Task[], nextTask: Task): Task[] => {
  const existingIndex = tasks.findIndex((task) => task.id === nextTask.id);
  if (existingIndex === -1) {
    return [nextTask, ...tasks];
  }

  return tasks.map((task, index) => (
    index === existingIndex ? mergeTask(task, nextTask) : task
  ));
};

export const applyTaskProgressUpdate = (
  tasks: Task[],
  progress: TaskProgressData,
): Task[] => tasks.map((task) => (
  task.id === progress.task_id
    ? {
        ...task,
        progress_percent: progress.percent,
        current_speed: progress.speed || task.current_speed,
        file_size: Number(progress.downloaded_bytes) || task.file_size,
        status: task.status === 'pending' ? 'running' : task.status,
      }
    : task
));

export const applyTaskStatusUpdate = (
  tasks: Task[],
  update: TaskUpdateData,
): Task[] => tasks.map((task) => {
  if (task.id !== update.task_id) {
    return task;
  }

  const nextStatus = update.status;
  const nextTask: Task = {
    ...task,
    status: nextStatus,
    error_message: update.error_message,
    progress_percent: nextStatus === 'completed' ? 100 : task.progress_percent,
    current_speed: TERMINAL_TASK_STATUSES.has(nextStatus) ? null : task.current_speed,
    ended_at:
      TERMINAL_TASK_STATUSES.has(nextStatus) && !task.ended_at
        ? new Date().toISOString()
        : task.ended_at,
  };

  return nextTask;
});

/**
 * 用 WS 进度更新补丁所有任务列表缓存。
 *
 * 列表查询分页 + 按状态筛选后,每个 (status,page,page_size) 组合是独立缓存项,
 * 返回 PaginatedTasks 信封。本函数用 setQueriesData({ root }) 前缀匹配遍历所有
 * 任务缓存,把更新应用到对应信封的 items 数组。
 *
 * 返回是否有任何缓存被实际修改(用于上层决定是否还要 invalidate 兜底)。
 */
export const patchTaskCache = (
  queryClient: QueryClient,
  updater: (tasks: Task[]) => Task[],
): boolean => {
  let changed = false;

  queryClient.setQueriesData<PaginatedTasks>({ queryKey: taskKeys.root }, (current) => {
    if (!current || !Array.isArray(current.items)) {
      return current;
    }
    const nextItems = updater(current.items);
    if (nextItems === current.items) {
      return current;
    }
    changed = true;
    return { ...current, items: nextItems };
  });

  return changed;
};

/**
 * 往所有任务列表缓存插入/更新单条任务(乐观更新)。
 * 与 patchTaskCache 同理,遍历所有任务信封缓存。
 */
export const upsertTaskCache = (queryClient: QueryClient, task: Task): void => {
  queryClient.setQueriesData<PaginatedTasks>({ queryKey: taskKeys.root }, (current) => {
    if (!current || !Array.isArray(current.items)) {
      return current;
    }
    return { ...current, items: upsertTaskList(current.items, task) };
  });
};
