import apiClient from './client';
import type { PaginatedResponse } from './channels';
import type { AppNotification } from '@/types';

export interface NotificationListParams {
  page?: number;
  page_size?: number;
}

/** 分页查询通知（最新在前） */
export const getNotifications = (
  params: NotificationListParams = {}
): Promise<PaginatedResponse<AppNotification>> => {
  return apiClient
    .get<PaginatedResponse<AppNotification>>('/notifications', { params })
    .then((res) => res.data);
};

/** 未读通知数量 */
export const getUnreadCount = (): Promise<{ count: number }> => {
  return apiClient.get('/notifications/unread-count').then((res) => res.data);
};

/** 标记单条通知已读 */
export const markNotificationRead = (id: string): Promise<{ updated: boolean }> => {
  return apiClient.post(`/notifications/${id}/read`).then((res) => res.data);
};

/** 全部通知标记已读 */
export const markAllNotificationsRead = (): Promise<{ updated: number }> => {
  return apiClient.post('/notifications/read-all').then((res) => res.data);
};

/** 删除单条通知 */
export const deleteNotification = (id: string): Promise<{ deleted: boolean }> => {
  return apiClient.delete(`/notifications/${id}`).then((res) => res.data);
};
