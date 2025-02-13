use std::{env, str::FromStr, sync::OnceLock, time::Duration};

use compositor_render::{web_renderer::WebRendererInitOptions, Framerate};
use log::error;

use crate::logger::FfmpegLogLevel;

pub struct Config {
    pub api_port: u16,
    pub logger: LoggerConfig,
    pub framerate: Framerate,
    pub stream_fallback_timeout: Duration,
    pub web_renderer: WebRendererInitOptions,
}

pub struct LoggerConfig {
    pub ffmpeg_logger_level: FfmpegLogLevel,
    pub format: LoggerFormat,
    pub level: String,
}

#[derive(Debug, Copy, Clone)]
pub enum LoggerFormat {
    Pretty,
    Json,
    Compact,
}

impl FromStr for LoggerFormat {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(LoggerFormat::Json),
            "pretty" => Ok(LoggerFormat::Pretty),
            "compact" => Ok(LoggerFormat::Compact),
            _ => Err("invalid logger format"),
        }
    }
}

pub fn config() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();

    CONFIG.get_or_init(|| {
        read_config().expect("Failed to read the config from environment variables.")
    })
}

fn read_config() -> Result<Config, &'static str> {
    let api_port = match env::var("LIVE_COMPOSITOR_API_PORT") {
        Ok(api_port) => api_port
            .parse::<u16>()
            .map_err(|_| "LIVE_COMPOSITOR_API_PORT has to be valid port number")?,
        Err(_) => 8081,
    };

    let ffmpeg_logger_level = match env::var("LIVE_COMPOSITOR_FFMPEG_LOGGER_LEVEL") {
        Ok(ffmpeg_log_level) => {
            FfmpegLogLevel::from_str(&ffmpeg_log_level).unwrap_or(FfmpegLogLevel::Warn)
        }
        Err(_) => FfmpegLogLevel::Warn,
    };

    let logger_level = match env::var("LIVE_COMPOSITOR_LOGGER_LEVEL") {
        Ok(level) => level,
        Err(_) => "info".to_string(),
    };

    // When building in repo use compact logger
    let default_logger_format = match env::var("CARGO_MANIFEST_DIR") {
        Ok(_) => LoggerFormat::Compact,
        Err(_) => LoggerFormat::Json,
    };
    let logger_format = match env::var("LIVE_COMPOSITOR_LOGGER_FORMAT") {
        Ok(format) => LoggerFormat::from_str(&format).unwrap_or(default_logger_format),
        Err(_) => default_logger_format,
    };

    const DEFAULT_FRAMERATE: Framerate = Framerate { num: 30, den: 1 };
    let framerate = match env::var("LIVE_COMPOSITOR_OUTPUT_FRAMERATE") {
        Ok(framerate) => framerate_from_str(&framerate).unwrap_or(DEFAULT_FRAMERATE),
        Err(_) => DEFAULT_FRAMERATE,
    };

    const DEFAULT_WEB_RENDERER_ENABLED: bool = cfg!(feature = "web_renderer");
    let web_renderer_enable = match env::var("LIVE_COMPOSITOR_WEB_RENDERER_ENABLE") {
        Ok(enable) => bool_env_from_str(&enable).unwrap_or(DEFAULT_WEB_RENDERER_ENABLED),
        Err(_) => DEFAULT_WEB_RENDERER_ENABLED,
    };

    let web_renderer_gpu_enable = match env::var("LIVE_COMPOSITOR_WEB_RENDERER_GPU_ENABLE") {
        Ok(enable) => bool_env_from_str(&enable).unwrap_or(true),
        Err(_) => true,
    };

    const DEFAULT_STREAM_FALLBACK_TIMEOUT: Duration = Duration::from_millis(2000);
    let stream_fallback_timeout = match env::var("LIVE_COMPOSITOR_STREAM_FALLBACK_TIMEOUT_MS") {
        Ok(timeout_ms) => match timeout_ms.parse::<f64>() {
            Ok(timeout_ms) => Duration::from_secs_f64(timeout_ms),
            Err(_) => {
                error!("Invalid value provided for \"LIVE_COMPOSITOR_STREAM_FALLBACK_TIMEOUT_MS\". Falling back to default value 2000ms.");
                DEFAULT_STREAM_FALLBACK_TIMEOUT
            }
        },
        Err(_) => DEFAULT_STREAM_FALLBACK_TIMEOUT,
    };

    Ok(Config {
        api_port,
        logger: LoggerConfig {
            ffmpeg_logger_level,
            format: logger_format,
            level: logger_level,
        },
        framerate,
        stream_fallback_timeout,
        web_renderer: WebRendererInitOptions {
            enable: web_renderer_enable,
            enable_gpu: web_renderer_gpu_enable,
        },
    })
}

fn framerate_from_str(s: &str) -> Result<Framerate, &'static str> {
    const ERROR_MESSAGE: &str = "Framerate needs to be an unsigned integer or a string in the \"NUM/DEN\" format, where NUM and DEN are both unsigned integers.";
    if s.contains('/') {
        let Some((num_str, den_str)) = s.split_once('/') else {
            return Err(ERROR_MESSAGE);
        };
        let num = num_str.parse::<u32>().map_err(|_| ERROR_MESSAGE)?;
        let den = den_str.parse::<u32>().map_err(|_| ERROR_MESSAGE)?;
        Ok(compositor_render::Framerate { num, den })
    } else {
        Ok(compositor_render::Framerate {
            num: s.parse::<u32>().map_err(|_| ERROR_MESSAGE)?,
            den: 1,
        })
    }
}

fn bool_env_from_str(s: &str) -> Option<bool> {
    match s {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}
