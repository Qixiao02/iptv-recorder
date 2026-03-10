import { create } from 'zustand';
import type { Task, TaskProgressData } from '@/types';

interface TaskState {
  tasks: Task[];
  taskProgress: Record<string, TaskProgressData>;
  loading: boolean;
  setTasks: (tasks: Task[]) => void;
  updateTask: (id: string, task: Partial<Task>) => void;
  updateTaskProgress: (taskId: string, progress: TaskProgressData) => void;
  setLoading: (loading: boolean) => void;
}

export const useTaskStore = create<TaskState>((set) => ({
  tasks: [],
  taskProgress: {},
  loading: false,

  setTasks: (tasks) => set({ tasks }),

  updateTask: (id, updatedTask) =>
    set((state) => ({
      tasks: state.tasks.map((t) =>
        t.id === id ? { ...t, ...updatedTask } : t
      ),
    })),

  updateTaskProgress: (taskId, progress) =>
    set((state) => ({
      taskProgress: {
        ...state.taskProgress,
        [taskId]: progress,
      },
      tasks: state.tasks.map((t) =>
        t.id === taskId
          ? { ...t, progress_percent: progress.percent, current_speed: progress.speed }
          : t
      ),
    })),

  setLoading: (loading) => set({ loading }),
}));
