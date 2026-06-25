import { describe, expect, it } from 'vitest';
import { formatBytes } from './format';

describe('formatBytes', () => {
  it('formats recording file sizes as gigabytes from the byte value', () => {
    expect(formatBytes(0.34 * 1024 ** 3, 'zh-CN')).toBe('0.34G');
    expect(formatBytes(0.5 * 1024 ** 3, 'zh-CN')).toBe('0.5G');
    expect(formatBytes(1024 ** 3, 'zh-CN')).toBe('1G');
    expect(formatBytes(1.23 * 1024 ** 3, 'zh-CN')).toBe('1.23G');
    expect(formatBytes(5.2 * 1024 ** 3, 'zh-CN')).toBe('5.2G');
  });

  it('keeps sub-gigabyte values in gigabytes', () => {
    expect(formatBytes(512 * 1024 ** 2, 'zh-CN')).toBe('0.5G');
    expect(formatBytes(128 * 1024 ** 2, 'zh-CN')).toBe('0.13G');
  });
});
