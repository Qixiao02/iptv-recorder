import { useCallback, useRef, type MouseEvent } from 'react';

/**
 * 纯手写 resize hook(零依赖)。
 *
 * 用法：把返回的 onResizeStart 绑到"右下角 resize 把手"的 onMouseDown。
 * 按下后监听 window mousemove/mouseup，水平拖动改变宽度，
 * 高度按 16:9 由 CSS 自动算(这里只管宽度)。带最小/最大宽度约束。
 *
 * @param onResize  拖动时回调，参数为新的宽度(px)
 * @param opts      { minWidth, maxWidth } 约束范围
 * @param enabled   是否启用
 */
export function useResizable(
  onResize: (width: number) => void,
  opts: { minWidth: number; maxWidth: number },
  enabled: boolean = true,
) {
  // 起点信息：按下时的鼠标 x 坐标 + 当时元素宽度
  const resizeInfo = useRef<{ startClientX: number; startWidth: number } | null>(null);

  const onResizeStart = useCallback(
    (e: MouseEvent<HTMLElement>) => {
      if (!enabled) return;
      if (e.button !== 0) return;
      e.preventDefault();
      e.stopPropagation();

      const el = e.currentTarget.parentElement;
      if (!el) return;

      const rect = el.getBoundingClientRect();
      resizeInfo.current = {
        startClientX: e.clientX,
        startWidth: rect.width,
      };

      const handleMove = (ev: globalThis.MouseEvent) => {
        const info = resizeInfo.current;
        if (!info) return;
        // 新宽度 = 起始宽度 + (当前鼠标 x - 起始鼠标 x)
        // 向右拖增大，向左拖减小
        const delta = ev.clientX - info.startClientX;
        let width = info.startWidth + delta;
        width = Math.max(opts.minWidth, Math.min(opts.maxWidth, width));
        // 不超过视口宽度(留点边距)
        width = Math.min(width, window.innerWidth - 32);
        onResize(width);
      };

      const handleUp = () => {
        resizeInfo.current = null;
        window.removeEventListener('mousemove', handleMove);
        window.removeEventListener('mouseup', handleUp);
        document.body.style.userSelect = '';
      };

      window.addEventListener('mousemove', handleMove);
      window.addEventListener('mouseup', handleUp);
      document.body.style.userSelect = 'none';
    },
    [enabled, opts.minWidth, opts.maxWidth, onResize],
  );

  return { onResizeStart };
}
