#!/bin/sh
# IPTV Recorder 容器入口脚本 —— PUID/PGID 动态用户切换
#
# 参照 jellyfin/sonarr/Transmission 等成熟方案:容器以 root 启动,
# 启动时读取 PUID/PGID 环境变量,动态调整 app 用户的 uid/gid,
# chown /app 目录后用 su-exec 切到该用户执行主程序。
#
# 这样容器内进程的 uid/gid 与宿主当前用户对齐,挂载进来的卷都能读写,
# 根除"app 用户写不进 root 属主目录 → SQLite code 14"的权限崩溃。
#
# 用法(compose 里):
#   environment:
#     PUID: 1000    # 宿主用户 uid(NAS 上执行 `id -u` 查看)
#     PGID: 1000    # 宿主用户 gid(NAS 上执行 `id -g` 查看)
set -e

PUID="${PUID:-1000}"
PGID="${PGID:-1000}"

echo "[entrypoint] 配置运行用户: PUID=$PUID PGID=$PGID"

# 校验 PUID/PGID 是合法正整数(防注入 + 防误填)
case "$PUID" in
    ''|*[!0-9]*) echo "[entrypoint] 错误: PUID 必须是正整数,当前='$PUID'"; exit 1 ;;
esac
case "$PGID" in
    ''|*[!0-9]*) echo "[entrypoint] 错误: PGID 必须是正整数,当前='$PGID'"; exit 1 ;;
esac

# ===== 1. 调整 GID =====
CURRENT_GID=$(id -g app 2>/dev/null || echo "none")
if [ "$CURRENT_GID" != "$PGID" ]; then
    # 若目标 GID 已被其他组占用,先调整那个组让出
    CONFLICT_GROUP=$(getent group "$PGID" | cut -d: -f1 || true)
    if [ -n "$CONFLICT_GROUP" ] && [ "$CONFLICT_GROUP" != "app" ]; then
        # 让出 GID:给冲突组分配一个临时大 GID
        groupmod -g 65534 "$CONFLICT_GROUP" 2>/dev/null || true
    fi
    # 重建 app 组为目标 GID
    delgroup app 2>/dev/null || true
    addgroup -g "$PGID" -S app
fi

# ===== 2. 调整 UID =====
CURRENT_UID=$(id -u app 2>/dev/null || echo "none")
if [ "$CURRENT_UID" != "$PUID" ]; then
    # 若目标 UID 已被其他用户占用,先调整那个用户
    CONFLICT_USER=$(getent passwd "$PUID" | cut -d: -f1 || true)
    if [ -n "$CONFLICT_USER" ] && [ "$CONFLICT_USER" != "app" ]; then
        usermod -u 65534 "$CONFLICT_USER" 2>/dev/null || true
    fi
    deluser app 2>/dev/null || true
    adduser -u "$PUID" -G app -S -D -H -s /sbin/nologin app
fi

# ===== 3. 确保 /app 及子目录属主正确(含挂载的 data 卷) =====
# chown 可能较慢(大库/多录制文件),但对保证权限必要。
# 只 chown /app,不碰其他系统目录。
chown -R "$PUID:$PGID" /app 2>/dev/null || {
    echo "[entrypoint] 警告: chown /app 部分失败(可能有只读挂载),继续启动"
}

echo "[entrypoint] 用户配置完成: app(uid=$(id -u app), gid=$(id -g app))"
echo "[entrypoint] 启动主程序: $@"

# ===== 4. 切到 app 用户执行(exec 让信号正确传递到主进程) =====
exec su-exec app "$@"
