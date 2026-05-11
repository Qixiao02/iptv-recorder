//! 后处理服务
//!
//! 支持实时转码和后期转码

use crate::config::PostProcessConfig;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};

/// 转码模式
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TranscodeMode {
    /// 不转码
    Off,
    /// 实时转码
    Realtime,
    /// 后期转码
    Post,
}

impl From<&str> for TranscodeMode {
    fn from(s: &str) -> Self {
        match s {
            "realtime" => TranscodeMode::Realtime,
            "post" => TranscodeMode::Post,
            _ => TranscodeMode::Off,
        }
    }
}

/// 后处理器
#[derive(Clone)]
pub struct PostProcessor {
    config: PostProcessConfig,
    recordings_dir: PathBuf,
}

impl PostProcessor {
    pub fn new(config: PostProcessConfig, recordings_dir: PathBuf) -> Self {
        Self {
            config,
            recordings_dir,
        }
    }

    /// 获取转码模式
    #[allow(dead_code)]
    pub fn mode(&self) -> TranscodeMode {
        TranscodeMode::from(self.config.mode.as_str())
    }

    /// 是否启用转码
    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled()
    }

    /// 是否实时转码
    #[allow(dead_code)]
    pub fn is_realtime(&self) -> bool {
        self.config.is_realtime()
    }

    /// 是否后期转码
    pub fn is_post(&self) -> bool {
        self.config.is_post()
    }

    /// 获取 FFmpeg 路径
    pub fn ffmpeg_path(&self) -> String {
        if self.config.ffmpeg_path.is_empty() {
            "ffmpeg".to_string()
        } else {
            self.config.ffmpeg_path.clone()
        }
    }

    /// 启动实时转码录制
    /// 返回 FFmpeg 子进程
    #[allow(dead_code)]
    pub fn start_realtime_recording(
        &self,
        url: &str,
        output_path: &Path,
        duration_seconds: Option<u64>,
    ) -> Result<Child> {
        let ffmpeg_path = self.ffmpeg_path();
        let args = self.build_realtime_args(url, output_path, duration_seconds)?;

        info!("🎬 启动实时转码录制: {} {:?}", ffmpeg_path, args);
        debug!("FFmpeg 命令: {} {:?}", ffmpeg_path, args);

        let child = Command::new(&ffmpeg_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("启动 FFmpeg 失败: {}", e))?;

        Ok(child)
    }

    /// 构建实时转码参数
    #[allow(dead_code)]
    fn build_realtime_args(
        &self,
        url: &str,
        output_path: &Path,
        duration_seconds: Option<u64>,
    ) -> Result<Vec<String>> {
        let mut args = vec!["-y".to_string(), "-i".to_string(), url.to_string()];

        // 添加时长限制
        if let Some(duration) = duration_seconds {
            args.extend(vec!["-t".to_string(), duration.to_string()]);
        }

        // 添加转码参数
        self.add_encoding_args(&mut args);

        // 输出文件
        args.push(output_path.to_string_lossy().to_string());

        Ok(args)
    }

    /// 后期转码处理
    pub async fn process(&self, input_path: &Path, _task_id: &str) -> Result<PathBuf> {
        if !self.is_post() {
            return Ok(input_path.to_path_buf());
        }

        // 检查输入文件是否存在
        if !input_path.exists() {
            warn!("输入文件不存在，跳过后处理: {:?}", input_path);
            return Ok(input_path.to_path_buf());
        }

        info!("🎬 开始后期转码: {:?}", input_path);

        // 生成输出文件路径
        let output_path = self.get_output_path(input_path)?;
        let ffmpeg_path = self.ffmpeg_path();

        // 构建参数
        let mut args = vec![
            "-y".to_string(),
            "-i".to_string(),
            input_path.to_string_lossy().to_string(),
        ];

        // 添加转码参数
        self.add_encoding_args(&mut args);

        // 输出文件
        args.push(output_path.to_string_lossy().to_string());

        debug!("FFmpeg 命令: {} {:?}", ffmpeg_path, args);

        // 执行转码
        let status = Command::new(&ffmpeg_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .status()
            .await
            .map_err(|e| anyhow!("执行 FFmpeg 失败: {}", e))?;

        if !status.success() {
            error!("FFmpeg 转码失败: {:?}", input_path);
            return Err(anyhow!("FFmpeg 转码失败，退出码: {:?}", status.code()));
        }

        // 检查输出文件是否存在
        if !output_path.exists() {
            return Err(anyhow!("转码完成但输出文件不存在"));
        }

        info!("✅ 后期转码完成: {:?}", output_path);

        // 删除原始文件
        if self.config.delete_original && input_path != output_path {
            match tokio::fs::remove_file(input_path).await {
                Ok(_) => info!("已删除原始文件: {:?}", input_path),
                Err(e) => warn!("删除原始文件失败: {}", e),
            }
        }

        Ok(output_path)
    }

    /// 添加编码参数
    fn add_encoding_args(&self, args: &mut Vec<String>) {
        match self.config.preset.as_str() {
            "high" => {
                args.extend(vec![
                    "-c:v".to_string(),
                    "libx264".to_string(),
                    "-crf".to_string(),
                    "18".to_string(),
                    "-preset".to_string(),
                    "slow".to_string(),
                    "-c:a".to_string(),
                    "aac".to_string(),
                    "-b:a".to_string(),
                    "192k".to_string(),
                ]);
            }
            "medium" => {
                args.extend(vec![
                    "-c:v".to_string(),
                    "libx264".to_string(),
                    "-crf".to_string(),
                    "23".to_string(),
                    "-preset".to_string(),
                    "medium".to_string(),
                    "-c:a".to_string(),
                    "aac".to_string(),
                    "-b:a".to_string(),
                    "128k".to_string(),
                ]);
            }
            "low" => {
                args.extend(vec![
                    "-c:v".to_string(),
                    "libx264".to_string(),
                    "-crf".to_string(),
                    "28".to_string(),
                    "-preset".to_string(),
                    "fast".to_string(),
                    "-c:a".to_string(),
                    "aac".to_string(),
                    "-b:a".to_string(),
                    "96k".to_string(),
                ]);
            }
            "copy" => {
                args.extend(vec!["-c".to_string(), "copy".to_string()]);
            }
            "custom" => {
                args.extend(vec!["-c:v".to_string(), "libx264".to_string()]);

                if !self.config.video_bitrate.is_empty() {
                    args.extend(vec!["-b:v".to_string(), self.config.video_bitrate.clone()]);
                } else {
                    args.extend(vec!["-crf".to_string(), self.config.crf.to_string()]);
                }

                args.extend(vec![
                    "-preset".to_string(),
                    self.config.encode_preset.clone(),
                    "-c:a".to_string(),
                    "aac".to_string(),
                    "-b:a".to_string(),
                    self.config.audio_bitrate.clone(),
                ]);

                if !self.config.custom_args.is_empty() {
                    let custom: Vec<String> = self
                        .config
                        .custom_args
                        .split_whitespace()
                        .map(|s| s.to_string())
                        .collect();
                    args.extend(custom);
                }
            }
            _ => {
                args.extend(vec![
                    "-c:v".to_string(),
                    "libx264".to_string(),
                    "-crf".to_string(),
                    "23".to_string(),
                    "-preset".to_string(),
                    "medium".to_string(),
                    "-c:a".to_string(),
                    "aac".to_string(),
                    "-b:a".to_string(),
                    "128k".to_string(),
                ]);
            }
        }
    }

    /// 获取输出文件路径
    fn get_output_path(&self, input_path: &Path) -> Result<PathBuf> {
        let stem = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("无法获取文件名"))?;

        let extension = match self.config.output_format.as_str() {
            "mp4" => "mp4",
            "mkv" => "mkv",
            "ts" => "ts",
            _ => "mp4",
        };

        let output_name = format!("{}.{}", stem, extension);

        if let Some(parent) = input_path.parent() {
            Ok(parent.join(&output_name))
        } else {
            Ok(self.recordings_dir.join(&output_name))
        }
    }

    /// 根据转码模式获取输出文件扩展名
    #[allow(dead_code)]
    pub fn get_output_extension(&self) -> &'static str {
        if !self.is_enabled() {
            return "ts"; // 不转码时使用 ts 格式
        }
        match self.config.output_format.as_str() {
            "mkv" => "mkv",
            "ts" => "ts",
            _ => "mp4",
        }
    }
}

/// 转码模式描述
#[allow(dead_code)]
pub fn get_mode_description(mode: &str) -> &'static str {
    match mode {
        "off" => "不转码 - 直接保存原始流，速度最快但文件最大",
        "realtime" => "实时转码 - 录制时直接转码，省时省空间但CPU要求高",
        "post" => "后期转码 - 录制完成后再转码，最稳定但需要双倍时间",
        _ => "未知模式",
    }
}

/// 转码预设描述
#[allow(dead_code)]
pub fn get_preset_description(preset: &str) -> &'static str {
    match preset {
        "high" => "高质量 (CRF 18, 文件较大)",
        "medium" => "中等质量 (CRF 23, 推荐)",
        "low" => "低质量 (CRF 28, 文件最小)",
        "copy" => "直接复制 (不转码)",
        "custom" => "自定义参数",
        _ => "未知预设",
    }
}
