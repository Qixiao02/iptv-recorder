# Security Operations

## First Deployment

1. set `IPTV_JWT_SECRET` to a random secret with at least 32 characters
2. optionally set `IPTV_INITIAL_ADMIN_PASSWORD`
3. start the backend and capture the bootstrap admin password if a one-time password is generated
4. log in immediately and change the admin password

## Password Rotation

1. sign in with an admin account
2. change the password from the Settings page
3. confirm the old password no longer works
4. update password vault records

## JWT Secret Rotation

1. notify users that existing sessions will be invalidated
2. stop new deploy traffic or enter maintenance mode
3. set a new `IPTV_JWT_SECRET`
4. restart backend services
5. validate login, WebSocket auth and stream proxy auth

## Incident Response

- for unauthorized proxy attempts, review application logs and block the source upstream if needed
- for suspected credential leakage, rotate admin passwords and `IPTV_JWT_SECRET`
- for suspicious recordings or schedule changes, review recent operator actions and scheduler logs
