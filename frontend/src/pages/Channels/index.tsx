import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getChannels, getChannelGroups, deleteChannel, testChannel } from '@/api/channels';
import { ImportM3UModal } from '@/components/ImportM3UModal';
import { ChannelModal } from '@/components/ChannelModal';
import { PlayerModal } from '@/components/PlayerModal';
import {
  Search,
  Plus,
  LayoutGrid,
  List,
  Filter,
  Upload,
  MoreHorizontal,
  CirclePlay,
  Pencil,
  Trash2,
  CircleCheck,
  CircleX,
  Tv,
  Loader2,
  Zap,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react';
import type { Channel } from '@/types';
import './Channels.css';

type ViewMode = 'table' | 'card';

export const Channels: React.FC = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [viewMode, setViewMode] = useState<ViewMode>('table');
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedGroup, setSelectedGroup] = useState<string>('all');
  const [selectedChannels, setSelectedChannels] = useState<Set<string>>(new Set());
  const [showImportModal, setShowImportModal] = useState(false);
  const [showChannelModal, setShowChannelModal] = useState(false);
  const [editingChannel, setEditingChannel] = useState<Channel | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [batchTesting, setBatchTesting] = useState(false);
  const [testProgress, setTestProgress] = useState({ current: 0, total: 0 });
  const [playerChannel, setPlayerChannel] = useState<Channel | null>(null);

  // 分页状态
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);

  // 搜索防抖
  const [debouncedSearch, setDebouncedSearch] = useState('');

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(searchQuery);
      setPage(1); // 搜索时重置到第一页
    }, 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  // 当分组改变时重置页码
  useEffect(() => {
    setPage(1);
  }, [selectedGroup]);

  const { data: channelsData, isLoading } = useQuery({
    queryKey: ['channels', page, pageSize, selectedGroup, debouncedSearch],
    queryFn: () => getChannels({
      page,
      page_size: pageSize,
      group: selectedGroup !== 'all' ? selectedGroup : undefined,
      search: debouncedSearch || undefined,
    }),
  });

  const { data: groups } = useQuery({
    queryKey: ['channels', 'groups'],
    queryFn: getChannelGroups,
  });

  const deleteMutation = useMutation({
    mutationFn: deleteChannel,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['channels'] });
    },
  });

  const batchDeleteMutation = useMutation({
    mutationFn: async (ids: string[]) => {
      for (const id of ids) await deleteChannel(id);
    },
    onSuccess: () => {
      setSelectedChannels(new Set());
      queryClient.invalidateQueries({ queryKey: ['channels'] });
    },
  });

  const handleBatchDelete = () => {
    if (!window.confirm(`确认删除选中的 ${selectedChannels.size} 个频道？`)) return;
    batchDeleteMutation.mutate(Array.from(selectedChannels));
  };

  const testMutation = useMutation({
    mutationFn: testChannel,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['channels'] });
      setTestingId(null);
    },
    onError: () => {
      setTestingId(null);
    },
  });

  // 一键测试当前页频道
  const handleBatchTest = async () => {
    const channelsToTest = selectedChannels.size > 0
      ? channelsData?.items?.filter(c => selectedChannels.has(c.id)) || []
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
        // 忽略单个测试错误
      }
      // 每次测试后刷新列表
      queryClient.invalidateQueries({ queryKey: ['channels'] });
    }

    setBatchTesting(false);
    setTestProgress({ current: 0, total: 0 });
  };

  // 播放频道
  const handlePlay = (channel: Channel) => {
    setPlayerChannel(channel);
  };

  const handleSelectAll = () => {
    if (selectedChannels.size === channelsData?.items?.length) {
      setSelectedChannels(new Set());
    } else {
      setSelectedChannels(new Set(channelsData?.items?.map((c) => c.id) || []));
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

  const handleCloseChannelModal = () => {
    setShowChannelModal(false);
    setEditingChannel(null);
  };

  const getStatusBadge = (status: Channel['status']) => {
    switch (status) {
      case 'online':
        return <span className="badge badge-success"><CircleCheck size={12} />在线</span>;
      case 'offline':
        return <span className="badge badge-error"><CircleX size={12} />离线</span>;
      case 'slow':
        return <span className="badge badge-warning"><CircleCheck size={12} />缓慢</span>;
      default:
        return <span className="badge badge-neutral">未知</span>;
    }
  };

  // 分页控制
  const handlePageChange = (newPage: number) => {
    setPage(newPage);
    setSelectedChannels(new Set()); // 切换页面时清空选择
  };

  const renderPagination = () => {
    if (!channelsData || channelsData.total_pages <= 1) return null;

    const pages = [];
    const maxVisiblePages = 5;
    let startPage = Math.max(1, page - Math.floor(maxVisiblePages / 2));
    let endPage = Math.min(channelsData.total_pages, startPage + maxVisiblePages - 1);

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
        >
          <ChevronLeft size={16} />
        </button>

        {startPage > 1 && (
          <>
            <button className="pagination-btn" onClick={() => handlePageChange(1)}>1</button>
            {startPage > 2 && <span className="pagination-ellipsis">...</span>}
          </>
        )}

        {pages.map(p => (
          <button
            key={p}
            className={`pagination-btn ${p === page ? 'active' : ''}`}
            onClick={() => handlePageChange(p)}
          >
            {p}
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
          <option value={10}>10 条/页</option>
          <option value={20}>20 条/页</option>
          <option value={50}>50 条/页</option>
          <option value={100}>100 条/页</option>
        </select>
      </div>
    );
  };

  return (
    <div className="channels-page">
      {/* Page Header */}
      <div className="page-header">
        <div className="page-title">
          <h1>{t('menu.channels')}</h1>
          <p className="page-subtitle">
            共 {channelsData?.total || 0} 个频道
          </p>
        </div>
        <div className="page-actions">
          <button className="btn btn-ghost" onClick={() => setShowImportModal(true)}>
            <Upload size={16} />
            导入 M3U
          </button>
          <button className="btn btn-primary" onClick={() => setShowChannelModal(true)}>
            <Plus size={16} />
            {t('common.add')}
          </button>
        </div>
      </div>

      {/* Toolbar */}
      <div className="toolbar card">
        <div className="toolbar-left">
          <div className="search-box">
            <Search size={16} />
            <input
              type="text"
              placeholder={t('common.search')}
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
              <option value="all">全部分组</option>
              {groups?.map((group) => (
                <option key={group} value={group}>
                  {group}
                </option>
              ))}
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
                测试中 {testProgress.current}/{testProgress.total}
              </>
            ) : (
              <>
                <Zap size={16} />
                一键测试
              </>
            )}
          </button>
        </div>

        <div className="toolbar-right">
          <div className="view-toggle">
            <button
              className={`toggle-btn ${viewMode === 'table' ? 'active' : ''}`}
              onClick={() => setViewMode('table')}
            >
              <List size={16} />
            </button>
            <button
              className={`toggle-btn ${viewMode === 'card' ? 'active' : ''}`}
              onClick={() => setViewMode('card')}
            >
              <LayoutGrid size={16} />
            </button>
          </div>
        </div>
      </div>

      {/* Batch Actions */}
      {selectedChannels.size > 0 && (
        <div className="batch-actions animate-fade-in">
          <span className="batch-count">已选择 {selectedChannels.size} 个频道</span>
          <div className="batch-buttons">
            <button className="btn btn-ghost btn-sm">批量录制</button>
            <button
              className="btn btn-ghost btn-sm danger"
              onClick={handleBatchDelete}
              disabled={batchDeleteMutation.isPending}
            >批量删除</button>
          </div>
        </div>
      )}

      {/* Content */}
      {isLoading ? (
        <div className="loading-grid">
          {[1, 2, 3, 4, 5, 6].map((i) => (
            <div key={i} className="skeleton-card animate-shimmer" />
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
                <th className="col-channel">频道</th>
                <th className="col-url">流地址</th>
                <th className="col-group">分组</th>
                <th className="col-status">状态</th>
                <th className="col-actions">操作</th>
              </tr>
            </thead>
            <tbody>
              {channelsData?.items?.map((channel, idx) => (
                <tr
                  key={channel.id}
                  className={`stagger-item ${selectedChannels.has(channel.id) ? 'selected' : ''}`}
                  style={{ animationDelay: `${idx * 0.03}s` }}
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
                    <span className="group-tag">{channel.group_name}</span>
                  </td>
                  <td className="col-status">{getStatusBadge(channel.status)}</td>
                  <td className="col-actions">
                    <div className="actions-cell">
                      <button
                        className="action-btn"
                        title="播放"
                        onClick={() => handlePlay(channel)}
                      >
                        <CirclePlay size={16} />
                      </button>
                      <button
                        className="action-btn"
                        title="测试连接"
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
                        title="编辑"
                        onClick={() => handleEditChannel(channel)}
                      >
                        <Pencil size={16} />
                      </button>
                      <button
                        className="action-btn danger"
                        title="删除"
                        onClick={() => deleteMutation.mutate(channel.id)}
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
              <div className="empty-title">没有找到频道</div>
              <div className="empty-desc">尝试调整搜索条件或导入新的 M3U 播放列表</div>
            </div>
          )}
        </div>
      ) : (
        <div className="cards-grid">
          {channelsData?.items?.map((channel, idx) => (
            <div
              key={channel.id}
              className={`channel-card card stagger-item ${selectedChannels.has(channel.id) ? 'selected' : ''}`}
              style={{ animationDelay: `${idx * 0.04}s` }}
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
                    title="播放"
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
                    title="测试连接"
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
              </div>
              <button className="card-menu">
                <MoreHorizontal size={16} />
              </button>
            </div>
          ))}
        </div>
      )}

      {/* Pagination */}
      {renderPagination()}

      {/* Import M3U Modal */}
      <ImportM3UModal
        isOpen={showImportModal}
        onClose={() => setShowImportModal(false)}
      />

      {/* Channel Modal */}
      <ChannelModal
        isOpen={showChannelModal}
        onClose={handleCloseChannelModal}
        channel={editingChannel}
      />

      {/* Player Modal */}
      <PlayerModal
        isOpen={!!playerChannel}
        onClose={() => setPlayerChannel(null)}
        channelId={playerChannel?.id || ''}
        channelName={playerChannel?.name || ''}
        channelUrl={playerChannel?.url || ''}
      />
    </div>
  );
};

export default Channels;
