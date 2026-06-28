# Frontend dev image – source code is mounted at runtime, NOT baked in.
# Only base deps (node + pnpm + container node_modules) live in the image,
# so editing files on the host instantly hot-reloads inside the container.

FROM node:20-alpine

# Use Chinese npm mirror to speed up installs in CN networks.
# Use pnpm 10 (supports lockfile v9.0, compatible with Node 20).
# pnpm 11.x requires Node 22.5+ because it uses the node:sqlite built-in,
# and node:22-alpine cannot be pulled here (Docker Hub blocked).
RUN npm config set registry https://registry.npmmirror.com \
    && corepack enable \
    && corepack prepare pnpm@10.18.0 --activate

WORKDIR /app

# Install deps into a container-owned node_modules.
# lockfile + package.json are mounted at runtime, but we also pre-install
# here so first start is fast and the image is usable standalone.
COPY package.json pnpm-lock.yaml ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile

# Source code is provided via volume mount in docker-compose.
# The CMD runs the dev server bound to all interfaces for HMR access.
ENV CI=1
EXPOSE 5173
CMD ["pnpm", "dev", "--host", "0.0.0.0"]
