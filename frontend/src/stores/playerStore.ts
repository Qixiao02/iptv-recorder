import { create } from 'zustand';
import type { Channel } from '@/types';

/**
 * 全局播放器状态。
 *
 * 大窗（large）↔ 小窗（mini）两种模式，由 mode 字段驱动：
 * - openPlayer: 设频道 + mode='large'（默认大窗，先大窗后小窗）
 * - minimize:   large → mini（大窗缩小为悬浮小窗，视频流不中断）
 * - restore:    mini → large（小窗还原为大窗）
 * - closePlayer: 清空频道 + mode=null
 *
 * 小窗位置/大小用 localStorage 持久化，下次打开还在上次位置。
 */

export type PlayerMode = 'large' | 'mini' | null;

export interface PlayerPosition {
  x: number;
  y: number;
}

export interface PlayerSize {
  /** 小窗宽度(px)，高度按 16:9 由 CSS 计算 */
  width: number;
}

// ===== localStorage 持久化辅助 =====
const STORAGE_KEY = 'iptv-player-mini-state';

interface PersistedShape {
  position: PlayerPosition;
  size: PlayerSize;
}

// 默认小窗位置：右下角（用负数/特殊值标记"右下角"，渲染时换算成实际 px）
// 这里用 null 表示"未设置过位置"，渲染时回退到右下角默认值。
const DEFAULT_SIZE: PlayerSize = { width: 320 };

function loadPersisted(): Partial<PersistedShape> {
  if (typeof window === 'undefined') return {};
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as PersistedShape;
    return {
      position:
        parsed.position
        && typeof parsed.position.x === 'number'
        && typeof parsed.position.y === 'number'
          ? parsed.position
          : undefined,
      size:
        parsed.size && typeof parsed.size.width === 'number'
          ? parsed.size
          : undefined,
    };
  } catch {
    return {};
  }
}

function persist(state: { position: PlayerPosition | null; size: PlayerSize }) {
  if (typeof window === 'undefined') return;
  try {
    const toSave: PersistedShape = {
      // position 为 null（默认右下角）时不存具体坐标，下次仍走默认右下角
      position: state.position ?? { x: -1, y: -1 },
      size: state.size,
    };
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(toSave));
  } catch {
    /* localStorage 不可用时静默忽略 */
  }
}

const persisted = loadPersisted();

interface PlayerState {
  /** 当前正在播放的频道；为 null 时播放器不显示 */
  channel: Channel | null;
  /** 播放器模式：large=全屏大窗, mini=悬浮小窗, null=未打开 */
  mode: PlayerMode;
  /** 小窗左上角位置(px, 相对视口)；null=使用默认右下角 */
  position: PlayerPosition | null;
  /** 小窗宽度(px) */
  size: PlayerSize;
  /** 打开播放(默认大窗) */
  openPlayer: (channel: Channel) => void;
  /** 最小化为小窗(large → mini) */
  minimize: () => void;
  /** 还原为大窗(mini → large) */
  restore: () => void;
  /** 关闭播放 */
  closePlayer: () => void;
  /** 更新小窗位置(并持久化) */
  setPosition: (pos: PlayerPosition) => void;
  /** 重置为默认右下角位置(并持久化) */
  resetPosition: () => void;
  /** 更新小窗宽度(并持久化) */
  setSize: (size: PlayerSize) => void;
}

export const usePlayerStore = create<PlayerState>((set) => ({
  channel: null,
  mode: null,
  position: persisted.position ?? null,
  size: persisted.size ?? DEFAULT_SIZE,
  openPlayer: (channel) => set({ channel, mode: 'large' }),
  minimize: () => set({ mode: 'mini' }),
  restore: () => set({ mode: 'large' }),
  closePlayer: () => set({ channel: null, mode: null }),
  setPosition: (position) => {
    persist({ position, size: usePlayerStore.getState().size });
    set({ position });
  },
  resetPosition: () => {
    persist({ position: null, size: usePlayerStore.getState().size });
    set({ position: null });
  },
  setSize: (size) => {
    persist({ position: usePlayerStore.getState().position, size });
    set({ size });
  },
}));
