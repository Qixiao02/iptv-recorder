import apiClient from './client';
import type {
  Schedule,
  CreateScheduleRequest,
} from '@/types';

// 获取计划列表
export const getSchedules = (): Promise<Schedule[]> => {
  return apiClient.get('/schedules').then((res) => res.data);
};

// 获取单个计划
export const getSchedule = (id: string): Promise<Schedule> => {
  return apiClient.get(`/schedules/${id}`).then((res) => res.data);
};

// 创建计划
export const createSchedule = (data: CreateScheduleRequest): Promise<Schedule> => {
  return apiClient.post('/schedules', data).then((res) => res.data);
};

// 更新计划
export const updateSchedule = (id: string, data: CreateScheduleRequest): Promise<Schedule> => {
  return apiClient.put(`/schedules/${id}`, data).then((res) => res.data);
};

// 删除计划
export const deleteSchedule = (id: string): Promise<void> => {
  return apiClient.delete(`/schedules/${id}`).then((res) => res.data);
};

// 切换计划状态
export const toggleSchedule = (id: string): Promise<Schedule> => {
  return apiClient.post(`/schedules/${id}/toggle`).then((res) => res.data);
};
