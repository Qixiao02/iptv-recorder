import React, { useState, useEffect } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { createChannel, updateChannel } from '@/api/channels';
import { X, Loader2 } from 'lucide-react';
import type { Channel, CreateChannelRequest } from '@/types';
import './Modal.css';

interface ChannelModalProps {
  isOpen: boolean;
  onClose: () => void;
  channel?: Channel | null; // 如果有值则为编辑模式
}

export const ChannelModal: React.FC<ChannelModalProps> = ({ isOpen, onClose, channel }) => {
  const queryClient = useQueryClient();
  const isEdit = !!channel;

  const [form, setForm] = useState<CreateChannelRequest>({
    name: '',
    url: '',
    group_name: '',
    logo_url: '',
  });

  useEffect(() => {
    if (channel) {
      setForm({
        name: channel.name,
        url: channel.url,
        group_name: channel.group_name,
        logo_url: channel.logo_url || '',
      });
    } else {
      setForm({ name: '', url: '', group_name: '', logo_url: '' });
    }
  }, [channel]);

  const createMutation = useMutation({
    mutationFn: createChannel,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['channels'] });
      handleClose();
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: CreateChannelRequest }) =>
      updateChannel(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['channels'] });
      handleClose();
    },
  });

  const isLoading = createMutation.isPending || updateMutation.isPending;

  const handleSubmit = () => {
    if (!form.name.trim() || !form.url.trim()) return;

    if (isEdit && channel) {
      updateMutation.mutate({ id: channel.id, data: form });
    } else {
      createMutation.mutate(form);
    }
  };

  const handleClose = () => {
    setForm({ name: '', url: '', group_name: '', logo_url: '' });
    onClose();
  };

  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={handleClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>{isEdit ? '编辑频道' : '新建频道'}</h2>
          <button className="modal-close" onClick={handleClose}>
            <X size={20} />
          </button>
        </div>

        <div className="modal-body">
          <div className="form-group">
            <label>频道名称 *</label>
            <input
              type="text"
              className="input"
              placeholder="例如：CCTV-1"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
          </div>

          <div className="form-group">
            <label>流地址 *</label>
            <input
              type="text"
              className="input"
              placeholder="https://example.com/stream.m3u8"
              value={form.url}
              onChange={(e) => setForm({ ...form, url: e.target.value })}
            />
          </div>

          <div className="form-group">
            <label>分组名称</label>
            <input
              type="text"
              className="input"
              placeholder="例如：央视"
              value={form.group_name}
              onChange={(e) => setForm({ ...form, group_name: e.target.value })}
            />
          </div>

          <div className="form-group">
            <label>Logo URL</label>
            <input
              type="text"
              className="input"
              placeholder="https://example.com/logo.png"
              value={form.logo_url}
              onChange={(e) => setForm({ ...form, logo_url: e.target.value })}
            />
          </div>
        </div>

        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={handleClose}>
            取消
          </button>
          <button
            className="btn btn-primary"
            onClick={handleSubmit}
            disabled={isLoading || !form.name.trim() || !form.url.trim()}
          >
            {isLoading ? (
              <>
                <Loader2 size={16} className="animate-spin" />
                保存中...
              </>
            ) : (
              isEdit ? '保存' : '创建'
            )}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ChannelModal;
