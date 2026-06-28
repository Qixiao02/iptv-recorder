import React, { useEffect, useRef } from 'react';
import './Markdown.css';

/**
 * 极简 Markdown 渲染器(零第三方依赖)
 *
 * 仅处理 README 需要的核心语法:标题、段落、列表、代码块、行内代码、
 * 表格、引用、链接、分隔线、加粗。足以在「关于」页友好展示 README。
 *
 * 不追求完整 CommonNet 兼容,聚焦可读性。
 */

interface MarkdownProps {
  content: string;
}

/** 转义 HTML 特殊字符,防止注入 */
function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

/** 将标题文本转为 HTML id 锚点(与 GitHub 风格接近):中文保留、小写、空格转连字符 */
function slugify(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[`*]/g, '')
    .replace(/\s+/g, '-');
}

/** 处理行内格式:加粗、行内代码、图片、链接 */
function renderInline(text: string): string {
  let out = escapeHtml(text);
  // 行内代码 `code`
  out = out.replace(/`([^`]+)`/g, '<code class="md-code">$1</code>');
  // 加粗 **text**
  out = out.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  // 图片 ![alt](url) —— 必须在链接之前匹配,否则会被链接正则误吃
  out = out.replace(
    /!\[([^\]]*)\]\(([^)\s]+)\)/g,
    '<img src="$2" alt="$1" class="md-img" loading="lazy" />'
  );
  // 链接 [text](url)
  // # 锚点链接:用平滑滚动跳转,不开新标签
  out = out.replace(
    /\[([^\]]+)\]\(#[^)\s]+\)/g,
    '<a href="#" data-md-anchor="$1" class="md-anchor">$1</a>'
  );
  // 普通外部链接:新标签打开
  out = out.replace(
    /\[([^\]]+)\]\(([^)\s]+)\)/g,
    '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>'
  );
  return out;
}

/** 解析表格(以 | 分隔、第二行是分隔行) */
function parseTable(lines: string[], startIndex: number): { html: string; nextIndex: number } {
  const rows: string[][] = [];
  let i = startIndex;
  while (i < lines.length && lines[i].trim().includes('|')) {
    const cells = lines[i]
      .trim()
      .replace(/^\||\|$/g, '')
      .split('|')
      .map((c) => c.trim());
    // 跳过分隔行 |---|---|
    if (cells.every((c) => /^:?-+:?$/.test(c))) {
      i++;
      continue;
    }
    rows.push(cells);
    i++;
  }
  if (rows.length === 0) return { html: '', nextIndex: startIndex };

  const header = rows[0];
  const body = rows.slice(1);
  let html = '<table class="md-table"><thead><tr>';
  header.forEach((c) => {
    html += `<th>${renderInline(c)}</th>`;
  });
  html += '</tr></thead><tbody>';
  body.forEach((row) => {
    html += '<tr>';
    row.forEach((c) => {
      html += `<td>${renderInline(c)}</td>`;
    });
    html += '</tr>';
  });
  html += '</tbody></table>';
  return { html, nextIndex: i };
}

export const Markdown: React.FC<MarkdownProps> = ({ content }) => {
  const lines = content.replace(/\r\n/g, '\n').split('\n');
  const parts: string[] = [];
  let i = 0;
  let listOpen = false;

  const closeList = () => {
    if (listOpen) {
      parts.push('</ul>');
      listOpen = false;
    }
  };

  while (i < lines.length) {
    const line = lines[i];
    const trimmed = line.trim();

    // 空行
    if (trimmed === '') {
      closeList();
      i++;
      continue;
    }

    // 水平分隔线
    if (/^(-{3,}|\*{3,}|_{3,})$/.test(trimmed)) {
      closeList();
      parts.push('<hr class="md-hr" />');
      i++;
      continue;
    }

    // 标题
    const headingMatch = trimmed.match(/^(#{1,6})\s+(.*)$/);
    if (headingMatch) {
      closeList();
      const level = headingMatch[1].length;
      const titleText = headingMatch[2];
      const id = slugify(titleText);
      parts.push(
        `<h${level} id="${id}" class="md-h md-h${level}">${renderInline(titleText)}</h${level}>`
      );
      i++;
      continue;
    }

    // 引用块
    if (trimmed.startsWith('>')) {
      closeList();
      parts.push(`<blockquote class="md-quote">${renderInline(trimmed.replace(/^>\s?/, ''))}</blockquote>`);
      i++;
      continue;
    }

    // 代码块 ```
    if (trimmed.startsWith('```')) {
      closeList();
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith('```')) {
        codeLines.push(lines[i]);
        i++;
      }
      i++; // 跳过结束的 ```
      parts.push(`<pre class="md-pre"><code>${escapeHtml(codeLines.join('\n'))}</code></pre>`);
      continue;
    }

    // 表格(当前行含 |,且下一行是分隔行 |---|)
    if (
      trimmed.includes('|') &&
      i + 1 < lines.length &&
      /^\s*\|?[\s:|-]+\|?\s*$/.test(lines[i + 1]) &&
      lines[i + 1].includes('-')
    ) {
      closeList();
      const { html, nextIndex } = parseTable(lines, i);
      if (nextIndex > i) {
        parts.push(html);
        i = nextIndex;
        continue;
      }
    }

    // 无序列表 - / *
    if (/^[-*]\s+/.test(trimmed)) {
      if (!listOpen) {
        parts.push('<ul class="md-ul">');
        listOpen = true;
      }
      parts.push(`<li>${renderInline(trimmed.replace(/^[-*]\s+/, ''))}</li>`);
      i++;
      continue;
    }

    // 普通段落
    closeList();
    parts.push(`<p class="md-p">${renderInline(trimmed)}</p>`);
    i++;
  }

  closeList();

  return <MarkdownBody html={parts.join('\n')} />;
};

/** 渲染容器 + 锚点点击事件委托(平滑滚动到对应标题) */
const MarkdownBody: React.FC<{ html: string }> = ({ html }) => {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const handleClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      const anchor = target.closest('[data-md-anchor]') as HTMLElement | null;
      if (!anchor) return;

      e.preventDefault();
      const anchorText = anchor.getAttribute('data-md-anchor') || '';
      const targetId = slugify(anchorText);
      // 在整个文档范围内查找标题(锚点可能指向容器外的元素)
      const heading = document.getElementById(targetId);
      if (heading) {
        heading.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    };

    el.addEventListener('click', handleClick);
    return () => el.removeEventListener('click', handleClick);
  }, [html]);

  return (
    <div
      ref={containerRef}
      className="markdown-body"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
};

export default Markdown;
