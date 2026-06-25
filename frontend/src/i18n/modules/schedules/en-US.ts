export default {
  title: 'Schedules',
  enabledCount: '{{count}} schedules enabled',
  add: 'Add Schedule',
  create: 'Create Schedule',
  created: 'Recording task created',
  executeFailed: 'Run now failed: {{message}}',
  cron: {
    everyMinute: 'Every minute',
    everyNMinutes: 'Every {{count}} minutes',
    everyHour: 'Every hour',
    everyNHours: 'Every {{count}} hours',
    weekday: 'Weekdays {{time}}',
    weekend: 'Weekends {{time}}',
    monthly: 'Monthly on day {{day}} at {{time}}',
    daily: 'Daily {{time}}',
    days: ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'],
  },
  details: {
    cron: 'Cron Expression',
    outputTemplate: 'Output Template',
    outputDir: 'Output Directory',
    systemDefault: 'Use system default',
    priority: 'Priority',
    retry: 'Retries',
  },
  actions: {
    execute: 'Run now',
    edit: 'Edit',
    delete: 'Delete',
  },
  empty: {
    title: 'No schedules',
    desc: 'Create scheduled recordings for your favorite programs',
  },
};

