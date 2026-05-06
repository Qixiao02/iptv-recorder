import React, { useEffect, useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { importEpgSource } from '@/api/epg';
import { Loader2, X } from 'lucide-react';
import './Modal.css';

interface EpgImportModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const EpgImportModal: React.FC<EpgImportModalProps> = ({ isOpen, onClose }) => {
  const queryClient = useQueryClient();
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');

  useEffect(() => {
    if (!isOpen) {
      setName('');
      setUrl('');
    }
  }, [isOpen]);

  const mutation = useMutation({
    mutationFn: importEpgSource,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['epg', 'sources'] });
      onClose();
    },
  });

  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>导入 EPG 节目单</h2>
          <button className="modal-close" onClick={onClose}>
            <X size={20} />
          </button>
        </div>

        <div className="modal-body">
          <div className="form-group">
            <label>来源名称 *</label>
            <input
              className="input"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如：央视 XMLTV"
            />
          </div>

          <div className="form-group">
            <label>XMLTV URL *</label>
            <input
              className="input"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://example.com/epg.xml"
            />
          </div>

          {mutation.isError && (
            <div className="form-error">
              {(mutation.error as Error).message}
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onClose}>
            取消
          </button>
          <button
            className="btn btn-primary"
            onClick={() => mutation.mutate({ name, url })}
            disabled={mutation.isPending || !name.trim() || !url.trim()}
          >
            {mutation.isPending ? <Loader2 size={16} className="animate-spin" /> : '导入'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default EpgImportModal;
