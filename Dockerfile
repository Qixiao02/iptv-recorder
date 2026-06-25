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
    && apk add --no-cache ca-certificates curl ffmpeg libgcc openssl sqlite sqlite-libs tar \
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

RUN mkdir -p /app/data/recordings /app/data/.tmp

ENV IPTV__SERVER__HOST=0.0.0.0 \
    IPTV__SERVER__PORT=3000 \
    IPTV__DATABASE__PATH=/app/data/iptv-recorder.db \
    IPTV__STORAGE__RECORDINGS_DIR=/app/data/recordings \
    IPTV__STORAGE__TEMP_DIR=/app/data/.tmp \
    IPTV__STORAGE__PREVIEW_TEMP_DIR=/dev/shm/iptv-recorder-hls \
    IPTV__RECORDER__EXECUTABLE=N_m3u8DL-RE \
    IPTV__RECORDER__POST_PROCESS__FFMPEG_PATH=ffmpeg

EXPOSE 3000

CMD ["/app/iptv-recorder"]
