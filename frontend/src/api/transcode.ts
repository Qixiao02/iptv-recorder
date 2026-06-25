import apiClient from './client';

// 转码响应
export interface TranscodeResponse {
  session_id: string;
  playlist_url: string;
  recording_active: boolean;
}

// 启动转码
export const startTranscode = (
  channelId: string,
): Promise<TranscodeResponse> => {
  return apiClient
    .post('/transcode/start', {
      channel_id: channelId,
    }, {
      timeout: 90000,
    })
    .then((res) => res.data);
};

// 停止转码
export const stopTranscode = (sessionId: string): Promise<void> => {
  return apiClient.post(`/transcode/${sessionId}`).then((res) => res.data);
};
