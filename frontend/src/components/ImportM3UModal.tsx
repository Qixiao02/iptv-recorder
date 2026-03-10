import React, { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { importM3UFromUrl, importM3UFromContent } from '@/api/channels';
import { X, Upload, Link, FileText, Loader2, CheckCircle, AlertCircle } from 'lucide-react';
import type { ImportM3UResponse } from '@/types';
import './Modal.css';

interface ImportM3UModalProps {
  isOpen: boolean;
  onClose: () => void;
}

type ImportMode = 'url' | 'content';

export const ImportM3UModal: React.FC<ImportM3UModalProps> = ({ isOpen, onClose }) => {
  const queryClient = useQueryClient();
  const [mode, setMode] = useState<ImportMode>('url');
  const [url, setUrl] = useState('');
  const [content, setContent] = useState('');
  const [overwrite, setOverwrite] = useState(false);
  const [result, setResult] = useState<ImportM3UResponse | null>(null);

  const importUrlMutation = useMutation({
    mutationFn: importM3UFromUrl,
    onSuccess: (data) => {
      setResult(data);
      queryClient.invalidateQueries({ queryKey: ['channels'] });
    },
  });

  const importContentMutation = useMutation({
    mutationFn: importM3UFromContent,
    onSuccess: (data) => {
      setResult(data);
      queryClient.invalidateQueries({ queryKey: ['channels'] });
    },
  });

  const isLoading = importUrlMutation.isPending || importContentMutation.isPending;

  const handleSubmit = () => {
    setResult(null);
    if (mode === 'url' && url.trim()) {
      importUrlMutation.mutate({ url: url.trim(), overwrite });
    } else if (mode === 'content' && content.trim()) {
      importContentMutation.mutate({ content: content.trim(), overwrite });
    }
  };

  const handleClose = () => {
    setUrl('');
    setContent('');
    setResult(null);
    setOverwrite(false);
    onClose();
  };

  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={handleClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>导入 M3U 播放列表</h2>
          <button className="modal-close" onClick={handleClose}>
            <X size={20} />
          </button>
        </div>

        <div className="modal-body">
          {/* Mode Tabs */}
          <div className="modal-tabs">
            <button
              className={`modal-tab ${mode === 'url' ? 'active' : ''}`}
              onClick={() => setMode('url')}
            >
              <Link size={16} />
              从 URL 导入
            </button>
            <button
              className={`modal-tab ${mode === 'content' ? 'active' : ''}`}
              onClick={() => setMode('content')}
            >
              <FileText size={16} />
              从内容导入
            </button>
          </div>

          {/* URL Input */}
          {mode === 'url' && (
            <div className="form-group">
              <label>M3U 文件 URL</label>
              <input
                type="text"
                className="input"
                placeholder="https://example.com/playlist.m3u"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
              />
            </div>
          )}

          {/* Content Input */}
          {mode === 'content' && (
            <div className="form-group">
              <label>M3U 文件内容</label>
              <textarea
                className="input textarea"
                placeholder="#EXTM3U&#10;#EXTINF:-1,CCTV-1&#10;http://example.com/stream.m3u8"
                value={content}
                onChange={(e) => setContent(e.target.value)}
                rows={10}
              />
            </div>
          )}

          {/* Options */}
          <div className="form-group">
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={overwrite}
                onChange={(e) => setOverwrite(e.target.checked)}
              />
              覆盖已存在的频道
            </label>
          </div>

          {/* Result */}
          {result && (
            <div className="import-result">
              <div className="result-header">
                <CheckCircle size={18} />
                导入完成
              </div>
              <div className="result-stats">
                <span className="stat success">成功: {result.imported}</span>
                <span className="stat warning">跳过: {result.skipped}</span>
                <span className="stat error">失败: {result.failed}</span>
              </div>
              {result.errors.length > 0 && (
                <div className="result-errors">
                  {result.errors.slice(0, 5).map((err, i) => (
                    <div key={i} className="error-item">
                      <AlertCircle size={14} />
                      {err}
                    </div>
                  ))}
                  {result.errors.length > 5 && (
                    <div className="more-errors">还有 {result.errors.length - 5} 个错误...</div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={handleClose}>
            取消
          </button>
          <button
            className="btn btn-primary"
            onClick={handleSubmit}
            disabled={isLoading || (mode === 'url' ? !url.trim() : !content.trim())}
          >
            {isLoading ? (
              <>
                <Loader2 size={16} className="animate-spin" />
                导入中...
              </>
            ) : (
              <>
                <Upload size={16} />
                开始导入
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ImportM3UModal;
