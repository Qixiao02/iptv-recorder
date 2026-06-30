import type { QueryClient } from '@tanstack/react-query';
import type { Task, TaskProgressData, TaskUpdateData } from '@/types';
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

export const patchTaskCache = (
  queryClient: QueryClient,
  updater: (tasks: Task[]) => Task[],
): boolean => {
  let changed = false;

  queryClient.setQueryData<Task[]>(taskKeys.all(), (current) => {
    if (!current) {
      return current;
    }

    changed = true;
    return updater(current);
  });

  return changed;
};

export const upsertTaskCache = (queryClient: QueryClient, task: Task): void => {
  queryClient.setQueryData<Task[]>(taskKeys.all(), (current) => (
    current ? upsertTaskList(current, task) : [task]
  ));
};
