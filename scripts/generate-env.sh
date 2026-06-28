#!/usr/bin/env bash
# 生成 .env 文件(首次部署运行一次)
# - IPTV_JWT_SECRET: 随机 64 位十六进制(32 字节)
# - IPTV_INITIAL_ADMIN_PASSWORD: 随机 16 位密码
#
# 用法: bash scripts/generate-env.sh
# 已存在 .env 时不会覆盖。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="$ROOT/.env"

if [ -f "$ENV_FILE" ]; then
  echo ".env 已存在,跳过生成(如需重新生成请先删除它)"
  exit 0
fi

JWT_SECRET="$(openssl rand -hex 32)"
ADMIN_PWD="$(openssl rand -base64 12 | tr -d '/+=' | head -c 16)"

cat > "$ENV_FILE" <<EOF
# IPTV Recorder 密钥与初始密码(自动生成,请勿提交到 git)
# 生成时间: $(date '+%Y-%m-%d %H:%M:%S')

# JWT 签名密钥(至少 32 字符)。更换后所有已签发的 token 立即失效。
IPTV_JWT_SECRET=$JWT_SECRET

# 初始管理员密码(首次启动创建 admin 账号时使用,登录后请立即修改)。
# 不设置则后端自动生成随机密码并记录到日志。
IPTV_INITIAL_ADMIN_PASSWORD=$ADMIN_PWD
EOF

echo "已生成 $ENV_FILE"
echo "  初始管理员密码: $ADMIN_PWD"
echo "  请妥善保存,登录后立即在「账户」页修改密码"
