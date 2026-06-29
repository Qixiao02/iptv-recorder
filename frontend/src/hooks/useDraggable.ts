import { useCallback, useRef, type MouseEvent } from 'react';

/**
 * 纯手写拖拽 hook(零依赖)。
 *
 * 用法：把返回的 onDragStart 绑到"拖动把手"的 onMouseDown。
 * 按下后监听 window mousemove/mouseup，移动时调用 onMove(新坐标)，
 * 抬起解绑。自带视口边界约束(不拖出屏幕)。
 *
 * @param onMove    拖动时回调，参数为元素左上角的新坐标(已做边界约束)
 * @param getSize   返回元素当前的 {width,height}，用于边界计算
 * @param enabled   是否启用拖拽
 */
export function useDraggable(
  onMove: (pos: { x: number; y: number }) => void,
  getSize: () => { width: number; height: number },
  enabled: boolean = true,
) {
  // 拖动起点信息(鼠标按下时的鼠标坐标 - 元素左上角坐标)
  const dragInfo = useRef<{ offsetX: number; offsetY: number } | null>(null);

  const onDragStart = useCallback(
    (e: MouseEvent<HTMLElement>) => {
      if (!enabled) return;
      // 只响应左键，忽略右键/中键
      if (e.button !== 0) return;
      // 阻止文本选中等默认行为
      e.preventDefault();
      e.stopPropagation();

      const el = e.currentTarget.parentElement;
      if (!el) return;

      // 当前元素的左上角坐标(相对视口)
      const rect = el.getBoundingClientRect();
      // 记录鼠标相对于元素左上角的偏移，这样拖动时元素不会"跳"到鼠标位置
      dragInfo.current = {
        offsetX: e.clientX - rect.left,
        offsetY: e.clientY - rect.top,
      };

      const handleMove = (ev: globalThis.MouseEvent) => {
        const info = dragInfo.current;
        if (!info) return;
        const { width } = getSize();
        // 新的左上角坐标 = 鼠标坐标 - 偏移
        let x = ev.clientX - info.offsetX;
        let y = ev.clientY - info.offsetY;
        // 边界约束：至少留 60px 在视口内，避免拖没了找不到
        const minVisible = 60;
        const margin = 8;
        x = Math.max(-(width - minVisible), x);
        x = Math.min(window.innerWidth - minVisible - margin, x);
        y = Math.max(margin, y);
        y = Math.min(window.innerHeight - minVisible - margin, y);
        onMove({ x, y });
      };

      const handleUp = () => {
        dragInfo.current = null;
        window.removeEventListener('mousemove', handleMove);
        window.removeEventListener('mouseup', handleUp);
        // 拖完恢复 body 的 user-select
        document.body.style.userSelect = '';
      };

      window.addEventListener('mousemove', handleMove);
      window.addEventListener('mouseup', handleUp);
      // 拖动期间禁止选中文本
      document.body.style.userSelect = 'none';
    },
    [enabled, getSize, onMove],
  );

  return { onDragStart };
}
