import apiClient from './client';
import type {
  Channel,
  CreateChannelRequest,
  ImportM3URequest,
  ImportM3UResponse,
  ChannelTestResult,
} from '@/types';

// 分页参数
export interface PaginationParams {
  page?: number;
  page_size?: number;
  group?: string;
  search?: string;
}

// 分页响应
export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

// 获取频道列表（分页）
export const getChannels = (params?: PaginationParams): Promise<PaginatedResponse<Channel>> => {
  const queryParams = new URLSearchParams();
  if (params?.page) queryParams.append('page', params.page.toString());
  if (params?.page_size) queryParams.append('page_size', params.page_size.toString());
  if (params?.group) queryParams.append('group', params.group);
  if (params?.search) queryParams.append('search', params.search);

  const queryString = queryParams.toString();
  const url = queryString ? `/channels?${queryString}` : '/channels';
  return apiClient.get(url).then((res) => res.data);
};

// 获取全部频道（不分页，用于下拉选择等场景）
export const getAllChannels = (): Promise<Channel[]> => {
  return apiClient.get('/channels?page_size=1000').then((res) => res.data.items);
};

// 获取单个频道
export const getChannel = (id: string): Promise<Channel> => {
  return apiClient.get(`/channels/${id}`).then((res) => res.data);
};

// 创建频道
export const createChannel = (data: CreateChannelRequest): Promise<Channel> => {
  return apiClient.post('/channels', data).then((res) => res.data);
};

// 更新频道
export const updateChannel = (id: string, data: CreateChannelRequest): Promise<Channel> => {
  return apiClient.put(`/channels/${id}`, data).then((res) => res.data);
};

// 删除频道
export const deleteChannel = (id: string): Promise<void> => {
  return apiClient.delete(`/channels/${id}`).then((res) => res.data);
};

// 获取频道分组
export const getChannelGroups = (): Promise<string[]> => {
  return apiClient.get('/channels/groups').then((res) => res.data);
};

// 从 URL 导入 M3U
export const importM3UFromUrl = (data: ImportM3URequest): Promise<ImportM3UResponse> => {
  return apiClient.post('/channels/import/url', data).then((res) => res.data);
};

// 从内容导入 M3U
export const importM3UFromContent = (data: ImportM3URequest): Promise<ImportM3UResponse> => {
  return apiClient.post('/channels/import/content', data).then((res) => res.data);
};

// 测试频道连接
export const testChannel = (id: string): Promise<ChannelTestResult> => {
  return apiClient.post(`/channels/${id}/test`).then((res) => res.data);
};
