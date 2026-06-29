import { useEffect, useRef, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import Hls, { type ErrorData } from 'hls.js/light';
import { startTranscode, stopTranscode } from '@/api/transcode';
import apiClient from '@/api/client';
import { getStoredAuthToken } from '@/stores/authStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import type { Channel } from '@/types';

/**
 * 播放核心逻辑（HLS / UDP 转码 / 错误恢复 / token 鉴权）。
 *
 * 供 MiniPlayer（统一播放器，大窗/小窗两种模式）使用。外壳层只负责渲染，
 * 播放细节全部在此 hook 内。
 *
 * 用法：
 *   const { videoRef, error, loading, transcoding, recordingActive, hlsUrlRef } =
 *     usePlayerCore({ channel, active });
 *   // active=false 时不启动播放（用于组件未挂载/未打开时）
 */

// ===== API URL 解析 =====
const buildApiPath = (path: string): string => {
  if (path.startsWith('http://') || path.startsWith('https://')) {
    return path;
  }
  return path.startsWith('/') ? path : `/${path}`;
};

const isLoopbackHost = (host: string): boolean => {
  const hostname = host.split(':')[0]?.toLowerCase();
  return hostname === 'localhost' || hostname === '127.0.0.1' || hostname === '::1';
};

const resolveApiOrigin = (): string => {
  const pageHost = window.location.host;
  const configuredBase = apiClient.defaults.baseURL;
  if (typeof configuredBase === 'string' && /^https?:\/\//.test(configuredBase)) {
    const configuredUrl = new URL(configuredBase);
    if (!isLoopbackHost(pageHost) && isLoopbackHost(configuredUrl.host)) {
      return window.location.origin;
    }
    return configuredUrl.origin;
  }

  const envBase = import.meta.env.VITE_API_BASE_URL;
  if (typeof envBase === 'string' && /^https?:\/\//.test(envBase)) {
    const envUrl = new URL(envBase);
    if (!isLoopbackHost(pageHost) && isLoopbackHost(envUrl.host)) {
      return window.location.origin;
    }
    return envUrl.origin;
  }

  return window.location.origin;
};

const buildApiUrl = (path: string): string => {
  const normalizedPath = buildApiPath(path);
  return new URL(normalizedPath, resolveApiOrigin()).toString();
};

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

interface UsePlayerCoreArgs {
  /** 当前播放频道；可为 null（小窗未打开时）。null 时 active 应为 false，hook 不启动播放。 */
  channel: Channel | null;
  /** 是否激活播放。false 时不启动（组件未挂载/未打开）。 */
  active: boolean;
}

export function usePlayerCore({ channel, active }: UsePlayerCoreArgs) {
  const { t } = useTranslation(['components']);
  useI18nNamespace('components');
  // channel 可能为 null（小窗未打开时 hook 仍会被调用，React 要求 hook 无条件执行）。
  // 用安全解构：channel 为 null 时取默认空值，配合 active=false 使所有 effect 不启动。
  const {
    id: channelId = '',
    url: channelUrl = '',
    source_visibility = 'public',
    playback_strategy = 'auto',
  } = channel ?? ({} as Partial<Channel>);

  const videoRef = useRef<HTMLVideoElement>(null);
  const hlsRef = useRef<Hls | null>(null);
  const sessionIdRef = useRef<string | null>(null);
  const sessionChannelIdRef = useRef<string | null>(null);
  const hlsUrlRef = useRef<string | null>(null); // 保存转码后的 HLS URL
  const recoveryAttemptRef = useRef({ media: 0, network: 0 });
  const playAttemptRef = useRef(0);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [transcoding, setTranscoding] = useState(false);
  const [recordingActive, setRecordingActive] = useState(false);
  const playbackStartedRef = useRef(false);

  const requestVideoPlay = useCallback(async (video: HTMLVideoElement) => {
    try {
      await video.play();
      return true;
    } catch (e) {
      console.warn('Video play() was deferred:', e);
      return false;
    }
  }, []);

  // 停止转码
  const cleanupTranscode = useCallback(async () => {
    if (sessionIdRef.current) {
      const sessionId = sessionIdRef.current;
      sessionIdRef.current = null; // 先清除引用，防止重复调用
      sessionChannelIdRef.current = null;
      try {
        await stopTranscode(sessionId);
        console.log('Transcode stopped:', sessionId);
      } catch (e) {
        console.error('Failed to stop transcode:', e);
      }
    }
  }, []);

  // 清理 HLS
  const cleanupHls = useCallback(() => {
    playAttemptRef.current += 1;
    if (hlsRef.current) {
      hlsRef.current.destroy();
      hlsRef.current = null;
    }
    hlsUrlRef.current = null;
    if (videoRef.current) {
      videoRef.current.pause();
      videoRef.current.removeAttribute('src');
      videoRef.current.load();
    }
    recoveryAttemptRef.current = { media: 0, network: 0 };
    playbackStartedRef.current = false;
  }, []);

  const tryStartPlayback = useCallback(async (
    video: HTMLVideoElement,
    minBufferedSeconds: number,
  ) => {
    if (playbackStartedRef.current) {
      return;
    }

    const bufferedEnd = video.buffered.length > 0 ? video.buffered.end(video.buffered.length - 1) : 0;
    const bufferedSeconds = Math.max(0, bufferedEnd - video.currentTime);
    const hasFutureData = video.readyState >= HTMLMediaElement.HAVE_FUTURE_DATA;

    if (bufferedSeconds < minBufferedSeconds && !hasFutureData) {
      if (video.readyState >= HTMLMediaElement.HAVE_METADATA) {
        await requestVideoPlay(video);
      }
      return;
    }

    setLoading(false);
    setTranscoding(false);
    setError(null);
    playbackStartedRef.current = true;
    const started = await requestVideoPlay(video);
    if (!started) {
      playbackStartedRef.current = false;
      setLoading(true);
    }
  }, [requestVideoPlay]);

  const attachHlsErrorRecovery = useCallback((
    hls: Hls,
    sourceUrl: string,
    onFatal: (message: string) => void,
  ) => {
    hls.on(Hls.Events.ERROR, (_event, data: ErrorData) => {
      // ===== 非致命错误：记录后交由 hls.js 内部重试机制处理 =====
      // 这包括单个分片加载失败的初次重试(fragLoadingMaxRetry=10)。
      if (!data.fatal) {
        return;
      }

      console.error('HLS fatal Error:', data);

      if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
        if (recoveryAttemptRef.current.media < 3) {
          recoveryAttemptRef.current.media += 1;
          console.warn(`Recovering media error (${recoveryAttemptRef.current.media}/3)...`);
          hls.recoverMediaError();
          return;
        }
      }

      if (data.type === Hls.ErrorTypes.NETWORK_ERROR) {
        // 直播流在网络错误(包括上游网关重置期间的分片 404/超时)时，
        // 用 startLoad 从当前播放位置恢复，而不是 reloadSource 从头开始。
        // 后端 ffmpeg 配了 reconnect 会自动重连产出新分片，这里只要继续推进即可。
        // 给一个较大的重试上限，因为 UDP-over-HTTP 网关可能周期性重置。
        if (recoveryAttemptRef.current.network < 5) {
          recoveryAttemptRef.current.network += 1;
          console.warn(`Recovering network error (${recoveryAttemptRef.current.network}/5), resuming load...`);
          hls.startLoad();
          return;
        }
        // 超过上限才回退到完整重载(清理状态重来)。
        console.warn('Network recovery exhausted, doing full reload...');
        recoveryAttemptRef.current.network = 0;
        hls.stopLoad();
        setTimeout(() => {
          hls.loadSource(sourceUrl);
          hls.startLoad();
        }, 500);
        return;
      }

      onFatal(t('components:player.playFailed', { type: data.type, details: data.details }));
    });
  }, [t]);

  // 卸载时清理
  useEffect(() => {
    return () => {
      cleanupHls();
      void cleanupTranscode();
    };
  }, [cleanupHls, cleanupTranscode]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !active) {
      return;
    }

    const handleCanPlay = () => {
      setLoading(false);
      setTranscoding(false);
    };

    const handlePlaying = () => {
      playbackStartedRef.current = true;
      setLoading(false);
      setTranscoding(false);
      setError(null);
    };

    const handleWaiting = () => {
      if (playbackStartedRef.current) {
        setLoading(true);
      }
    };

    const handleEnded = () => {
      setLoading(false);
    };

    const handleLoadedMetadata = () => {
      void requestVideoPlay(video);
    };

    const handleVideoError = () => {
      const mediaError = video.error;
      const message = mediaError
        ? t('components:player.decodeFailed', { code: mediaError.code })
        : t('components:player.videoLoadFailed');
      setError(message);
      setLoading(false);
      setTranscoding(false);
    };

    video.addEventListener('canplay', handleCanPlay);
    video.addEventListener('loadedmetadata', handleLoadedMetadata);
    video.addEventListener('playing', handlePlaying);
    video.addEventListener('waiting', handleWaiting);
    video.addEventListener('stalled', handleWaiting);
    video.addEventListener('ended', handleEnded);
    video.addEventListener('error', handleVideoError);

    return () => {
      video.removeEventListener('canplay', handleCanPlay);
      video.removeEventListener('loadedmetadata', handleLoadedMetadata);
      video.removeEventListener('playing', handlePlaying);
      video.removeEventListener('waiting', handleWaiting);
      video.removeEventListener('stalled', handleWaiting);
      video.removeEventListener('ended', handleEnded);
      video.removeEventListener('error', handleVideoError);
    };
  }, [active, requestVideoPlay, t]);

  // 播放 UDP 流（需要转码）
  const playUDPStream = useCallback(async () => {
    const attemptId = ++playAttemptRef.current;
    const isCurrentAttempt = () => playAttemptRef.current === attemptId;

    setTranscoding(true);
    setLoading(true);
    setError(null);

    try {
      // 仅在切换到其他频道时停止旧会话，避免同频道的重复初始化把刚起来的预览会话停掉。
      if (sessionIdRef.current && sessionChannelIdRef.current !== channelId) {
        await cleanupTranscode();
      }
      recoveryAttemptRef.current = { media: 0, network: 0 };

      console.log('Starting transcode for channel:', channelId);
      const result = await startTranscode(channelId);
      if (!isCurrentAttempt()) {
        await stopTranscode(result.session_id).catch(() => {});
        return;
      }
      sessionIdRef.current = result.session_id;
      sessionChannelIdRef.current = channelId;
      setRecordingActive(result.recording_active);

      const hlsUrl = buildApiUrl(result.playlist_url);
      hlsUrlRef.current = hlsUrl;
      console.log('Transcode started, playlist URL:', hlsUrl);

      // 后端会等到首个可播放片段基本就绪，这里只做一个很短的二次确认。
      let verified = false;
      for (let i = 0; i < 6; i++) {
        await wait(500);
        console.log(`Verifying HLS file... (attempt ${i + 1}/6)`);
        try {
          const checkResp = await fetch(hlsUrl);
          if (checkResp.ok) {
            console.log('HLS file verified');
            verified = true;
            break;
          }
        } catch {
          console.log('HLS file check failed, retrying...');
        }
      }

      if (!verified) {
        throw new Error(t('components:player.hlsTimeout'));
      }

      if (!isCurrentAttempt()) {
        return;
      }

      if (!videoRef.current) return;

      const video = videoRef.current;

      if (Hls.isSupported()) {
        const hls = new Hls({
          enableWorker: true,
          lowLatencyMode: false,
          // 边看边转码场景:后端写分片有抖动,前端要多囤缓冲 + 放宽时间戳容差,
          // 避免一点点抖动就转圈。尤其 UDP-over-HTTP 网关会周期性重置连接,
          // 中断期间会产生 0 字节分片,需要播放器有足够缓冲吸收。
          maxBufferLength: 60,
          maxMaxBufferLength: 120,
          maxBufferSize: 120 * 1000 * 1000,
          // copy/remux 分片时长会在 GOP 边界漂移(2~6s),
          // 0.8s 容差太严会反复触发缓冲缺口判定。放宽到 2s。
          maxBufferHole: 2.0,
          liveDurationInfinity: true,
          backBufferLength: 30,
          liveBackBufferLength: 30,
          startLevel: -1,
          autoStartLoad: true,
          // 落后直播边缘 6 个分片(~36s @6s 分片),给后端重连足够容错空间。
          liveSyncDurationCount: 6,
          liveMaxLatencyDurationCount: 15,
          startFragPrefetch: true,
          manifestLoadingTimeOut: 10000,
          manifestLoadingMaxRetry: 6,
          levelLoadingTimeOut: 10000,
          levelLoadingMaxRetry: 6,
          fragLoadingTimeOut: 20000,
          // 分片加载失败重试次数提高:网关重置期间会有几个坏分片,
          // 多重试几次能跳过中断窗口,避免直接判 fatal 卡死。
          fragLoadingMaxRetry: 10,
          // 坏分片重试之间的间隔,给后端 ffmpeg 重连留时间。
          fragLoadingRetryDelay: 1000,
        });

        hls.loadSource(hlsUrl);
        hls.attachMedia(video);

        hls.on(Hls.Events.MANIFEST_PARSED, () => {
          console.log('HLS manifest parsed, starting playback');
          recoveryAttemptRef.current = { media: 0, network: 0 };
          setTranscoding(false);
          setLoading(true);
          void requestVideoPlay(video);
        });
        hls.on(Hls.Events.BUFFER_APPENDED, async () => {
          if (!isCurrentAttempt()) {
            return;
          }
          await tryStartPlayback(video, 6);
        });
        attachHlsErrorRecovery(hls, hlsUrl, (message) => {
          if (!isCurrentAttempt()) {
            return;
          }
          setError(message);
          setLoading(false);
          setTranscoding(false);
        });

        hlsRef.current = hls;
      } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
        video.src = hlsUrl;
        video.addEventListener('loadedmetadata', async () => {
          setTranscoding(false);
          await wait(300);
          await tryStartPlayback(video, 0);
        });
      }
    } catch (e) {
      if (!isCurrentAttempt()) {
        return;
      }
      console.error('Transcode error:', e);
      setError(e instanceof Error ? e.message : t('components:player.transcodeStartFailed'));
      setLoading(false);
      setTranscoding(false);
    }
  }, [attachHlsErrorRecovery, channelId, cleanupTranscode, requestVideoPlay, t, tryStartPlayback]);

  // 播放 HLS 流
  const playHLSStream = useCallback(() => {
    const attemptId = ++playAttemptRef.current;
    const isCurrentAttempt = () => playAttemptRef.current === attemptId;
    if (!videoRef.current) return;

    const video = videoRef.current;
    const token = getStoredAuthToken();
    const proxyUrl = buildApiUrl(`/api/channels/${channelId}/stream${
      token ? `?token=${encodeURIComponent(token)}` : ''
    }`);

    if (Hls.isSupported()) {
      const hls = new Hls({
        enableWorker: true,
        lowLatencyMode: false,
        maxBufferLength: 40,
        maxMaxBufferLength: 90,
        // 直连源同样存在 remux 漂移与网关重置,放宽容差。
        maxBufferHole: 2.0,
        liveSyncDurationCount: 5,
        liveMaxLatencyDurationCount: 12,
        fragLoadingMaxRetry: 10,
        fragLoadingRetryDelay: 1000,
      });

      hls.loadSource(proxyUrl);
      hls.attachMedia(video);

      hls.on(Hls.Events.MANIFEST_PARSED, () => {
          recoveryAttemptRef.current = { media: 0, network: 0 };
          void requestVideoPlay(video);
      });
      hls.on(Hls.Events.BUFFER_APPENDED, async () => {
        if (!isCurrentAttempt()) {
          return;
        }
        await tryStartPlayback(video, 6);
      });
      attachHlsErrorRecovery(hls, proxyUrl, (message) => {
        if (!isCurrentAttempt()) {
          return;
        }
        setError(message);
        setLoading(false);
      });

      hlsRef.current = hls;
    } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
      video.src = proxyUrl;
      video.addEventListener('loadedmetadata', async () => {
        await wait(300);
        await tryStartPlayback(video, 0);
      });
    }
  }, [channelId, attachHlsErrorRecovery, requestVideoPlay, tryStartPlayback]);

  // 播放其他流
  const playOtherStream = useCallback(() => {
    ++playAttemptRef.current;
    if (!videoRef.current) return;

    const video = videoRef.current;
    const token = getStoredAuthToken();
    const proxyUrl = buildApiUrl(`/api/channels/${channelId}/stream${
      token ? `?token=${encodeURIComponent(token)}` : ''
    }`);

    video.src = proxyUrl;
    video.addEventListener('loadeddata', () => {
      setLoading(false);
      video.play().catch(() => {});
    });
    video.addEventListener('error', () => {
      setError(t('components:player.formatUnsupported'));
      setLoading(false);
    });
  }, [channelId, t]);

  // 重试（错误恢复用）：根据频道类型重新选播放路径
  const retry = useCallback(() => {
    setError(null);
    setLoading(true);
    const isUDP = channelUrl.includes('/udp/');
    const mustUseHls =
      playback_strategy === 'hls_only'
      || source_visibility === 'private_server_only'
      || isUDP;

    if (mustUseHls) {
      void playUDPStream();
    } else if (channelUrl.includes('.m3u8') || channelUrl.includes('m3u8') || playback_strategy === 'proxy_only') {
      playHLSStream();
    } else {
      playOtherStream();
    }
  }, [channelUrl, playback_strategy, source_visibility, playUDPStream, playHLSStream, playOtherStream]);

  // 初始化播放
  useEffect(() => {
    if (active && channelUrl) {
      // 重置状态
      cleanupHls();
      setError(null);
      setLoading(true);
      setTranscoding(false);
      setRecordingActive(false);
      recoveryAttemptRef.current = { media: 0, network: 0 };
      playbackStartedRef.current = false;

      const isHLS = channelUrl.includes('.m3u8') || channelUrl.includes('m3u8');
      const isUDP = channelUrl.includes('/udp/');

      const mustUseHls =
        playback_strategy === 'hls_only' ||
        source_visibility === 'private_server_only' ||
        isUDP;

      if (playback_strategy === 'record_only') {
        setError(t('components:player.recordOnly'));
        setLoading(false);
        return;
      }

      if (mustUseHls) {
        void playUDPStream();
      } else if (isHLS || playback_strategy === 'proxy_only') {
        playHLSStream();
      } else {
        playOtherStream();
      }
    }

    return () => {
      cleanupHls();
    };
  }, [active, channelUrl, cleanupHls, playUDPStream, playHLSStream, playOtherStream, playback_strategy, source_visibility, t]);

  return {
    videoRef,
    hlsUrlRef,
    error,
    loading,
    transcoding,
    recordingActive,
    retry,
    cleanupHls,
    cleanupTranscode,
  };
}
