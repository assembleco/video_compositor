use bytes::Bytes;

/// A struct representing a chunk of encoded data.
///
/// Many codecs specify that encoded data is split into chunks.
/// For example, H264 splits the data into NAL units and AV1 splits the data into OBU frames.
pub struct EncodedChunk {
    pub data: Bytes,
    pub pts: i64,
    pub dts: Option<i64>,
    pub kind: EncodedChunkKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodedChunkKind {
    Video(Codec),
}

#[derive(Debug, thiserror::Error)]
pub enum ChunkFromFfmpegError {
    #[error("No data")]
    NoData,
    #[error("No pts")]
    NoPts,
}

impl EncodedChunk {
    pub fn from_av_packet(
        value: &ffmpeg_next::Packet,
        kind: EncodedChunkKind,
    ) -> Result<Self, ChunkFromFfmpegError> {
        let data = match value.data() {
            Some(data) => Bytes::copy_from_slice(data),
            None => return Err(ChunkFromFfmpegError::NoData),
        };

        Ok(Self {
            data,
            pts: value.pts().ok_or(ChunkFromFfmpegError::NoPts)?,
            dts: value.dts(),
            kind,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
}

#[derive(Debug, thiserror::Error)]
pub enum CodecFromFfmpegError {
    #[error("Unsupported codec {0:?}")]
    UnsupportedCodec(ffmpeg_next::codec::Id),
}

impl TryFrom<ffmpeg_next::Codec> for Codec {
    type Error = CodecFromFfmpegError;

    fn try_from(value: ffmpeg_next::Codec) -> Result<Self, Self::Error> {
        match value.id() {
            ffmpeg_next::codec::Id::H264 => Ok(Self::H264),
            v => Err(CodecFromFfmpegError::UnsupportedCodec(v)),
        }
    }
}
