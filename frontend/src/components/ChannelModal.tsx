import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { createChannel, updateChannel } from '@/api/channels';
import { toast } from '@/stores/toastStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { channelKeys } from '@/lib/queryKeys';
import { X, Loader2 } from 'lucide-react';
import type { Channel, CreateChannelRequest } from '@/types';
import './Modal.css';

interface ChannelModalProps {
  isOpen: boolean;
  onClose: () => void;
  channel?: Channel | null;
}

const emptyForm: CreateChannelRequest = {
  name: '',
  url: '',
  group_name: '',
  logo_url: '',
  source_visibility: 'public',
  playback_strategy: 'auto',
};

export const ChannelModal: React.FC<ChannelModalProps> = ({ isOpen, onClose, channel }) => {
  const { t } = useTranslation(['components', 'common']);
  useI18nNamespace(['components', 'common']);
  const queryClient = useQueryClient();
  const isEdit = !!channel;
  const [form, setForm] = useState<CreateChannelRequest>(emptyForm);

  useEffect(() => {
    queueMicrotask(() => {
      if (channel) {
        setForm({
          name: channel.name,
          url: channel.url,
          group_name: channel.group_name,
          logo_url: channel.logo_url || '',
          source_visibility: channel.source_visibility,
          playback_strategy: channel.playback_strategy,
        });
      } else {
        setForm(emptyForm);
      }
    });
  }, [channel]);

  const createMutation = useMutation({
    mutationFn: createChannel,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: channelKeys.root });
      toast.success(t('common:toast.channelCreated'));
      handleClose();
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: CreateChannelRequest }) => updateChannel(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: channelKeys.root });
      toast.success(t('common:toast.channelUpdated'));
      handleClose();
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
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
    setForm(emptyForm);
    onClose();
  };

  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={handleClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>{isEdit ? t('components:channelModal.editTitle') : t('components:channelModal.createTitle')}</h2>
          <button className="modal-close" onClick={handleClose}>
            <X size={20} />
          </button>
        </div>

        <div className="modal-body">
          <div className="form-group">
            <label>{t('components:channelModal.name')}</label>
            <input
              type="text"
              className="input"
              placeholder={t('components:channelModal.namePlaceholder')}
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
          </div>

          <div className="form-group">
            <label>{t('components:channelModal.url')}</label>
            <input
              type="text"
              className="input"
              placeholder="https://example.com/stream.m3u8"
              value={form.url}
              onChange={(e) => setForm({ ...form, url: e.target.value })}
            />
          </div>

          <div className="form-group">
            <label>{t('components:channelModal.group')}</label>
            <input
              type="text"
              className="input"
              placeholder={t('components:channelModal.groupPlaceholder')}
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

          <div className="form-row">
            <div className="form-group">
              <label>{t('components:channelModal.sourceVisibility')}</label>
              <select
                className="input"
                value={form.source_visibility}
                onChange={(e) => setForm({
                  ...form,
                  source_visibility: e.target.value as NonNullable<CreateChannelRequest['source_visibility']>,
                })}
              >
                <option value="public">{t('components:channelModal.sourcePublic')}</option>
                <option value="private_server_only">{t('components:channelModal.sourcePrivate')}</option>
              </select>
            </div>

            <div className="form-group">
              <label>{t('components:channelModal.playbackStrategy')}</label>
              <select
                className="input"
                value={form.playback_strategy}
                onChange={(e) => setForm({
                  ...form,
                  playback_strategy: e.target.value as NonNullable<CreateChannelRequest['playback_strategy']>,
                })}
              >
                <option value="auto">{t('components:channelModal.playbackAuto')}</option>
                <option value="hls_only">{t('components:channelModal.playbackHls')}</option>
                <option value="proxy_only">{t('components:channelModal.playbackProxy')}</option>
                <option value="record_only">{t('components:channelModal.playbackRecord')}</option>
              </select>
            </div>
          </div>

          {form.source_visibility === 'private_server_only' && (
            <div className="form-hint">
              {t('components:channelModal.privateHint')}
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={handleClose}>
            {t('common:cancel')}
          </button>
          <button
            className="btn btn-primary"
            onClick={handleSubmit}
            disabled={isLoading || !form.name.trim() || !form.url.trim()}
          >
            {isLoading ? (
              <>
                <Loader2 size={16} className="animate-spin" />
                {t('components:channelModal.saving')}
              </>
            ) : (
              isEdit ? t('components:channelModal.save') : t('components:channelModal.create')
            )}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ChannelModal;
