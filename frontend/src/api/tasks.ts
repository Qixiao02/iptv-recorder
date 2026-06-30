import apiClient from './client';
import type {
  Task,
  TaskListParams,
  PaginatedTasks,
  ManualRecordRequest,
} from '@/types';

// 获取任务列表(分页 + 状态筛选,返回信封)。省略参数时后端默认 page=1/page_size=20。
export const getTasks = (params?: TaskListParams): Promise<PaginatedTasks> => {
  return apiClient.get('/tasks', { params }).then((res) => res.data);
};

// 获取单个任务
export const getTask = (id: string): Promise<Task> => {
  return apiClient.get(`/tasks/${id}`).then((res) => res.data);
};

// 取消任务
export const cancelTask = (id: string): Promise<void> => {
  return apiClient.post(`/tasks/${id}/cancel`).then((res) => res.data);
};

// 手动录制
export const startManualRecord = (data: ManualRecordRequest): Promise<Task> => {
  return apiClient.post('/tasks/manual', data).then((res) => res.data);
};

// 清除已完成的任务记录
export const clearCompletedTasks = (): Promise<{ deleted: number }> => {
  return apiClient.post('/tasks/clear').then((res) => res.data);
};

// 删除单条任务记录
export const deleteTask = (id: string): Promise<void> => {
  return apiClient.delete(`/tasks/${id}`).then((res) => res.data);
};
