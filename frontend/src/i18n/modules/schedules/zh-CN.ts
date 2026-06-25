export default {
  title: '录制计划',
  enabledCount: '{{count}} 个计划启用中',
  add: '新建计划',
  create: '创建计划',
  created: '录制任务已创建',
  executeFailed: '立即执行失败: {{message}}',
  cron: {
    everyMinute: '每分钟',
    everyNMinutes: '每 {{count}} 分钟',
    everyHour: '每小时',
    everyNHours: '每 {{count}} 小时',
    weekday: '工作日 {{time}}',
    weekend: '周末 {{time}}',
    monthly: '每月 {{day}} 日 {{time}}',
    daily: '每天 {{time}}',
    days: ['周日', '周一', '周二', '周三', '周四', '周五', '周六'],
  },
  details: {
    cron: 'Cron 表达式',
    outputTemplate: '输出模板',
    outputDir: '输出目录',
    systemDefault: '使用系统默认',
    priority: '优先级',
    retry: '重试次数',
  },
  actions: {
    execute: '立即执行',
    edit: '编辑',
    delete: '删除',
  },
  empty: {
    title: '暂无录制计划',
    desc: '创建定时录制计划，自动录制您喜爱的节目',
  },
};

