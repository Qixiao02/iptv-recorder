# Operations Runbook

## Scope

This runbook covers the day-2 operational flow for the IPTV Recorder service after deployment:

- runtime health checks
- audit review
- cleanup and scheduler maintenance
- incident triage

## Daily Checks

1. Open the Settings page and switch to the `运行维护` section with an admin account.
2. Confirm:
   - system status is `稳定` or expected `运行中`
   - `24h 失败` count is within expected range
   - recent WebSocket alerts do not show repeated proxy, disk or recording failures
3. Review the latest audit log rows for unexpected:
   - schedule changes
   - manual recordings
   - config updates
   - EPG imports

## Before A Release

1. Complete `docs/release-checklist.md`.
2. Confirm the runtime values shown in Settings match the intended deployment config.
3. Run one manual recording and verify:
   - task appears in the Tasks page
   - WebSocket progress updates are visible
   - output file lands in the expected directory

## Scheduler Maintenance

Use the `重载调度器` action from the `运行维护` section when:

- schedules were corrected after an unexpected runtime issue
- a deployment changed scheduler-related code or config
- you suspect in-memory state diverged from database state

After reloading, verify:

- enabled schedule count still matches expectations
- upcoming tasks can be queried successfully
- no new warnings appear in the alert stream

## Cleanup Maintenance

Use the `执行清理` action when:

- retention policy changed and you want immediate effect
- expired task records are known to be accumulating
- you need to validate the cleanup pipeline after deployment

After cleanup:

- confirm the returned deleted count is reasonable
- re-open the Tasks page and verify stale completed/failed records were removed as expected
- review the audit log for a `cleanup.run` entry

## Incident Triage

### Repeated task failures

1. Filter the Tasks page to `失败`.
2. Open task details and compare error messages.
3. Review corresponding audit actions and recent alerts.
4. Check:
   - recording executable path
   - channel source availability
   - storage free space threshold
   - recent scheduler/config changes

### Suspicious configuration or schedule changes

1. Open `运行维护` and inspect recent audit rows.
2. Identify:
   - acting user
   - timestamp
   - action type
   - affected resource id
3. If unauthorized:
   - rotate the account password
   - rotate `IPTV_JWT_SECRET` if broader compromise is suspected
   - document impacted schedules/tasks before rollback

### WebSocket or live status missing

1. Confirm the Tasks page shows `实时同步中`.
2. If not:
   - re-login to refresh token/session
   - inspect reverse proxy websocket forwarding
   - verify backend `/ws` access and auth handling
3. Run one manual recording and confirm the task list updates again.

## Related Documents

- `docs/release-checklist.md`
- `docs/security-operations.md`
- `docs/deployment.md`
