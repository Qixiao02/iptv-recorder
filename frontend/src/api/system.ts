import apiClient from './client';
import type { UpcomingTask, SystemConfig, AuditLog, SystemHealth } from '@/types';

export interface ConfigUpdateRequest {
  storage?: {
    recordings_path?: string;
    auto_cleanup_days?: number;
    min_free_space_gb?: number;
  };
  recording?: {
    default_duration_minutes?: number;
    n_m3u8dl_re_path?: string;
    max_retry?: number;
    thread_count?: number;
  };
  notification?: {
    on_complete?: boolean;
    on_failure?: boolean;
    disk_warning?: boolean;
  };
}

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
export const updateConfig = (data: ConfigUpdateRequest): Promise<SystemConfig> => {
  return apiClient.post('/config', data).then((res) => res.data);
};

export const getSystemHealth = (): Promise<SystemHealth> => {
  return apiClient.get('/system/health').then((res) => res.data);
};

export const getAuditLogs = (): Promise<AuditLog[]> => {
  return apiClient.get('/audit/logs').then((res) => res.data);
};

export const runCleanup = (): Promise<{ deleted: number; message: string }> => {
  return apiClient.post('/system/cleanup/run').then((res) => res.data);
};
