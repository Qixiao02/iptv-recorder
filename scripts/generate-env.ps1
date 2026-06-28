# 生成 .env 文件(首次部署运行一次)
# - IPTV_JWT_SECRET: 随机 64 位十六进制(32 字节)
# - IPTV_INITIAL_ADMIN_PASSWORD: 随机 16 位密码
#
# 用法: powershell -ExecutionPolicy Bypass -File scripts/generate-env.ps1
# 已存在 .env 时不会覆盖。

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$envFile = Join-Path $root ".env"

if (Test-Path $envFile) {
    Write-Host ".env 已存在,跳过生成(如需重新生成请先删除它)" -ForegroundColor Yellow
    exit 0
}

# 生成随机 JWT 密钥(32 字节 = 64 位十六进制)
$jwtSecret = -join ((1..32) | ForEach-Object {
    '{0:x2}' -f (Get-Random -Maximum 256)
})

# 生成随机初始管理员密码(16 位,字母数字)
$chars = "abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789"
$adminPwd = -join ((1..16) | ForEach-Object { $chars[(Get-Random -Maximum $chars.Length)] })

$content = @"
# IPTV Recorder 密钥与初始密码(自动生成,请勿提交到 git)
# 生成时间: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")

# JWT 签名密钥(至少 32 字符)。更换后所有已签发的 token 立即失效。
IPTV_JWT_SECRET=$jwtSecret

# 初始管理员密码(首次启动创建 admin 账号时使用,登录后请立即修改)。
# 不设置则后端自动生成随机密码并记录到日志。
IPTV_INITIAL_ADMIN_PASSWORD=$adminPwd
"@

Set-Content -Path $envFile -Value $content -Encoding UTF8
Write-Host "已生成 $envFile" -ForegroundColor Green
Write-Host "  初始管理员密码: $adminPwd" -ForegroundColor Cyan
Write-Host "  请妥善保存,登录后立即在「账户」页修改密码" -ForegroundColor Yellow
