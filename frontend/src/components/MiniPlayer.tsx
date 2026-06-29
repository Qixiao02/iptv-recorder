import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { X, PictureInPicture2, AlertCircle, Loader2, Copy } from 'lucide-react';
import { usePlayerStore } from '@/stores/playerStore';
import { getStoredAuthToken } from '@/stores/authStore';
import { toast } from '@/stores/toastStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { usePlayerCore } from './usePlayerCore';
import './PlayerModal.css';

/**
 * 悬浮迷你播放器：固定在屏幕右下角，不遮挡背景页面。
 *
 * 与 PlayerModal（全屏大窗）共享 usePlayerCore 播放逻辑，仅外壳不同。
 * 通过 playerStore 全局状态驱动，跨路由保持播放（切到任务页/设置页小窗仍在）。
 *
 * 设计要点：
 * - 没有 overlay 遮罩：背后页面完全可点击、可操作。
 * - position: fixed 右下角，z-index 高于内容但低于 modal（900 < 1100）。
 * - 鼠标悬停显示控制条（标题/复制/PiP/关闭），移开隐藏以节省空间。
 * - 一次只播一个频道（playerStore.channel 单值）。
 */
export const MiniPlayer: React.FC = () => {
  const { t } = useTranslation(['components']);
  useI18nNamespace('components');
  const channel = usePlayerStore((s) => s.channel);
  const closePlayer = usePlayerStore((s) => s.closePlayer);
  const [hovered, setHovered] = useState(false);

  const {
    videoRef,
    hlsUrlRef,
    error,
    loading,
    transcoding,
    recordingActive,
    retry,
    cleanupHls,
    cleanupTranscode,
  } = usePlayerCore({ channel, active: !!channel });

  if (!channel) return null;

  const { name: channelName, url: channelUrl, source_visibility } = channel;

  const handleClose = async () => {
    cleanupHls();
    await cleanupTranscode();
    closePlayer();
  };

  const handleTogglePiP = async () => {
    const video = videoRef.current;
    if (!video || typeof document === 'undefined' || !document.pictureInPictureEnabled) {
      toast.error(t('components:player.pipUnsupported'));
      return;
    }
    if (document.pictureInPictureElement) {
      try {
        await document.exitPictureInPicture();
      } catch {
        /* 忽略 */
      }
      return;
    }
    try {
      if (video.paused) {
        await video.play().catch(() => {});
      }
      await video.requestPictureInPicture();
    } catch (e) {
      console.error('Failed to enter PiP:', e);
      toast.error(t('components:player.pipFailed'));
    }
  };

  const handleCopyUrl = () => {
    const token = getStoredAuthToken();
    const serverStreamUrl = `/api/channels/${channel.id}/stream${
      token ? `?token=${encodeURIComponent(token)}` : ''
    }`;
    const urlToCopy = hlsUrlRef.current
      || (source_visibility === 'private_server_only' ? serverStreamUrl : channelUrl);
    navigator.clipboard.writeText(urlToCopy).then(() => {
      toast.success(t('components:player.copied'));
    });
  };

  return (
    <div
      className="mini-player"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <video
        ref={videoRef}
        className="mini-player-video"
        controls
        autoPlay
        muted
        playsInline
        style={{ display: error ? 'none' : 'block' }}
      />

      {/* 加载/转码指示 */}
      {(loading || transcoding) && !error && (
        <div className="mini-player-loading">
          <Loader2 size={24} className="animate-spin" />
          <span>{transcoding ? t('components:player.transcoding') : t('components:player.loading')}</span>
        </div>
      )}

      {/* 错误态 */}
      {error && (
        <div className="mini-player-error">
          <AlertCircle size={24} />
          <span className="mini-player-error-msg">{error}</span>
          <button className="mini-player-retry" onClick={retry}>
            {t('components:player.retry')}
          </button>
        </div>
      )}

      {/* 悬停控制条：标题 + 操作按钮 */}
      <div className={`mini-player-bar ${hovered || error ? 'visible' : ''}`}>
        <span className="mini-player-title" title={channelName}>{channelName}</span>
        <div className="mini-player-actions">
          <button className="mini-player-btn" onClick={handleCopyUrl} title={t('components:player.copyTitle')} aria-label={t('components:player.copyTitle')}>
            <Copy size={14} />
          </button>
          <button className="mini-player-btn" onClick={handleTogglePiP} title={t('components:player.pipTitle')} aria-label={t('components:player.pipTitle')}>
            <PictureInPicture2 size={14} />
          </button>
          <button className="mini-player-btn close" onClick={handleClose} title={t('common:close', { defaultValue: '关闭' })} aria-label={t('common:close', { defaultValue: '关闭' })}>
            <X size={14} />
          </button>
        </div>
      </div>

      {/* 录制中提示（不阻断，仅角落标记） */}
      {recordingActive && !error && (
        <div className="mini-player-recording-dot" title={t('components:player.recordingActiveWarning')} />
      )}
    </div>
  );
};

export default MiniPlayer;
