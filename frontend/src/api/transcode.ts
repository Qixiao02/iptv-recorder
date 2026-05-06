import apiClient from './client';

// 转码响应
export interface TranscodeResponse {
  session_id: string;
  playlist_url: string;
}

// 启动转码
export const startTranscode = (
  channelId: string,
): Promise<TranscodeResponse> => {
  return apiClient
    .post('/transcode/start', {
      channel_id: channelId,
    })
    .then((res) => res.data);
};

// 停止转码
export const stopTranscode = (sessionId: string): Promise<void> => {
  return apiClient.post(`/transcode/${sessionId}`).then((res) => res.data);
};
