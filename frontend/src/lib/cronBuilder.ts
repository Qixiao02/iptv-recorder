/**
 * Cron 表达式与简单 UI 状态的双向转换。
 *
 * 简单 UI 只能表达「每天/某些星期的固定时间」这种最常见的录制规则,
 * 对应 cron 的「M H * * W」形式(分、时为具体值,日/月为 *,周为 * 或列举)。
 * 高频规则(每小时、每 N 分钟、含日/月)无法用简单 UI 表达,会返回 null,
 * 调用方据此切换到「高级模式」让用户直接编辑 cron 字符串。
 *
 * 字段顺序遵循标准 5 字段 cron:分 时 日 月 周
 */

/** 简单 UI 的状态 */
export interface SimpleSchedule {
  /** 开始时间,格式 "HH:MM",如 "19:00"、"07:30" */
  time: string;
  /** 选中的星期,数字 0-6(0=周日, 1=周一, ..., 6=周六),与 cron 周字段语义一致。空数组=每天 */
  weekdays: number[];
}

/** 一周 7 天,按「周一→周日」顺序(便于 UI 按习惯显示),值用 cron 周字段语义(0=周日) */
export const WEEKDAY_ORDER = [1, 2, 3, 4, 5, 6, 0] as const;

/**
 * 把简单 UI 状态组装成合法的 5 字段 cron 表达式。
 *
 * 规则:
 * - time "HH:MM" → 分钟字段=MM, 小时字段=HH
 * - weekdays 空 → 周字段=*(每天)
 * - weekdays 全选(0-6) → 周字段=*(等价于每天,显示更简洁)
 * - weekdays 连续(如 1,2,3,4,5) → 周字段=1-5(范围语法)
 * - weekdays 不连续(如 6,0) → 周字段=6,0(逗号列举)
 *
 * @example buildCron({time:"19:00", weekdays:[]})       // "0 19 ... every day"
 * @example buildCron({time:"19:00", weekdays:[1,2,3,4,5]}) // weekdays 1-5
 * @example buildCron({time:"20:30", weekdays:[6,0]})     // weekend 6,0
 */
export function buildCron(s: SimpleSchedule): string {
  const [hh, mm] = s.time.split(':');
  const hour = Number.parseInt(hh, 10);
  const minute = Number.parseInt(mm, 10);

  // 防御:非法时间回退到 00:00
  const h = Number.isFinite(hour) && hour >= 0 && hour <= 23 ? hour : 0;
  const m = Number.isFinite(minute) && minute >= 0 && minute <= 59 ? minute : 0;

  // 周字段:去重 + 排序
  const uniqueDays = Array.from(new Set(s.weekdays)).filter(
    (d) => d >= 0 && d <= 6,
  );

  // 空或全选 → *(每天)
  const weekField =
    uniqueDays.length === 0 || uniqueDays.length === 7
      ? '*'
      : compressWeekdays(uniqueDays);

  return `${m} ${h} * * ${weekField}`;
}

/**
 * 尝试把 cron 表达式解析回简单 UI 状态。
 * 只识别「M H * * W」形式(分=具体值、时=具体值、日=*、月=*、周=任意合法)。
 *
 * @returns 解析成功返回 SimpleSchedule;无法用简单 UI 表达(高频/含日月)返回 null
 *
 * @example parseCron("0 19 ... every day")   // {time:"19:00", weekdays:[]}
 * @example parseCron with weekdays 1-5        // {time:"19:00", weekdays:[1,2,3,4,5]}
 * @example parseCron weekend                   // {time:"19:00", weekdays:[6,0]}
 * @example parseCron high-frequency            // null(简单 UI 表达不了)
 * @example parseCron with day-of-month         // null(含日字段,每月某号)
 */
export function parseCron(cron: string): SimpleSchedule | null {
  if (!cron || typeof cron !== 'string') return null;

  // 标准化:去首尾空格,中间多空格合并
  const parts = cron.trim().split(/\s+/);
  if (parts.length !== 5) return null;

  const [minuteField, hourField, dayField, monthField, weekField] = parts;

  // 必须是简单形式:日=*、月=*
  if (dayField !== '*' || monthField !== '*') return null;

  // 分、时必须是纯数字(不接受 */N、范围、列表)
  if (!/^\d+$/.test(minuteField) || !/^\d+$/.test(hourField)) return null;

  const minute = Number.parseInt(minuteField, 10);
  const hour = Number.parseInt(hourField, 10);
  if (minute < 0 || minute > 59 || hour < 0 || hour > 23) return null;

  // 周字段:*=每天(空数组);否则解析为数字数组
  let weekdays: number[] = [];
  if (weekField !== '*') {
    const expanded = expandWeekField(weekField);
    if (expanded === null) return null; // 周字段格式不识别
    weekdays = expanded;
  }

  // 格式化时间为 "HH:MM"(补零)
  const time = `${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')}`;

  return { time, weekdays };
}

/**
 * 把数字数组压缩成 cron 周字段字符串。
 * 连续序列用范围(1-5),不连续用逗号(6,0)。
 */
function compressWeekdays(days: number[]): string {
  const sorted = [...days].sort((a, b) => a - b);

  // 特殊处理:6,0(周六+周日)是常见组合,但排序后是 [0,6],范围语法 0-6 会变成全选
  // 这里检测这种"跨周末"情况,保留 6,0 形式
  if (sorted.length === 2 && sorted[0] === 0 && sorted[1] === 6) {
    return '6,0';
  }

  const segments: string[] = [];
  let start = sorted[0];
  let prev = sorted[0];

  for (let i = 1; i < sorted.length; i++) {
    const cur = sorted[i];
    if (cur === prev + 1) {
      prev = cur;
      continue;
    }
    // 段结束
    segments.push(start === prev ? `${start}` : `${start}-${prev}`);
    start = cur;
    prev = cur;
  }
  segments.push(start === prev ? `${start}` : `${start}-${prev}`);

  return segments.join(',');
}

/**
 * 把 cron 周字段展开成数字数组。
 * 支持:*（全选）、单值(3)、范围(1-5)、列表(1,3,5)、范围+列表混合(1-3,6)。
 * 不支持的格式返回 null。
 */
function expandWeekField(field: string): number[] | null {
  if (field === '*') {
    return [0, 1, 2, 3, 4, 5, 6];
  }

  const result: number[] = [];
  const parts = field.split(',');
  for (const part of parts) {
    // 范围:1-5
    const rangeMatch = part.match(/^(\d+)-(\d+)$/);
    if (rangeMatch) {
      const lo = Number.parseInt(rangeMatch[1], 10);
      const hi = Number.parseInt(rangeMatch[2], 10);
      if (lo < 0 || lo > 7 || hi < 0 || hi > 7 || lo > hi) return null;
      for (let d = lo; d <= hi; d++) {
        result.push(d % 7); // 7 等价于 0(周日),某些 cron 用 7 表示周日
      }
      continue;
    }
    // 单值:3
    const singleMatch = part.match(/^\d+$/);
    if (singleMatch) {
      const d = Number.parseInt(part, 10);
      if (d < 0 || d > 7) return null;
      result.push(d % 7);
      continue;
    }
    // 不识别(如 */2、1/2 等)
    return null;
  }

  return result;
}
