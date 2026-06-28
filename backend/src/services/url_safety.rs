//! URL 安全校验(SSRF 防护)
//!
//! 提供 scheme 白名单、主机名/私有 IP 拦截、DNS 解析后查私有 IP 等能力。
//! 服务层(channel/epg 导入)与 API 层(流代理)共用,避免校验逻辑分散。

use anyhow::Result;
use std::net::IpAddr;

/// 校验 URL 是否安全(严格模式):拦截非 http(s)、内网主机名、私有 IP。
///
/// 适用于频道导入、EPG 导入等"用户提供的 URL 会被服务端抓取"的场景。
/// 内网地址(localhost、私有 IP 段、云元数据 169.254.x.x 等)一律拒绝。
pub async fn assert_safe_url(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url).map_err(|e| anyhow::anyhow!("无效的 URL: {}", e))?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            anyhow::bail!("仅允许 HTTP/HTTPS 地址,不支持 {}", other);
        }
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("URL 缺少主机名"))?;

    if is_disallowed_hostname(host) {
        anyhow::bail!("不允许使用本地或内网地址: {}", host);
    }

    let port = parsed.port_or_known_default().unwrap_or(80);
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| anyhow::anyhow!("解析地址失败: {}", e))?;

    for address in addresses {
        if is_private_ip(address.ip()) {
            anyhow::bail!("不允许使用内网地址: {}", address.ip());
        }
    }

    Ok(())
}

/// 宽松校验:只校验 scheme(http/https),不拦截内网地址。
///
/// 适用于「私有服务器」频道场景(源本身就是内网流),但仍需防止 file:// 等危险 scheme。
pub fn assert_safe_url_scheme_only(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url).map_err(|e| anyhow::anyhow!("无效的 URL: {}", e))?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        other => anyhow::bail!("仅允许 HTTP/HTTPS 地址,不支持 {}", other),
    }
}

/// 判断主机名是否属于禁用域(本地/内网域名)
pub fn is_disallowed_hostname(host: &str) -> bool {
    let normalized = host.trim().trim_end_matches('.').to_ascii_lowercase();
    normalized == "localhost"
        || normalized.ends_with(".localhost")
        || normalized.ends_with(".local")
        || normalized.ends_with(".internal")
        || normalized.ends_with(".lan")
        || normalized.ends_with(".home")
        || normalized.ends_with(".corp")
        || normalized.ends_with(".box")
}

/// 判断 IP 是否属于私有/内网/保留地址段
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                // 文档地址 2001:db8::/32
                || (v6.segments()[0] == 0x2001 && v6.segments()[1] == 0x0db8)
        }
    }
}
