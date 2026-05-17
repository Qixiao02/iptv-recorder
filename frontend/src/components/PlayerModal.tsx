import React, { useEffect, useRef, useState, useCallback } from 'react';
import Hls, { type ErrorData } from 'hls.js/light';
import { X, ExternalLink, AlertCircle, Loader2 } from 'lucide-react';
import { startTranscode, stopTranscode } from '@/api/transcode';
import apiClient from '@/api/client';
import { getStoredAuthToken } from '@/stores/authStore';
import type { Channel } from '@/types';
import './PlayerModal.css';

interface PlayerModalProps {
  isOpen: boolean;
  onClose: () => void;
  channel: Channel;
}

const buildApiPath = (path: string): string => {
  if (path.startsWith('http://') || path.startsWith('https://')) {
    return path;
  }
  return path.startsWith('/') ? path : `/${path}`;
};

const resolveApiOrigin = (): string => {
  const configuredBase = apiClient.defaults.baseURL;
  if (typeof configuredBase === 'string' && /^https?:\/\//.test(configuredBase)) {
    return new URL(configuredBase).origin;
  }

  const envBase = import.meta.env.VITE_API_BASE_URL;
  if (typeof envBase === 'string' && /^https?:\/\//.test(envBase)) {
    return new URL(envBase).origin;
  }

  return window.location.origin;
};

const buildApiUrl = (path: string): string => {
  const normalizedPath = buildApiPath(path);
  return new URL(normalizedPath, resolveApiOrigin()).toString();
};

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export const PlayerModal: React.FC<PlayerModalProps> = ({
  isOpen,
  onClose,
  channel,
}) => {
  const { id: channelId, name: channelName, url: channelUrl, source_visibility, playback_strategy } = channel;
  const videoRef = useRef<HTMLVideoElement>(null);
  const hlsRef = useRef<Hls | null>(null);
  const sessionIdRef = useRef<string | null>(null);
  const hlsUrlRef = useRef<string | null>(null); // 保存转码后的 HLS URL
  const recoveryAttemptRef = useRef({ media: 0, network: 0 });
  const playAttemptRef = useRef(0);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [transcoding, setTranscoding] = useState(false);
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
      if (!data.fatal) {
        return;
      }

      console.error('HLS Error:', data);

      if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
        if (recoveryAttemptRef.current.media < 2) {
          recoveryAttemptRef.current.media += 1;
          console.warn(`Recovering media error (${recoveryAttemptRef.current.media}/2)...`);
          hls.recoverMediaError();
          return;
        }
      }

      if (data.type === Hls.ErrorTypes.NETWORK_ERROR) {
        if (recoveryAttemptRef.current.network < 2) {
          recoveryAttemptRef.current.network += 1;
          console.warn(`Recovering network error (${recoveryAttemptRef.current.network}/2)...`);
          hls.stopLoad();
          setTimeout(() => {
            hls.loadSource(sourceUrl);
            hls.startLoad();
          }, 500);
          return;
        }
      }

      onFatal(`播放失败: ${data.type} - ${data.details}`);
    });
  }, []);

  // 关闭时清理
  useEffect(() => {
    return () => {
      cleanupHls();
      cleanupTranscode();
    };
  }, [cleanupHls, cleanupTranscode]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !isOpen) {
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
        ? `视频解码失败（code=${mediaError.code}）`
        : '视频加载失败';
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
  }, [isOpen, requestVideoPlay]);

  // 播放 UDP 流（需要转码）
  const playUDPStream = useCallback(async () => {
    const attemptId = ++playAttemptRef.current;
    const isCurrentAttempt = () => playAttemptRef.current === attemptId;

    setTranscoding(true);
    setLoading(true);
    setError(null);

    try {
      // 先停止之前的转码会话
      await cleanupTranscode();
      recoveryAttemptRef.current = { media: 0, network: 0 };

      console.log('Starting transcode for channel:', channelId);
      const result = await startTranscode(channelId);
      if (!isCurrentAttempt()) {
        await stopTranscode(result.session_id).catch(() => {});
        return;
      }
      sessionIdRef.current = result.session_id;

      const hlsUrl = buildApiUrl(result.playlist_url);
      hlsUrlRef.current = hlsUrl; // 保存 HLS URL 供外部播放器使用
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
        } catch (e) {
          console.log('HLS file check failed, retrying...');
        }
      }

      if (!verified) {
        throw new Error('HLS 文件生成超时，请稍后重试；若源是内网组播/网关流，服务端可能仍在等待稳定关键帧');
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
          // 缓冲配置 - 允许播放器先囤几片，减少“每切下一片就转圈”。
          maxBufferLength: 45,
          maxMaxBufferLength: 90,
          maxBufferSize: 80 * 1000 * 1000,
          maxBufferHole: 0.8,
          liveDurationInfinity: true,
          backBufferLength: 30,
          liveBackBufferLength: 30,
          startLevel: -1,
          autoStartLoad: true,
          liveSyncDurationCount: 3,
          liveMaxLatencyDurationCount: 8,
          startFragPrefetch: true,
          manifestLoadingTimeOut: 10000,
          manifestLoadingMaxRetry: 6,
          levelLoadingTimeOut: 10000,
          levelLoadingMaxRetry: 6,
          fragLoadingTimeOut: 20000,
          fragLoadingMaxRetry: 6,
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
          await tryStartPlayback(video, 0.1);
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
      setError(e instanceof Error ? e.message : '转码启动失败，请确保服务器已安装 FFmpeg');
      setLoading(false);
      setTranscoding(false);
    }
  }, [channelId, cleanupTranscode]);

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
        maxBufferLength: 30,
        maxMaxBufferLength: 60,
        maxBufferHole: 0.8,
        liveSyncDurationCount: 3,
        liveMaxLatencyDurationCount: 8,
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
        await tryStartPlayback(video, 0.1);
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
      setError('视频格式不支持或加载失败');
      setLoading(false);
    });
  }, [channelId]);

  // 初始化播放
  useEffect(() => {
    if (isOpen && channelUrl) {
      // 重置状态
      cleanupHls();
      setError(null);
      setLoading(true);
      setTranscoding(false);
      recoveryAttemptRef.current = { media: 0, network: 0 };
      playbackStartedRef.current = false;

      const isHLS = channelUrl.includes('.m3u8') || channelUrl.includes('m3u8');
      const isUDP = channelUrl.includes('/udp/');

      const mustUseHls =
        playback_strategy === 'hls_only' ||
        source_visibility === 'private_server_only' ||
        isUDP;

      if (playback_strategy === 'record_only') {
        setError('该频道当前设置为仅允许录制，不提供在线预览。');
        setLoading(false);
        return;
      }

      if (mustUseHls) {
        playUDPStream();
      } else if (isHLS || playback_strategy === 'proxy_only') {
        playHLSStream();
      } else {
        playOtherStream();
      }
    }

    return () => {
      cleanupHls();
    };
  }, [isOpen, channelUrl, cleanupHls, playUDPStream, playHLSStream, playOtherStream, playback_strategy, source_visibility]);

  // 关闭时停止转码
  const handleClose = useCallback(async () => {
    cleanupHls();
    await cleanupTranscode();
    onClose();
  }, [cleanupHls, cleanupTranscode, onClose]);

  const handleOpenExternal = () => {
    const token = getStoredAuthToken();
    const serverStreamUrl = buildApiUrl(`/api/channels/${channelId}/stream${
      token ? `?token=${encodeURIComponent(token)}` : ''
    }`);
    // 对于 UDP 流，使用转码后的 HLS 地址
    // 对于其他流，使用原始地址
    const urlToOpen = hlsUrlRef.current
      || (source_visibility === 'private_server_only' ? serverStreamUrl : channelUrl);
    window.open(urlToOpen, '_blank');
  };

  const handleCopyUrl = () => {
    const token = getStoredAuthToken();
    const urlToCopy = source_visibility === 'private_server_only'
      ? buildApiUrl(`/api/channels/${channelId}/stream${token ? `?token=${encodeURIComponent(token)}` : ''}`)
      : channelUrl;
    navigator.clipboard.writeText(urlToCopy).then(() => {
      alert('流地址已复制到剪贴板');
    });
  };

  const handleVideoClick = (e: React.MouseEvent) => {
    e.stopPropagation();
  };

  if (!isOpen) return null;

  return (
    <div className="player-modal-overlay" onClick={handleClose}>
      <div className="player-modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="player-modal-header">
          <h2>{channelName}</h2>
          <div className="player-modal-actions">
            <button
              className="btn btn-ghost btn-sm"
              onClick={handleCopyUrl}
              title="复制流地址"
            >
              复制地址
            </button>
            <button
              className="btn btn-ghost btn-sm"
              onClick={handleOpenExternal}
              title="在外部播放器打开"
            >
              <ExternalLink size={16} />
            </button>
            <button className="modal-close" onClick={handleClose}>
              <X size={20} />
            </button>
          </div>
        </div>
        <div className="player-modal-body">
          <div className="player-video-container">
            {source_visibility === 'private_server_only' && !error && (
              <div className="player-warning-banner">
                私有源正在通过服务器中转播放，外网预览会占用服务器出口带宽。
              </div>
            )}
            {(loading || transcoding) && !error && (
              <div className="player-loading">
                <Loader2 size={48} className="animate-spin" />
                <span>{transcoding ? '正在转码...' : '加载中...'}</span>
              </div>
            )}
            {error && (
              <div className="player-error">
                <AlertCircle size={48} />
                <span className="error-message">{error}</span>
                <button
                  className="btn btn-primary btn-sm"
                  onClick={() => {
                    setError(null);
                    setLoading(true);
                    const isUDP = channelUrl.includes('/udp/');
                    const mustUseHls =
                      playback_strategy === 'hls_only'
                      || source_visibility === 'private_server_only'
                      || isUDP;

                    if (mustUseHls) {
                      playUDPStream();
                    } else if (channelUrl.includes('.m3u8') || channelUrl.includes('m3u8') || playback_strategy === 'proxy_only') {
                      playHLSStream();
                    } else {
                      playOtherStream();
                    }
                  }}
                >
                  重试
                </button>
              </div>
            )}
            <video
              ref={videoRef}
              className="player-video"
              controls
              autoPlay
              muted
              playsInline
              onClick={handleVideoClick}
              style={{ display: error ? 'none' : 'block' }}
            />
          </div>
          <div className="player-url">
            <span>流地址：</span>
            <code>{channelUrl}</code>
          </div>
        </div>
      </div>
    </div>
  );
};

export default PlayerModal;
