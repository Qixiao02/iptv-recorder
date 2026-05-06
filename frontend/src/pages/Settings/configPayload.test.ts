import { describe, expect, it } from 'vitest';
import { buildConfigUpdateRequest } from './configPayload';

describe('buildConfigUpdateRequest', () => {
  it('builds the backend patch payload from full settings state', () => {
    const payload = buildConfigUpdateRequest({
      server: { host: '127.0.0.1', port: 3000 },
      storage: {
        recordings_path: './recordings',
        auto_cleanup_days: 14,
        min_free_space_gb: 8,
      },
      recording: {
        default_duration_minutes: 90,
        n_m3u8dl_re_path: '/usr/local/bin/N_m3u8DL-RE',
        max_retry: 5,
        thread_count: 6,
      },
      notification: {
        on_complete: true,
        on_failure: false,
        disk_warning: true,
      },
    });

    expect(payload).toEqual({
      storage: {
        recordings_path: './recordings',
        auto_cleanup_days: 14,
        min_free_space_gb: 8,
      },
      recording: {
        default_duration_minutes: 90,
        n_m3u8dl_re_path: '/usr/local/bin/N_m3u8DL-RE',
        max_retry: 5,
        thread_count: 6,
      },
      notification: {
        on_complete: true,
        on_failure: false,
        disk_warning: true,
      },
    });
  });
});
