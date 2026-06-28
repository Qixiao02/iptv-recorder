import type { ConfigUpdateRequest } from '@/api/system';
import type { SystemConfig } from '@/types';

export const buildConfigUpdateRequest = (config: SystemConfig): ConfigUpdateRequest => ({
  storage: {
    recordings_path: config.storage.recordings_path,
    auto_cleanup_days: config.storage.auto_cleanup_days,
    min_free_space_gb: config.storage.min_free_space_gb,
  },
  recording: {
    default_duration_minutes: config.recording.default_duration_minutes,
    // n_m3u8dl_re_path 由后端/Docker 镜像内部集成，不开放修改，提交时不发送
    max_retry: config.recording.max_retry,
    thread_count: config.recording.thread_count,
  },
  notification: {
    on_complete: config.notification.on_complete,
    on_failure: config.notification.on_failure,
    disk_warning: config.notification.disk_warning,
  },
});
