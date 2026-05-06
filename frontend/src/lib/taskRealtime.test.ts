import { describe, expect, it } from 'vitest';
import type { Task } from '@/types';
import {
  applyTaskProgressUpdate,
  applyTaskStatusUpdate,
  upsertTaskList,
} from './taskRealtime';

const baseTask: Task = {
  id: 'task-1',
  schedule_id: null,
  channel_id: 'channel-1',
  status: 'pending',
  started_at: '2026-05-06T10:00:00Z',
  ended_at: null,
  exit_code: null,
  error_message: null,
  output_path: null,
  file_size: 0,
  duration_recorded: 0,
  progress_percent: 0,
  current_speed: null,
  created_at: '2026-05-06T10:00:00Z',
  updated_at: '2026-05-06T10:00:00Z',
};

describe('taskRealtime helpers', () => {
  it('upserts new tasks to the front of the list', () => {
    const nextTask = { ...baseTask, id: 'task-2' };

    expect(upsertTaskList([baseTask], nextTask).map((task) => task.id)).toEqual([
      'task-2',
      'task-1',
    ]);
  });

  it('applies progress updates and promotes pending tasks to running', () => {
    const [updated] = applyTaskProgressUpdate([baseTask], {
      task_id: baseTask.id,
      percent: 24,
      downloaded_bytes: 2048,
      speed: '1.5 MB/s',
      eta_seconds: 120,
    });

    expect(updated.status).toBe('running');
    expect(updated.progress_percent).toBe(24);
    expect(updated.file_size).toBe(2048);
    expect(updated.current_speed).toBe('1.5 MB/s');
  });

  it('applies terminal status updates and clears transient speed fields', () => {
    const runningTask: Task = {
      ...baseTask,
      status: 'running',
      current_speed: '2.0 MB/s',
      progress_percent: 83,
    };

    const [updated] = applyTaskStatusUpdate([runningTask], {
      task_id: runningTask.id,
      status: 'completed',
      error_message: null,
    });

    expect(updated.status).toBe('completed');
    expect(updated.progress_percent).toBe(100);
    expect(updated.current_speed).toBeNull();
    expect(updated.ended_at).not.toBeNull();
  });
});
