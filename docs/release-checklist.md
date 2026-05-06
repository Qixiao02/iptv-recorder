# Release Checklist

## Build And Test

- [ ] `IPTV_JWT_SECRET=... cargo test`
- [ ] `IPTV_JWT_SECRET=... cargo check`
- [ ] `pnpm test`
- [ ] `pnpm build`
- [ ] `git diff --check`

## Security

- [ ] `IPTV_JWT_SECRET` is set and length >= 32
- [ ] `IPTV_INITIAL_ADMIN_PASSWORD` is set for first-time deployment, or one-time password handling is documented
- [ ] production config does not expose debug endpoints
- [ ] proxy target restrictions are verified in staging

## Operations

- [ ] scheduler reload tested in staging
- [ ] recording executable path verified from Settings and runtime config
- [ ] auto-cleanup retention checked against business expectations
- [ ] disk free space alert threshold validated

## Rollback

- [ ] previous backend binary or image is available
- [ ] previous frontend bundle is available
- [ ] database backup taken before schema-affecting release

## Post Release

- [ ] login, channel list, schedule list and task list smoke tested
- [ ] one manual recording succeeds end-to-end
- [ ] one scheduled recording fires as expected
- [ ] WebSocket task updates and system alerts are visible in UI
