import React, { Suspense, lazy, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { batchDeleteChannels, getChannels, getChannelGroups, deleteChannel, testChannel } from '@/api/channels';
import { toast } from '@/stores/toastStore';
import { usePlayerStore } from '@/stores/playerStore';
import { useI18nNamespace } from '@/i18n/useI18nNamespace';
import { channelKeys } from '@/lib/queryKeys';
import {
  Search,
  Plus,
  LayoutGrid,
  List,
  Filter,
  Upload,
  CirclePlay,
  Pencil,
  Trash2,
  CircleCheck,
  CircleX,
  Tv,
  CalendarDays,
  Loader2,
  Zap,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react';
import type { Channel } from '@/types';
import './Channels.css';

const ImportM3UModal = lazy(() => import('@/components/ImportM3UModal'));
const ChannelModal = lazy(() => import('@/components/ChannelModal'));
const EpgImportModal = lazy(() => import('@/components/EpgImportModal'));
const EpgProgramsModal = lazy(() => import('@/components/EpgProgramsModal'));
const ConfirmDialog = lazy(() => import('@/components/ConfirmDialog'));

type ViewMode = 'table' | 'card';

export const Channels: React.FC = () => {
  const { t } = useTranslation(['channels', 'common']);
  const isI18nReady = useI18nNamespace(['channels', 'common']);
  const queryClient = useQueryClient();
  const [viewMode, setViewMode] = useState<ViewMode>('table');
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedGroup, setSelectedGroup] = useState<string>('all');
  const [selectedSource, setSelectedSource] = useState<string>('all');
  const [selectedChannels, setSelectedChannels] = useState<Set<string>>(new Set());
  const [showImportModal, setShowImportModal] = useState(false);
  const [showChannelModal, setShowChannelModal] = useState(false);
  const [editingChannel, setEditingChannel] = useState<Channel | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [batchTesting, setBatchTesting] = useState(false);
  const [testProgress, setTestProgress] = useState({ current: 0, total: 0 });
  const openPlayer = usePlayerStore((s) => s.openPlayer);
  const [showEpgImportModal, setShowEpgImportModal] = useState(false);
  const [epgChannel, setEpgChannel] = useState<Channel | null>(null);
  const [deletingChannel, setDeletingChannel] = useState<Channel | null>(null);
  const [showBatchDeleteConfirm, setShowBatchDeleteConfirm] = useState(false);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [debouncedSearch, setDebouncedSearch] = useState('');

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(searchQuery);
      setPage(1);
    }, 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  useEffect(() => {
    queueMicrotask(() => setPage(1));
  }, [selectedGroup, selectedSource]);

  const { data: channelsData, isLoading } = useQuery({
    queryKey: channelKeys.list([page, pageSize, selectedGroup, selectedSource, debouncedSearch]),
    queryFn: () => getChannels({
      page,
      page_size: pageSize,
      group: selectedGroup !== 'all' ? selectedGroup : undefined,
      source_visibility: selectedSource !== 'all' ? (selectedSource as 'public' | 'private_server_only') : undefined,
      search: debouncedSearch || undefined,
    }),
  });

  const { data: groups } = useQuery({
    queryKey: channelKeys.groups(),
    queryFn: getChannelGroups,
  });

  const deleteMutation = useMutation({
    mutationFn: deleteChannel,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: channelKeys.root });
      toast.success(t('common:toast.channelDeleted'));
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
    },
  });

  const batchDeleteMutation = useMutation({
    mutationFn: batchDeleteChannels,
    onSuccess: (result) => {
      const count = result?.deleted ?? selectedChannels.size;
      setSelectedChannels(new Set());
      queryClient.invalidateQueries({ queryKey: channelKeys.root });
      toast.success(t('common:toast.channelsBatchDeleted', { count }));
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
    },
  });

  const handleBatchDelete = () => {
    if (selectedChannels.size === 0) return;
    setShowBatchDeleteConfirm(true);
  };

  const handleConfirmBatchDelete = () => {
    batchDeleteMutation.mutate(Array.from(selectedChannels));
    setShowBatchDeleteConfirm(false);
  };

  const handleConfirmChannelDelete = () => {
    if (!deletingChannel) return;
    deleteMutation.mutate(deletingChannel.id);
    setDeletingChannel(null);
  };

  const handleImportCompleted = () => {
    setPage(1);
    setSelectedChannels(new Set());
  };

  const testMutation = useMutation({
    mutationFn: testChannel,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: channelKeys.root });
      toast.success(t('common:toast.channelTestOk'));
      setTestingId(null);
    },
    onError: (error) => {
      toast.error(t('common:toast.operationFailed', { message: (error as Error).message }));
      setTestingId(null);
    },
  });

  const handleBatchTest = async () => {
    const channelsToTest = selectedChannels.size > 0
      ? channelsData?.items?.filter((channel) => selectedChannels.has(channel.id)) || []
      : channelsData?.items || [];

    if (channelsToTest.length === 0) return;

    setBatchTesting(true);
    setTestProgress({ current: 0, total: channelsToTest.length });

    for (let i = 0; i < channelsToTest.length; i++) {
      const channel = channelsToTest[i];
      setTestProgress({ current: i + 1, total: channelsToTest.length });
      try {
        await testChannel(channel.id);
      } catch {
        // Ignore individual test errors and continue testing the rest.
      }
      queryClient.invalidateQueries({ queryKey: channelKeys.root });
    }

    setBatchTesting(false);
    setTestProgress({ current: 0, total: 0 });
  };

  const handlePlay = (channel: Channel) => {
    openPlayer(channel);
  };

  const handleSelectAll = () => {
    if (selectedChannels.size === channelsData?.items?.length) {
      setSelectedChannels(new Set());
    } else {
      setSelectedChannels(new Set(channelsData?.items?.map((channel) => channel.id) || []));
    }
  };

  const handleSelectChannel = (id: string) => {
    const newSet = new Set(selectedChannels);
    if (newSet.has(id)) {
      newSet.delete(id);
    } else {
      newSet.add(id);
    }
    setSelectedChannels(newSet);
  };

  const handleEditChannel = (channel: Channel) => {
    setEditingChannel(channel);
    setShowChannelModal(true);
  };

  const handleOpenEpgPrograms = (channel: Channel) => {
    setEpgChannel(channel);
  };

  const handleCloseChannelModal = () => {
    setShowChannelModal(false);
    setEditingChannel(null);
  };

  const getStatusBadge = (status: Channel['status']) => {
    switch (status) {
      case 'online':
        return <span className="badge badge-success"><CircleCheck size={12} />{t('channels:status.online')}</span>;
      case 'offline':
        return <span className="badge badge-error"><CircleX size={12} />{t('channels:status.offline')}</span>;
      case 'slow':
        return <span className="badge badge-warning"><CircleCheck size={12} />{t('channels:status.slow')}</span>;
      default:
        return <span className="badge badge-neutral">{t('channels:status.unknown')}</span>;
    }
  };

  const getSourceBadge = (channel: Channel) => {
    if (channel.playback_strategy === 'record_only') {
      return <span className="badge badge-neutral">{t('channels:source.recordOnly')}</span>;
    }
    if (channel.source_visibility === 'private_server_only') {
      return <span className="badge badge-warning">{t('channels:source.private')}</span>;
    }
    return <span className="badge badge-success">{t('channels:source.public')}</span>;
  };

  const handlePageChange = (newPage: number) => {
    setPage(newPage);
    setSelectedChannels(new Set());
  };

  const renderPagination = () => {
    if (!channelsData || channelsData.total_pages <= 1) return null;

    const pages = [];
    const maxVisiblePages = 5;
    let startPage = Math.max(1, page - Math.floor(maxVisiblePages / 2));
    const endPage = Math.min(channelsData.total_pages, startPage + maxVisiblePages - 1);

    if (endPage - startPage + 1 < maxVisiblePages) {
      startPage = Math.max(1, endPage - maxVisiblePages + 1);
    }

    for (let i = startPage; i <= endPage; i++) {
      pages.push(i);
    }

    return (
      <div className="pagination">
        <button
          className="pagination-btn"
          onClick={() => handlePageChange(page - 1)}
          disabled={page <= 1}
          aria-label={t('common:previousPage', { defaultValue: '上一页' })}
        >
          <ChevronLeft size={16} />
        </button>

        {startPage > 1 && (
          <>
            <button className="pagination-btn" onClick={() => handlePageChange(1)}>1</button>
            {startPage > 2 && <span className="pagination-ellipsis">...</span>}
          </>
        )}

        {pages.map((pageNumber) => (
          <button
            key={pageNumber}
            className={`pagination-btn ${pageNumber === page ? 'active' : ''}`}
            onClick={() => handlePageChange(pageNumber)}
          >
            {pageNumber}
          </button>
        ))}

        {endPage < channelsData.total_pages && (
          <>
            {endPage < channelsData.total_pages - 1 && <span className="pagination-ellipsis">...</span>}
            <button className="pagination-btn" onClick={() => handlePageChange(channelsData.total_pages)}>
              {channelsData.total_pages}
            </button>
          </>
        )}

        <button
          className="pagination-btn"
          onClick={() => handlePageChange(page + 1)}
          disabled={page >= channelsData.total_pages}
          aria-label={t('common:nextPage', { defaultValue: '下一页' })}
        >
          <ChevronRight size={16} />
        </button>

        <select
          className="pagination-size"
          value={pageSize}
          onChange={(e) => {
            setPageSize(Number(e.target.value));
            setPage(1);
          }}
        >
          {[10, 20, 50, 100].map((size) => (
            <option key={size} value={size}>{t('channels:pageSize', { count: size })}</option>
          ))}
        </select>
      </div>
    );
  };

  if (!isI18nReady) {
    return <div className="page-loading">{t('common:loading')}</div>;
  }

  const shouldRenderImportModal = showImportModal;
  const shouldRenderChannelModal = showChannelModal || editingChannel !== null;
  const shouldRenderEpgImportModal = showEpgImportModal;
  const shouldRenderEpgProgramsModal = epgChannel !== null;

  return (
    <div className="channels-page">
      <div className="page-header">
        <div className="page-title">
          <h1>{t('channels:title')}</h1>
          <p className="page-subtitle">
            {t('channels:subtitle', { count: channelsData?.total || 0 })}
          </p>
        </div>
        <div className="page-actions">
          <button className="btn btn-ghost" onClick={() => setShowImportModal(true)}>
            <Upload size={16} />
            {t('channels:importM3u')}
          </button>
          <button className="btn btn-ghost" onClick={() => setShowEpgImportModal(true)}>
            <CalendarDays size={16} />
            {t('channels:importEpg')}
          </button>
          <button className="btn btn-primary" onClick={() => setShowChannelModal(true)}>
            <Plus size={16} />
            {t('channels:add')}
          </button>
        </div>
      </div>

      <div className="toolbar card">
        <div className="toolbar-left">
          <div className="search-box">
            <Search size={16} />
            <input
              type="text"
              placeholder={t('common:search')}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="input"
            />
          </div>

          <div className="filter-group">
            <Filter size={16} />
            <select
              value={selectedGroup}
              onChange={(e) => setSelectedGroup(e.target.value)}
              className="input"
            >
              <option value="all">{t('channels:allGroups')}</option>
              {groups?.map((group) => (
                <option key={group} value={group}>
                  {group}
                </option>
              ))}
            </select>
          </div>

          <div className="filter-group">
            <select
              value={selectedSource}
              onChange={(e) => setSelectedSource(e.target.value)}
              className="input"
              aria-label={t('channels:sourceFilter')}
            >
              <option value="all">{t('channels:source.all')}</option>
              <option value="public">{t('channels:source.public')}</option>
              <option value="private_server_only">{t('channels:source.private')}</option>
            </select>
          </div>

          <button
            className="btn btn-ghost"
            onClick={handleBatchTest}
            disabled={batchTesting || !channelsData?.items?.length}
          >
            {batchTesting ? (
              <>
                <Loader2 size={16} className="animate-spin" />
                {t('channels:batchTesting', { current: testProgress.current, total: testProgress.total })}
              </>
            ) : (
              <>
                <Zap size={16} />
                {t('channels:batchTest')}
              </>
            )}
          </button>
        </div>

        <div className="toolbar-right">
          <div className="view-toggle">
            <button
              className={`toggle-btn ${viewMode === 'table' ? 'active' : ''}`}
              onClick={() => setViewMode('table')}
              aria-label={t('channels:actions.tableView', { defaultValue: '表格视图' })}
              aria-pressed={viewMode === 'table'}
              title={t('channels:actions.tableView', { defaultValue: '表格视图' })}
            >
              <List size={16} />
            </button>
            <button
              className={`toggle-btn ${viewMode === 'card' ? 'active' : ''}`}
              onClick={() => setViewMode('card')}
              aria-label={t('channels:actions.cardView', { defaultValue: '卡片视图' })}
              aria-pressed={viewMode === 'card'}
              title={t('channels:actions.cardView', { defaultValue: '卡片视图' })}
            >
              <LayoutGrid size={16} />
            </button>
          </div>
        </div>
      </div>

      {selectedChannels.size > 0 && (
        <div className="batch-actions animate-fade-in">
          <span className="batch-count">{t('channels:selectedCount', { count: selectedChannels.size })}</span>
          <div className="batch-buttons">
            <button className="btn btn-ghost btn-sm">{t('channels:batchRecord')}</button>
            <button
              className="btn btn-ghost btn-sm danger"
              onClick={handleBatchDelete}
              disabled={batchDeleteMutation.isPending}
            >
              {t('channels:batchDelete')}
            </button>
          </div>
        </div>
      )}

      {isLoading ? (
        <div className="loading-grid">
          {[1, 2, 3, 4, 5, 6].map((item) => (
            <div key={item} className="skeleton-card animate-shimmer" />
          ))}
        </div>
      ) : viewMode === 'table' ? (
        <div className="table-container card">
          <table className="data-table">
            <thead>
              <tr>
                <th className="col-checkbox">
                  <input
                    type="checkbox"
                    checked={selectedChannels.size === channelsData?.items?.length && channelsData?.items?.length > 0}
                    onChange={handleSelectAll}
                  />
                </th>
                <th className="col-channel">{t('channels:table.channel')}</th>
                <th className="col-url">{t('channels:table.url')}</th>
                <th className="col-group">{t('channels:table.group')}</th>
                <th className="col-status">{t('channels:table.status')}</th>
                <th className="col-actions">{t('channels:table.actions')}</th>
              </tr>
            </thead>
            <tbody>
              {channelsData?.items?.map((channel, index) => (
                <tr
                  key={channel.id}
                  className={`stagger-item ${selectedChannels.has(channel.id) ? 'selected' : ''}`}
                  style={{ animationDelay: `${index * 0.03}s` }}
                >
                  <td className="col-checkbox">
                    <input
                      type="checkbox"
                      checked={selectedChannels.has(channel.id)}
                      onChange={() => handleSelectChannel(channel.id)}
                    />
                  </td>
                  <td className="col-channel">
                    <div className="channel-cell">
                      <div className="channel-logo">
                        {channel.logo_url ? (
                          <img src={channel.logo_url} alt={channel.name} />
                        ) : (
                          <Tv size={18} />
                        )}
                      </div>
                      <span className="channel-name">{channel.name}</span>
                    </div>
                  </td>
                  <td className="col-url">
                    <code className="url-code">{channel.url.slice(0, 40)}...</code>
                  </td>
                  <td className="col-group">
                    <div className="channel-group-stack">
                      <span className="group-tag">{channel.group_name}</span>
                      {getSourceBadge(channel)}
                    </div>
                  </td>
                  <td className="col-status">{getStatusBadge(channel.status)}</td>
                  <td className="col-actions">
                    <div className="actions-cell">
                      <button
                        className="action-btn"
                        title={t('channels:actions.play')}
                        aria-label={t('channels:actions.play')}
                        onClick={() => handlePlay(channel)}
                      >
                        <CirclePlay size={16} />
                      </button>
                      <button
                        className="action-btn"
                        title={t('channels:actions.test')}
                        aria-label={t('channels:actions.test')}
                        onClick={() => {
                          setTestingId(channel.id);
                          testMutation.mutate(channel.id);
                        }}
                        disabled={testingId === channel.id || batchTesting}
                      >
                        {testingId === channel.id ? (
                          <Loader2 size={16} className="animate-spin" />
                        ) : (
                          <Zap size={16} />
                        )}
                      </button>
                      <button
                        className="action-btn"
                        title={t('channels:actions.epg')}
                        aria-label={t('channels:actions.epg')}
                        onClick={() => handleOpenEpgPrograms(channel)}
                      >
                        <CalendarDays size={16} />
                      </button>
                      <button
                        className="action-btn"
                        title={t('channels:actions.edit')}
                        aria-label={t('channels:actions.edit')}
                        onClick={() => handleEditChannel(channel)}
                      >
                        <Pencil size={16} />
                      </button>
                      <button
                        className="action-btn danger"
                        title={t('channels:actions.delete')}
                        aria-label={t('channels:actions.delete')}
                        onClick={() => setDeletingChannel(channel)}
                      >
                        <Trash2 size={16} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          {channelsData?.items?.length === 0 && (
            <div className="empty-state">
              <div className="empty-icon">
                <Tv size={48} strokeWidth={1} />
              </div>
              <div className="empty-title">{t('channels:empty.title')}</div>
              <div className="empty-desc">{t('channels:empty.desc')}</div>
            </div>
          )}
        </div>
      ) : (
        <div className="cards-grid">
          {channelsData?.items?.map((channel, index) => (
            <div
              key={channel.id}
              className={`channel-card card stagger-item ${selectedChannels.has(channel.id) ? 'selected' : ''}`}
              style={{ animationDelay: `${index * 0.04}s` }}
              onClick={() => handleSelectChannel(channel.id)}
            >
              <div className="card-thumbnail">
                {channel.logo_url ? (
                  <img src={channel.logo_url} alt={channel.name} />
                ) : (
                  <div className="thumbnail-placeholder">
                    <Tv size={32} />
                  </div>
                )}
                <div className="card-overlay">
                  <button
                    className="overlay-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      handlePlay(channel);
                    }}
                    title={t('channels:actions.play')}
                    aria-label={t('channels:actions.play')}
                  >
                    <CirclePlay size={24} />
                  </button>
                  <button
                    className="overlay-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      setTestingId(channel.id);
                      testMutation.mutate(channel.id);
                    }}
                    disabled={testingId === channel.id || batchTesting}
                    title={t('channels:actions.test')}
                    aria-label={t('channels:actions.test')}
                    style={{ marginLeft: '8px' }}
                  >
                    {testingId === channel.id ? (
                      <Loader2 size={24} className="animate-spin" />
                    ) : (
                      <Zap size={24} />
                    )}
                  </button>
                </div>
                <div className="card-status">{getStatusBadge(channel.status)}</div>
              </div>
              <div className="card-info">
                <div className="card-name">{channel.name}</div>
                <div className="card-group">{channel.group_name}</div>
                <div className="card-group">{getSourceBadge(channel)}</div>
              </div>
              <div className="actions-cell">
                <button
                  className="action-btn"
                  title={t('channels:actions.epg')}
                  aria-label={t('channels:actions.epg')}
                  onClick={(e) => {
                    e.stopPropagation();
                    handleOpenEpgPrograms(channel);
                  }}
                >
                  <CalendarDays size={16} />
                </button>
                <button
                  className="action-btn"
                  title={t('channels:actions.edit')}
                  aria-label={t('channels:actions.edit')}
                  onClick={(e) => {
                    e.stopPropagation();
                    handleEditChannel(channel);
                  }}
                >
                  <Pencil size={16} />
                </button>
                <button
                  className="action-btn danger"
                  title={t('channels:actions.delete')}
                  aria-label={t('channels:actions.delete')}
                  onClick={(e) => {
                    e.stopPropagation();
                    setDeletingChannel(channel);
                  }}
                >
                  <Trash2 size={16} />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {renderPagination()}

      <Suspense fallback={null}>
        {shouldRenderImportModal && (
          <ImportM3UModal
            isOpen={showImportModal}
            onClose={() => setShowImportModal(false)}
            onImported={handleImportCompleted}
          />
        )}

        {shouldRenderChannelModal && (
          <ChannelModal
            isOpen={showChannelModal}
            onClose={handleCloseChannelModal}
            channel={editingChannel}
          />
        )}

        {shouldRenderEpgImportModal && (
          <EpgImportModal
            isOpen={showEpgImportModal}
            onClose={() => setShowEpgImportModal(false)}
          />
        )}

        {shouldRenderEpgProgramsModal && epgChannel && (
          <EpgProgramsModal
            isOpen
            onClose={() => setEpgChannel(null)}
            channelRef={epgChannel.name}
            channelName={epgChannel.name}
          />
        )}

        <ConfirmDialog
          isOpen={deletingChannel !== null}
          onClose={() => setDeletingChannel(null)}
          onConfirm={handleConfirmChannelDelete}
          title={t('channels:deleteConfirmTitle')}
          message={t('channels:deleteConfirmMessage', { name: deletingChannel?.name ?? '' })}
          confirmText={t('common:delete')}
          type="danger"
          isLoading={deleteMutation.isPending}
        />

        <ConfirmDialog
          isOpen={showBatchDeleteConfirm}
          onClose={() => setShowBatchDeleteConfirm(false)}
          onConfirm={handleConfirmBatchDelete}
          title={t('channels:deleteSelectedConfirmTitle')}
          message={t('channels:deleteSelectedConfirm', { count: selectedChannels.size })}
          confirmText={t('common:delete')}
          type="danger"
          isLoading={batchDeleteMutation.isPending}
        />
      </Suspense>
    </div>
  );
};

export default Channels;
