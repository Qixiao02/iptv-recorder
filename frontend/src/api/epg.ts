import apiClient from './client';
import type { EpgProgram, EpgSource, ImportEpgRequest } from '@/types';

export const getEpgSources = (): Promise<EpgSource[]> => {
  return apiClient.get('/epg/sources').then((res) => res.data);
};

export const importEpgSource = (data: ImportEpgRequest): Promise<EpgSource> => {
  return apiClient.post('/epg/sources', data).then((res) => res.data);
};

export const getEpgPrograms = (channelRef: string, limit = 20): Promise<EpgProgram[]> => {
  const query = new URLSearchParams({
    channel_ref: channelRef,
    limit: limit.toString(),
  });
  return apiClient.get(`/epg/programs?${query.toString()}`).then((res) => res.data);
};
