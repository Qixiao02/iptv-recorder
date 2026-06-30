import { useEffect, type RefObject } from 'react';

/**
 * 模态无障碍:Esc 关闭 + 焦点陷阱 + 焦点还原。
 *
 * 背景:此前 7 个模态全部没有 Esc 关闭、没有焦点陷阱、关闭不还原焦点。键盘和屏幕
 * 阅读器用户会被困住——Tab 会跑到模态背后的页面,Esc 无法关闭。
 *
 * 用法:在模态组件里给 overlay div 加 ref,并传入 isOpen 与 onClose:
 *   const overlayRef = useRef<HTMLDivElement>(null);
 *   useModalA11y(overlayRef, isOpen, onClose);
 *   return <div ref={overlayRef} role="dialog" aria-modal="true" onKeyDown={...} ...>
 *
 * 注意:本 hook 只负责焦点管理与 Esc;overlay 的 role/aria-modal 由组件 JSX 提供,
 * 这样不同模态(dialog/alertdialog)可用各自语义。Esc 也由本 hook 在 document 上
 * 监听(覆盖整个模态,不必每个子元素绑 onKeyDown)。
 *
 * 无外部依赖(项目未装 react-focus-lock 等),手写最小实现。
 */

// 可聚焦元素选择器(排除 disabled / hidden / aria-hidden)
const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'textarea:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

export function useModalA11y(
  overlayRef: RefObject<HTMLElement | null>,
  isOpen: boolean,
  onClose: () => void,
): void {
  useEffect(() => {
    if (!isOpen) {
      return;
    }

    // 记录打开模态前的焦点元素,关闭时还原(让触发按钮重新获得焦点)。
    const previouslyFocused = document.activeElement as HTMLElement | null;

    const overlay = overlayRef.current;
    if (overlay) {
      // 打开时把焦点移进模态(优先第一个可聚焦元素,否则 overlay 本身)。
      const focusables = Array.from(
        overlay.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
      ).filter((el) => el.offsetParent !== null || el === document.activeElement);
      const initial = focusables[0] ?? overlay;
      // 微延迟等 DOM 渲染完成(模态由条件渲染挂载)
      requestAnimationFrame(() => initial.focus());
    }

    // Esc 关闭(忽略输入框里的 Esc 由原生处理?这里统一关闭更符合模态预期)。
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        onClose();
        return;
      }

      // Tab 焦点陷阱:在首末可聚焦元素之间循环,不让 Tab 跑到背景页面。
      if (event.key === 'Tab' && overlay) {
        const focusables = Array.from(
          overlay.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
        ).filter((el) => el.offsetParent !== null);
        if (focusables.length === 0) {
          // 没有可聚焦子元素,把焦点钉在 overlay(overlay 自身需 tabindex=-1 才能聚焦,
          // 由组件 JSX 提供;否则保持焦点不动)。
          return;
        }
        const first = focusables[0];
        const last = focusables[focusables.length - 1];
        const active = document.activeElement;

        if (event.shiftKey) {
          // Shift+Tab:从首个跳回末个
          if (active === first || !overlay.contains(active)) {
            event.preventDefault();
            last.focus();
          }
        } else {
          // Tab:从末个跳回首个
          if (active === last || !overlay.contains(active)) {
            event.preventDefault();
            first.focus();
          }
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      // 还原焦点到触发元素
      if (previouslyFocused && typeof previouslyFocused.focus === 'function') {
        previouslyFocused.focus();
      }
    };
    // onClose 引用变化不应重跑 effect(否则会重复记焦点)。用 ref 稳定化代价高,
    // 这里依赖 [isOpen]——onClose 通常是稳定回调(setState 包装),实践中安全。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen]);
}
