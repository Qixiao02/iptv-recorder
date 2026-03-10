import apiClient from './client';
import type { UpcomingTask, SystemConfig } from '@/types';

// 获取即将执行的任务
export const getUpcoming = (): Promise<UpcomingTask[]> => {
  return apiClient.get('/scheduler/upcoming').then((res) => res.data);
};

// 重新加载调度器
export const reloadScheduler = (): Promise<{ status: string; message: string }> => {
  return apiClient.post('/scheduler/reload').then((res) => res.data);
};

// 获取系统配置
export const getConfig = (): Promise<SystemConfig> => {
  return apiClient.get('/config').then((res) => res.data);
};

// 更新系统配置
export const updateConfig = (data: Partial<SystemConfig>): Promise<SystemConfig> => {
  return apiClient.post('/config', data).then((res) => res.data);
};
