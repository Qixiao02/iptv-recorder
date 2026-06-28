import { create } from 'zustand';
import type { Channel } from '@/types';

/**
 * 全局播放器状态。
 *
 * 把播放频道状态从 Channels 页提升到全局 store，让悬浮小窗能跨路由保持——
 * 切到任务页/设置页，小窗依然在播。
 *
 * 一次只播一个频道：openPlayer 会替换当前频道。
 */
interface PlayerState {
  /** 当前正在播放的频道；为 null 时小窗不显示 */
  channel: Channel | null;
  /** 打开播放（替换当前频道） */
  openPlayer: (channel: Channel) => void;
  /** 关闭播放（小窗消失，由组件负责停止流/转码） */
  closePlayer: () => void;
}

export const usePlayerStore = create<PlayerState>((set) => ({
  channel: null,
  openPlayer: (channel) => set({ channel }),
  closePlayer: () => set({ channel: null }),
}));
