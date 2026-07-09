# ── Stage 1: 前端构建 ──────────────────────────────────────────
# 产物 dist 由后端 ServeDir::new("static") 在 /static 路径下服务
FROM node:20-alpine AS frontend

WORKDIR /build

# 国内 npm 镜像加速;pnpm 10 兼容 Node 20(pnpm 11 需 Node 22.5+)
RUN npm config set registry https://registry.npmmirror.com \
    && corepack enable \
    && corepack prepare pnpm@10.18.0 --activate

# 先 COPY 锁文件以利用 Docker 层缓存
COPY frontend/package.json frontend/pnpm-lock.yaml ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile

COPY frontend/ .
RUN pnpm build
# 产物在 /build/dist (index.html + assets/ + logo.png + vite.svg)


# ── Stage 2: 后端(Rust)构建 ───────────────────────────────────
FROM node:20-alpine AS builder

WORKDIR /build

RUN sed -i 's#https://dl-cdn.alpinelinux.org/alpine#https://mirrors.aliyun.com/alpine#g' /etc/apk/repositories \
    && apk add --no-cache cargo rust musl-dev pkgconfig openssl-dev sqlite-dev

RUN mkdir -p /root/.cargo \
    && printf '%s\n' \
        '[source.crates-io]' \
        'replace-with = "rsproxy-sparse"' \
        '' \
        '[source.rsproxy-sparse]' \
        'registry = "sparse+https://rsproxy.cn/index/"' \
        '' \
        '[net]' \
        'git-fetch-with-cli = true' \
        > /root/.cargo/config.toml

COPY backend/Cargo.toml backend/Cargo.lock ./backend/
COPY backend/migrations ./backend/migrations
COPY backend/src ./backend/src

WORKDIR /build/backend
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/build/backend/target \
    cargo build --release \
    && cp /build/backend/target/release/iptv-recorder /tmp/iptv-recorder

FROM node:20-alpine AS runtime

ARG N_M3U8DL_RE_VERSION=0.5.1-beta
ARG N_M3U8DL_RE_DATE=20251029
ARG N_M3U8DL_RE_SHA256=7105e26b76b099b41fcd490b9d09b3d43be971a880b6323fb988b688be00ab82

RUN sed -i 's#https://dl-cdn.alpinelinux.org/alpine#https://mirrors.aliyun.com/alpine#g' /etc/apk/repositories \
    && apk add --no-cache ca-certificates curl ffmpeg libgcc openssl sqlite sqlite-libs su-exec tar \
    && curl -fsSL \
        "https://github.com/nilaoda/N_m3u8DL-RE/releases/download/v${N_M3U8DL_RE_VERSION}/N_m3u8DL-RE_v${N_M3U8DL_RE_VERSION}_linux-musl-x64_${N_M3U8DL_RE_DATE}.tar.gz" \
        -o /tmp/N_m3u8DL-RE.tar.gz \
    && echo "${N_M3U8DL_RE_SHA256}  /tmp/N_m3u8DL-RE.tar.gz" | sha256sum -c - \
    && tar -xzf /tmp/N_m3u8DL-RE.tar.gz -C /usr/local/bin N_m3u8DL-RE \
    && chmod +x /usr/local/bin/N_m3u8DL-RE \
    && rm -f /tmp/N_m3u8DL-RE.tar.gz

WORKDIR /app

COPY --from=builder /tmp/iptv-recorder /app/iptv-recorder
COPY backend/config/default.toml /app/config/default.toml

# 前端构建产物:后端工作目录为 /app,ServeDir::new("static") 解析为 /app/static
COPY --from=frontend /build/dist /app/static

RUN mkdir -p /app/data/recordings /app/data/.tmp \
    && addgroup -S app && adduser -S -G app app \
    && chown -R app:app /app

# PUID/PGID 动态用户入口脚本(entrypoint 启动时按环境变量调整 app 用户 uid/gid)
COPY docker/entrypoint.sh /docker/entrypoint.sh
RUN chmod +x /docker/entrypoint.sh

ENV IPTV__SERVER__HOST=0.0.0.0 \
    IPTV__SERVER__PORT=3000 \
    IPTV__DATABASE__PATH=/app/data/iptv-recorder.db \
    IPTV__STORAGE__RECORDINGS_DIR=/app/data/recordings \
    IPTV__STORAGE__TEMP_DIR=/app/data/.tmp \
    IPTV__STORAGE__PREVIEW_TEMP_DIR=/dev/shm/iptv-recorder-hls \
    IPTV__RECORDER__EXECUTABLE=N_m3u8DL-RE \
    IPTV__RECORDER__POST_PROCESS__FFMPEG_PATH=ffmpeg \
    # PUID/PGID 默认 1000:1000(NAS 上用 `id` 看自己的 uid/gid 填入 .env)
    PUID=1000 \
    PGID=1000

# 注意:不设 USER —— 容器以 root 启动,entrypoint 内部 chown 后用 su-exec 切到 app
EXPOSE 3000

ENTRYPOINT ["/docker/entrypoint.sh"]
CMD ["/app/iptv-recorder"]
