// ==================== 类型定义 ====================

export interface Channel {
  id: string;
  name: string;
  url: string;
  group_name: string;
  logo_url: string | null;
  source_type: string;
  source_url: string | null;
  status: 'online' | 'offline' | 'slow' | 'unknown';
  last_check_at: string | null;
  fail_count: number;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface CreateChannelRequest {
  name: string;
  url: string;
  group_name?: string;
  logo_url?: string;
}

export interface Schedule {
  id: string;
  name: string;
  channel_id: string;
  cron_expression: string;
  duration_seconds: number;
  output_template: string;
  output_dir: string | null;
  priority: number;
  enabled: boolean;
  max_retry: number;
  notify_on_complete: boolean;
  video_quality: string;
  audio_quality: string;
  max_speed: string | null;
  thread_count: number;
  transcode_mode: string;
  transcode_preset: string;
  created_at: string;
  updated_at: string;
}

export interface CreateScheduleRequest {
  name: string;
  channel_id: string;
  cron_expression: string;
  duration_seconds: number;
  output_template?: string;
  output_dir?: string;
  priority?: number;
  video_quality?: string;
  audio_quality?: string;
  max_speed?: string;
  thread_count?: number;
  transcode_mode?: string;
  transcode_preset?: string;
}

export type TaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface Task {
  id: string;
  schedule_id: string | null;
  channel_id: string;
  status: TaskStatus;
  started_at: string | null;
  ended_at: string | null;
  exit_code: number | null;
  error_message: string | null;
  output_path: string | null;
  file_size: number;
  duration_recorded: number;
  progress_percent: number;
  current_speed: string | null;
  created_at: string;
  updated_at: string;
}

export interface ManualRecordRequest {
  channel_id: string;
  duration_seconds?: number;
  output_name?: string;
  output_dir?: string;
  output_template?: string;
  video_quality?: string;
  audio_quality?: string;
  max_speed?: string;
  thread_count?: number;
}

export interface UpcomingTask {
  schedule_id: string;
  schedule_name: string;
  channel_id: string;
  next_run: string;
  duration_seconds: number;
}

export interface ImportM3URequest {
  url?: string;
  content?: string;
  overwrite?: boolean;
}

export interface ImportM3UResponse {
  imported: number;
  skipped: number;
  failed: number;
  errors: string[];
}

export interface ChannelTestResult {
  channel_id: string;
  status: 'online' | 'offline';
  response_time_ms: number | null;
  error: string | null;
}

export interface ErrorResponse {
  error: string;
  details?: string;
}

export interface SystemConfig {
  server: {
    host: string;
    port: number;
  };
  storage: {
    recordings_path: string;
    auto_cleanup_days: number;
    min_free_space_gb: number;
  };
  recording: {
    default_duration_minutes: number;
    n_m3u8dl_re_path: string;
    max_retry: number;
    thread_count: number;
  };
  notification: {
    on_complete: boolean;
    on_failure: boolean;
    disk_warning: boolean;
  };
}

// WebSocket 消息类型
export type WsMessageType =
  | 'task.update'
  | 'task.progress'
  | 'channel.status'
  | 'system.alert'
  | 'ping'
  | 'pong';

export interface WsMessage {
  type: WsMessageType;
  data?: unknown;
}

export interface TaskUpdateData {
  task_id: string;
  status: TaskStatus;
  error_message: string | null;
}

export interface TaskProgressData {
  task_id: string;
  percent: number;
  downloaded_bytes: number;
  speed: string;
  eta_seconds: number | null;
}

export interface ChannelStatusData {
  channel_id: string;
  status: string;
}

export interface SystemAlertData {
  level: 'info' | 'warning' | 'error' | 'critical';
  message: string;
  details?: string;
}
