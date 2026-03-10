import { create } from 'zustand';
import type { Channel } from '@/types';

interface ChannelState {
  channels: Channel[];
  selectedChannelIds: string[];
  loading: boolean;
  setChannels: (channels: Channel[]) => void;
  setSelectedChannelIds: (ids: string[]) => void;
  addChannel: (channel: Channel) => void;
  updateChannel: (id: string, channel: Partial<Channel>) => void;
  removeChannel: (id: string) => void;
  setLoading: (loading: boolean) => void;
}

export const useChannelStore = create<ChannelState>((set) => ({
  channels: [],
  selectedChannelIds: [],
  loading: false,

  setChannels: (channels) => set({ channels }),

  setSelectedChannelIds: (ids) => set({ selectedChannelIds: ids }),

  addChannel: (channel) =>
    set((state) => ({ channels: [...state.channels, channel] })),

  updateChannel: (id, updatedChannel) =>
    set((state) => ({
      channels: state.channels.map((ch) =>
        ch.id === id ? { ...ch, ...updatedChannel } : ch
      ),
    })),

  removeChannel: (id) =>
    set((state) => ({
      channels: state.channels.filter((ch) => ch.id !== id),
    })),

  setLoading: (loading) => set({ loading }),
}));
