use std::sync::Arc;

use compositor_pipeline::pipeline::encoder;
use compositor_pipeline::pipeline::output;
use compositor_pipeline::pipeline::structs::Codec;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::renderer::*;
use super::util::*;
use super::*;

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(tag = "entity_type", rename_all = "snake_case")]
pub enum RegisterRequest {
    InputStream(RegisterInputRequest),
    OutputStream(RegisterOutputRequest),
    Shader(ShaderSpec),
    WebRenderer(WebRendererSpec),
    Image(ImageSpec),
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct RegisterInputRequest {
    pub input_id: InputId,
    pub port: Port,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema, PartialEq, Eq)]
#[serde(untagged)]
pub enum Port {
    String(String),
    U16(u16),
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct RegisterOutputRequest {
    pub output_id: OutputId,
    pub port: u16,
    pub ip: Arc<str>,
    pub resolution: Resolution,
    pub encoder_settings: EncoderSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct EncoderSettings {
    preset: Option<EncoderPreset>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EncoderPreset {
    Ultrafast,
    Superfast,
    Veryfast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    Veryslow,
    Placebo,
}

impl From<RegisterOutputRequest> for encoder::EncoderOptions {
    fn from(request: RegisterOutputRequest) -> Self {
        let preset = match request
            .encoder_settings
            .preset
            .unwrap_or(EncoderPreset::Medium)
        {
            EncoderPreset::Ultrafast => encoder::ffmpeg_h264::EncoderPreset::Ultrafast,
            EncoderPreset::Superfast => encoder::ffmpeg_h264::EncoderPreset::Superfast,
            EncoderPreset::Veryfast => encoder::ffmpeg_h264::EncoderPreset::Veryfast,
            EncoderPreset::Faster => encoder::ffmpeg_h264::EncoderPreset::Faster,
            EncoderPreset::Fast => encoder::ffmpeg_h264::EncoderPreset::Fast,
            EncoderPreset::Medium => encoder::ffmpeg_h264::EncoderPreset::Medium,
            EncoderPreset::Slow => encoder::ffmpeg_h264::EncoderPreset::Slow,
            EncoderPreset::Slower => encoder::ffmpeg_h264::EncoderPreset::Slower,
            EncoderPreset::Veryslow => encoder::ffmpeg_h264::EncoderPreset::Veryslow,
            EncoderPreset::Placebo => encoder::ffmpeg_h264::EncoderPreset::Placebo,
        };
        Self::H264(encoder::ffmpeg_h264::Options {
            preset,
            resolution: request.resolution.into(),
            output_id: request.output_id.into(),
        })
    }
}

impl From<RegisterOutputRequest> for output::OutputOptions {
    fn from(value: RegisterOutputRequest) -> Self {
        output::OutputOptions::Rtp(output::rtp::RtpSenderOptions {
            codec: Codec::H264,
            ip: value.ip,
            port: value.port,
            output_id: value.output_id.into(),
        })
    }
}
