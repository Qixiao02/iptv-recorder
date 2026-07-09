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
# 注意:不全局 set -e —— 用户/组调整的边界情况用降级处理,
# 避免某一步非致命失败导致容器崩溃循环。

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

# ===== 1. 调整 GID(优先 groupmod,失败则降级) =====
CURRENT_GID=$(id -g app 2>/dev/null || echo "none")
if [ "$CURRENT_GID" != "$PGID" ]; then
    if groupmod -g "$PGID" app 2>/dev/null; then
        : # 成功
    else
        # groupmod 失败(可能目标 GID 被占):先让占用的组让出,再 mod
        CONFLICT_GROUP=$(getent group "$PGID" | cut -d: -f1 || true)
        if [ -n "$CONFLICT_GROUP" ] && [ "$CONFLICT_GROUP" != "app" ]; then
            groupmod -g 65534 "$CONFLICT_GROUP" 2>/dev/null || true
            groupmod -g "$PGID" app 2>/dev/null || true
        fi
        # 若 groupmod 仍不可用(BusyBox 极简),直接改 /etc/group
        if [ "$(id -g app 2>/dev/null)" != "$PGID" ]; then
            sed -i "s/^app:x:[0-9]*:/app:x:$PGID:/" /etc/group
        fi
    fi
    echo "[entrypoint] app 组 GID: $CURRENT_GID -> $(id -g app 2>/dev/null)"
fi

# ===== 2. 调整 UID(优先 usermod,失败则降级) =====
CURRENT_UID=$(id -u app 2>/dev/null || echo "none")
if [ "$CURRENT_UID" != "$PUID" ]; then
    if usermod -u "$PUID" -g app app 2>/dev/null; then
        : # 成功
    else
        # usermod 失败(目标 UID 被占):让占用者让出
        CONFLICT_USER=$(getent passwd "$PUID" | cut -d: -f1 || true)
        if [ -n "$CONFLICT_USER" ] && [ "$CONFLICT_USER" != "app" ]; then
            usermod -u 65534 "$CONFLICT_USER" 2>/dev/null || true
            usermod -u "$PUID" -g app app 2>/dev/null || true
        fi
        # 降级:直接改 /etc/passwd
        if [ "$(id -u app 2>/dev/null)" != "$PUID" ]; then
            sed -i "s/^app:x:[0-9]*:[0-9]*:/app:x:$PUID:$PGID:/" /etc/passwd
        fi
    fi
    echo "[entrypoint] app 用户 UID: $CURRENT_UID -> $(id -u app 2>/dev/null)"
fi

# ===== 3. 确保 /app 及子目录属主正确(含挂载的 data 卷) =====
# chown 可能较慢(大库/多录制文件),但对保证权限必要。
# 只 chown /app,不碰其他系统目录。失败不致命(挂载点可能只读)。
chown -R "$PUID:$PGID" /app 2>/dev/null || {
    echo "[entrypoint] 警告: chown /app 部分失败(可能有只读挂载),继续启动"
}

echo "[entrypoint] 用户配置完成: app(uid=$(id -u app 2>/dev/null), gid=$(id -g app 2>/dev/null))"

# ===== 4. 切到 app 用户执行(exec 让信号正确传递到主进程) =====
exec su-exec app "$@"
