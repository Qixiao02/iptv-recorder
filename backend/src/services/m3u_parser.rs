//! M3U 解析器模块
//!
//! 支持 M3U/M3U8 文件解析，提取频道信息

#![allow(dead_code)]

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

static EXTINF_RE: OnceLock<Regex> = OnceLock::new();
static TVG_ID_RE: OnceLock<Regex> = OnceLock::new();
static TVG_NAME_RE: OnceLock<Regex> = OnceLock::new();
static TVG_LOGO_RE: OnceLock<Regex> = OnceLock::new();
static GROUP_RE: OnceLock<Regex> = OnceLock::new();
static URL_RE: OnceLock<Regex> = OnceLock::new();

fn extinf_re() -> &'static Regex {
    EXTINF_RE.get_or_init(|| Regex::new(r"#EXTINF:-?\d*\s*(.*)").expect("invalid regex"))
}
fn tvg_id_re() -> &'static Regex {
    TVG_ID_RE.get_or_init(|| Regex::new(r#"tvg-id="([^"]*)""#).expect("invalid regex"))
}
fn tvg_name_re() -> &'static Regex {
    TVG_NAME_RE.get_or_init(|| Regex::new(r#"tvg-name="([^"]*)""#).expect("invalid regex"))
}
fn tvg_logo_re() -> &'static Regex {
    TVG_LOGO_RE.get_or_init(|| Regex::new(r#"tvg-logo="([^"]*)""#).expect("invalid regex"))
}
fn group_re() -> &'static Regex {
    GROUP_RE.get_or_init(|| Regex::new(r#"group-title="([^"]*)""#).expect("invalid regex"))
}
fn url_re() -> &'static Regex {
    URL_RE.get_or_init(|| Regex::new(r"^(https?://[^\s]+)").expect("invalid regex"))
}

/// M3U 频道信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M3UChannel {
    /// 频道名称
    pub name: String,
    /// 频道流地址
    pub url: String,
    /// 频道分组
    pub group: String,
    /// 频道 Logo
    pub logo: Option<String>,
    /// EPG ID
    pub tvg_id: Option<String>,
    /// EPG 名称
    pub tvg_name: Option<String>,
    /// 其他扩展属性
    pub attrs: Vec<(String, String)>,
}

/// M3U 解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M3UParseResult {
    /// 解析出的频道列表
    pub channels: Vec<M3UChannel>,
    /// 总频道数
    pub total: usize,
    /// 成功解析数
    pub successful: usize,
    /// 失败数
    pub failed: usize,
    /// 解析错误信息
    pub errors: Vec<String>,
}

/// M3U 解析器
pub struct M3UParser;

impl M3UParser {
    /// 从 URL 解析 M3U 文件
    pub async fn from_url(url: &str) -> Result<M3UParseResult> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let response = client
            .get(url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await
            .context("Failed to fetch M3U file")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP error: {}", response.status());
        }

        let content = response
            .text()
            .await
            .context("Failed to read M3U content")?;

        Self::parse(&content)
    }

    /// 从本地文件解析 M3U
    pub async fn from_file(path: &str) -> Result<M3UParseResult> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context("Failed to read M3U file")?;

        Self::parse(&content)
    }

    /// 从内容字符串解析 M3U
    pub fn parse(content: &str) -> Result<M3UParseResult> {
        let mut result = M3UParseResult {
            channels: Vec::new(),
            total: 0,
            successful: 0,
            failed: 0,
            errors: Vec::new(),
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        // 使用静态编译的正则表达式
        let extinf_re = extinf_re();
        let tvg_id_re = tvg_id_re();
        let tvg_name_re = tvg_name_re();
        let tvg_logo_re = tvg_logo_re();
        let group_re = group_re();
        let url_re = url_re();

        // 检查是否是有效的 M3U 文件
        if lines.is_empty() {
            result.errors.push("文件内容为空".to_string());
            return Ok(result);
        }

        // 第一行应该是 #EXTM3U
        let first_line = lines[0].trim();
        if !first_line.eq_ignore_ascii_case("#EXTM3U") && !first_line.starts_with("#EXTM3U") {
            result.errors.push(format!(
                "无效的 M3U 文件，第一行应为 #EXTM3U，实际: {}",
                first_line
            ));
            // 继续尝试解析
        }

        let mut current_attrs: Vec<(String, String)> = Vec::new();
        let mut current_name: Option<String> = None;

        while i < lines.len() {
            let line = lines[i].trim();
            i += 1;

            if line.is_empty() || line.starts_with("#EXTVLCOPT") {
                continue;
            }

            if line.starts_with("#EXTINF:") {
                // 解析 #EXTINF 行
                let info = extinf_re
                    .captures(line)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str())
                    .unwrap_or("");

                // 提取名称（通常是逗号后的部分）
                let name = if let Some(pos) = info.find(',') {
                    info[pos + 1..].trim().to_string()
                } else {
                    info.to_string()
                };

                if !name.is_empty() {
                    current_name = Some(name);
                }

                // 提取属性
                current_attrs.clear();
                for cap in tvg_id_re.captures_iter(line) {
                    if let Some(id) = cap.get(1) {
                        current_attrs.push(("tvg-id".to_string(), id.as_str().to_string()));
                    }
                }
                for cap in tvg_name_re.captures_iter(line) {
                    if let Some(name) = cap.get(1) {
                        current_attrs.push(("tvg-name".to_string(), name.as_str().to_string()));
                    }
                }
                for cap in tvg_logo_re.captures_iter(line) {
                    if let Some(logo) = cap.get(1) {
                        current_attrs.push(("tvg-logo".to_string(), logo.as_str().to_string()));
                    }
                }
                for cap in group_re.captures_iter(line) {
                    if let Some(group) = cap.get(1) {
                        current_attrs.push(("group-title".to_string(), group.as_str().to_string()));
                    }
                }
            } else if !line.starts_with('#') {
                // 这是一个 URL 行
                result.total += 1;

                let url = if let Some(cap) = url_re.captures(line) {
                    cap.get(1).map(|m| m.as_str()).unwrap_or(line).to_string()
                } else {
                    line.to_string()
                };

                // 严格校验 URL scheme：仅接受 http/https，拒绝 file://、本地绝对路径等，
                // 防止 SSRF —— 不允许把本地文件路径当 channel URL 注入后被 FFmpeg 读取。
                if !url.is_empty() {
                    match url::Url::parse(&url) {
                        Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => {
                            let channel = Self::build_channel(
                                current_name
                                    .clone()
                                    .unwrap_or_else(|| "Unknown".to_string()),
                                url,
                                &current_attrs,
                            );

                            result.channels.push(channel);
                            result.successful += 1;
                        }
                        _ => {
                            result.failed += 1;
                            result.errors.push(format!("不支持的 URL scheme: {}", url));
                        }
                    }
                } else {
                    result.failed += 1;
                    result.errors.push(format!("无效的 URL: {}", url));
                }

                // 重置状态
                current_name = None;
                current_attrs.clear();
            }
        }

        Ok(result)
    }

    /// 构建频道信息
    fn build_channel(name: String, url: String, attrs: &[(String, String)]) -> M3UChannel {
        let tvg_id = attrs
            .iter()
            .find(|(k, _)| k == "tvg-id")
            .map(|(_, v)| v.clone());

        let tvg_name = attrs
            .iter()
            .find(|(k, _)| k == "tvg-name")
            .map(|(_, v)| v.clone());

        let logo = attrs
            .iter()
            .find(|(k, _)| k == "tvg-logo")
            .map(|(_, v)| v.clone());

        let group = attrs
            .iter()
            .find(|(k, _)| k == "group-title")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "Uncategorized".to_string());

        M3UChannel {
            name,
            url,
            group,
            logo,
            tvg_id,
            tvg_name,
            attrs: attrs.to_vec(),
        }
    }

    /// 从多个 URL 合并解析
    pub async fn from_urls(urls: &[String]) -> Result<M3UParseResult> {
        let mut combined = M3UParseResult {
            channels: Vec::new(),
            total: 0,
            successful: 0,
            failed: 0,
            errors: Vec::new(),
        };

        for url in urls {
            match Self::from_url(url).await {
                Ok(mut result) => {
                    combined.total += result.total;
                    combined.successful += result.successful;
                    combined.failed += result.failed;
                    combined.channels.append(&mut result.channels);
                    combined.errors.append(&mut result.errors);
                }
                Err(e) => {
                    combined.failed += 1;
                    combined.errors.push(format!("解析 {} 失败: {}", url, e));
                }
            }
        }

        Ok(combined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_m3u() {
        let content = r#"#EXTM3U
#EXTINF:-1,CCTV-1
http://example.com/cctv1.m3u8
#EXTINF:-1,CCTV-2
http://example.com/cctv2.m3u8
"#;

        let result = M3UParser::parse(content).unwrap();
        assert_eq!(result.channels.len(), 2);
        assert_eq!(result.channels[0].name, "CCTV-1");
        assert_eq!(result.channels[1].name, "CCTV-2");
    }

    #[test]
    fn test_parse_m3u_with_attrs() {
        let content = r#"#EXTM3U
#EXTINF:-1 tvg-id="cctv1" tvg-logo="http://example.com/logo.png" group-title="央视",CCTV-1
http://example.com/cctv1.m3u8
"#;

        let result = M3UParser::parse(content).unwrap();
        assert_eq!(result.channels.len(), 1);
        assert_eq!(result.channels[0].name, "CCTV-1");
        assert_eq!(result.channels[0].tvg_id.as_ref().unwrap(), "cctv1");
        assert_eq!(
            result.channels[0].logo.as_ref().unwrap(),
            "http://example.com/logo.png"
        );
        assert_eq!(result.channels[0].group, "央视");
    }

    /// 本地绝对路径（如 `/etc/passwd`）必须被拒绝，防止被 FFmpeg 当 file:// 读取（SSRF）。
    #[test]
    fn rejects_local_path() {
        let content = r#"#EXTM3U
#EXTINF:-1,Evil
/etc/passwd
"#;

        let result = M3UParser::parse(content).unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.successful, 0);
        assert_eq!(result.failed, 1);
        assert!(result.channels.is_empty());
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("不支持的 URL scheme") && e.contains("/etc/passwd")),
            "errors: {:?}",
            result.errors
        );
    }

    /// `file://` scheme 必须被拒绝。
    #[test]
    fn rejects_file_scheme() {
        let content = r#"#EXTM3U
#EXTINF:-1,Evil
file:///etc/shadow
"#;

        let result = M3UParser::parse(content).unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.successful, 0);
        assert_eq!(result.failed, 1);
        assert!(result.channels.is_empty());
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("不支持的 URL scheme") && e.contains("file:///etc/shadow")),
            "errors: {:?}",
            result.errors
        );
    }

    /// 正常 `https://` URL 必须被接受。
    #[test]
    fn accepts_https() {
        let content = r#"#EXTM3U
#EXTINF:-1,Valid
https://example.com/playlist.m3u8
"#;

        let result = M3UParser::parse(content).unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.successful, 1);
        assert_eq!(result.failed, 0);
        assert_eq!(result.channels.len(), 1);
        assert_eq!(result.channels[0].name, "Valid");
        assert_eq!(result.channels[0].url, "https://example.com/playlist.m3u8");
    }
}
