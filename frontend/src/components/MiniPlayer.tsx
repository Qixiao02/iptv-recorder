import React, { useState, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  X,
  PictureInPicture2,
  AlertCircle,
  Loader2,
  Copy,
  Minimize2,
  Maximize2,
  Radio,
} from 'lucide-react';
import { usePlayerStore } from '@/stores/playerStore';
import { getStoredAuthToken } from '@/stores/authStore';
import { toast } from '@/stores/toastStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { usePlayerCore } from './usePlayerCore';
import { useDraggable } from '@/hooks/useDraggable';
import { useResizable } from '@/hooks/useResizable';
import './PlayerModal.css';

/**
 * 统一播放器：大窗(large)↔ 小窗(mini)两种模式，由 playerStore.mode 驱动。
 *
 * 关键架构：大窗和小窗是【同一个组件的两种 CSS 表现】，共用同一个
 * <video ref={videoRef}> 节点。切换大小窗时 video 节点不变 → 视频流不中断。
 * (若用条件渲染两个组件，各自一个 video，切换时会销毁重建导致重连流)
 *
 * 交互(YouTube/B 站式)：
 * - 点"播放" → 默认大窗(mode='large')
 * - 大窗"最小化" → 缩为右下角小窗(mode='mini')，流不中断
 * - 小窗：可拖拽(标题栏)、可 resize(右下角)、「还原」回大窗
 * - 小窗位置/大小 localStorage 记忆
 */
export const MiniPlayer: React.FC = () => {
  const { t } = useTranslation(['components']);
  useI18nNamespace('components');
  const channel = usePlayerStore((s) => s.channel);
  const mode = usePlayerStore((s) => s.mode);
  const position = usePlayerStore((s) => s.position);
  const size = usePlayerStore((s) => s.size);
  const closePlayer = usePlayerStore((s) => s.closePlayer);
  const minimize = usePlayerStore((s) => s.minimize);
  const restore = usePlayerStore((s) => s.restore);
  const setPosition = usePlayerStore((s) => s.setPosition);
  const setSize = usePlayerStore((s) => s.setSize);
  const [hovered, setHovered] = useState(false);

  // 小窗元素 ref，供拖拽/resize 读取尺寸
  const miniRef = useRef<HTMLDivElement>(null);

  // active 只看 channel 是否存在，不看 mode——大小窗都保持播放
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

  // ===== 拖拽(仅小窗启用) =====
  const getSize = useCallback(
    () => {
      const el = miniRef.current;
      if (el) {
        const r = el.getBoundingClientRect();
        return { width: r.width, height: r.height };
      }
      // 16:9 估算
      return { width: size.width, height: (size.width * 9) / 16 };
    },
    [size.width],
  );
  const { onDragStart } = useDraggable(setPosition, getSize, mode === 'mini');

  // ===== resize(仅小窗启用) =====
  const { onResizeStart } = useResizable(
    (width) => setSize({ width }),
    { minWidth: 240, maxWidth: 720 },
    mode === 'mini',
  );

  if (!channel || !mode) return null;

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

  // ===== 小窗样式：位置 + 尺寸 =====
  // position 为 null(默认)时用右下角定位
  const miniStyle: React.CSSProperties = position
    ? {
        left: position.x,
        top: position.y,
        width: size.width,
        height: (size.width * 9) / 16,
        right: 'auto',
        bottom: 'auto',
      }
    : {
        width: size.width,
        height: (size.width * 9) / 16,
      };

  // 公共操作按钮(复制/PiP)，大小窗都用
  const renderCommonActions = () => (
    <>
      <button
        className="player-action-btn"
        onClick={handleCopyUrl}
        title={t('components:player.copyTitle')}
        aria-label={t('components:player.copyTitle')}
      >
        <Copy size={14} />
      </button>
      <button
        className="player-action-btn"
        onClick={handleTogglePiP}
        title={t('components:player.pipTitle')}
        aria-label={t('components:player.pipTitle')}
      >
        <PictureInPicture2 size={14} />
      </button>
    </>
  );

  // ============ 大窗模式 ============
  if (mode === 'large') {
    return (
      <div className="player-modal-overlay" onClick={handleClose}>
        <div className="player-modal-content" onClick={(e) => e.stopPropagation()}>
          <div className="player-modal-header">
            <h2>{channelName}</h2>
            <div className="player-modal-actions">
              <button
                className="btn btn-ghost btn-sm"
                onClick={handleCopyUrl}
                title={t('components:player.copyTitle')}
              >
                {t('components:player.copyAddress')}
              </button>
              <button
                className="btn btn-ghost btn-sm"
                onClick={handleTogglePiP}
                title={t('components:player.pipTitle')}
                aria-label={t('components:player.pipTitle')}
              >
                <PictureInPicture2 size={16} />
              </button>
              {/* 最小化为悬浮小窗(流不中断) */}
              <button
                className="btn btn-ghost btn-sm"
                onClick={() => minimize()}
                title={t('components:player.minimize')}
                aria-label={t('components:player.minimize')}
              >
                <Minimize2 size={16} />
              </button>
              <button className="modal-close" onClick={handleClose} aria-label={t('common:close', { defaultValue: '关闭' })}>
                <X size={20} />
              </button>
            </div>
          </div>
          <div className="player-modal-body">
            <div className="player-video-container">
              {/* 信息提示:收成右上角小图标 badge,悬停看详情,不持续遮挡视频 */}
              {(source_visibility === 'private_server_only' || recordingActive) && !error && (
                <div className="player-info-badges">
                  {source_visibility === 'private_server_only' && (
                    <span
                      className="player-info-badge player-info-badge-warn"
                      role="img"
                      tabIndex={0}
                      aria-label={t('components:player.privateRelayWarning')}
                    >
                      <Radio size={13} />
                      <span className="player-info-badge-tip">{t('components:player.privateRelayWarning')}</span>
                    </span>
                  )}
                  {recordingActive && (
                    <span
                      className="player-info-badge player-info-badge-recording"
                      role="img"
                      tabIndex={0}
                      aria-label={t('components:player.recordingActiveWarning')}
                    >
                      <span className="player-info-badge-rec-dot" />
                      <span className="player-info-badge-tip">{t('components:player.recordingActiveWarning')}</span>
                    </span>
                  )}
                </div>
              )}
              {(loading || transcoding) && !error && (
                <div className="player-loading">
                  <Loader2 size={48} className="animate-spin" />
                  <span>{transcoding ? t('components:player.transcoding') : t('components:player.loading')}</span>
                </div>
              )}
              {error && (
                <div className="player-error">
                  <AlertCircle size={48} />
                  <span className="error-message">{error}</span>
                  <button className="btn btn-primary btn-sm" onClick={retry}>
                    {t('components:player.retry')}
                  </button>
                </div>
              )}
              {/* 同一个 video 节点：大窗/小窗切换时不变，流不中断 */}
              <video
                ref={videoRef}
                className="player-video"
                controls
                autoPlay
                muted
                playsInline
                onClick={(e) => e.stopPropagation()}
                style={{ display: error ? 'none' : 'block' }}
              />
            </div>
            <div className="player-url">
              <span>{t('components:player.streamUrl')}</span>
              <code>{channelUrl}</code>
            </div>
          </div>
        </div>
      </div>
    );
  }

  // ============ 小窗模式 ============
  return (
    <div
      ref={miniRef}
      className="mini-player"
      style={miniStyle}
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

      {/* 顶部拖拽把手 + 控制条(悬停显示) */}
      <div
        className={`mini-player-bar ${hovered || error ? 'visible' : ''}`}
        onMouseDown={onDragStart}
        title={t('components:player.dragHint', { defaultValue: '拖动移动' })}
      >
        <span className="mini-player-title" title={channelName}>{channelName}</span>
        <div className="mini-player-actions" onMouseDown={(e) => e.stopPropagation()}>
          {renderCommonActions()}
          {/* 还原为大窗 */}
          <button
            className="player-action-btn"
            onClick={() => restore()}
            title={t('components:player.restore')}
            aria-label={t('components:player.restore')}
          >
            <Maximize2 size={14} />
          </button>
          <button
            className="player-action-btn close"
            onClick={handleClose}
            title={t('common:close', { defaultValue: '关闭' })}
            aria-label={t('common:close', { defaultValue: '关闭' })}
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* 右下角 resize 把手 */}
      <div
        className="mini-player-resize-handle"
        onMouseDown={onResizeStart}
      />

      {/* 信息提示:右上角小图标 badge,悬停看详情(小窗空间小,用极简样式) */}
      {(source_visibility === 'private_server_only' || recordingActive) && !error && (
        <div className="mini-player-badges">
          {source_visibility === 'private_server_only' && (
            <span
              className="player-info-badge player-info-badge-warn player-info-badge-mini"
              role="img"
              tabIndex={0}
              aria-label={t('components:player.privateRelayWarning')}
            >
              <Radio size={11} />
              <span className="player-info-badge-tip">{t('components:player.privateRelayWarning')}</span>
            </span>
          )}
          {recordingActive && (
            <span
              className="player-info-badge player-info-badge-recording player-info-badge-mini"
              role="img"
              tabIndex={0}
              aria-label={t('components:player.recordingActiveWarning')}
            >
              <span className="player-info-badge-rec-dot" />
              <span className="player-info-badge-tip">{t('components:player.recordingActiveWarning')}</span>
            </span>
          )}
        </div>
      )}
    </div>
  );
};

export default MiniPlayer;
