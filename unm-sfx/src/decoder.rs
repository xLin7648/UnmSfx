use symphonia::core::{
    audio::{AudioBufferRef, Signal},
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    conv::FromSample,
    errors::Error,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

use std::io::Cursor;

use crate::atlas::RawSource;

macro_rules! fill_interleaved {
    ($audio_buf:expr, $out_data:expr) => {{
        let frames = $audio_buf.frames();
        let chan_count = $audio_buf.spec().channels.count();

        for i in 0..frames {
            for c in 0..chan_count {
                $out_data.push(f32::from_sample($audio_buf.chan(c)[i]));
            }
        }
    }};
}

pub(crate) fn decode(data: Vec<u8>) -> anyhow::Result<RawSource> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(data)), Default::default());

    let probed = symphonia::default::get_probe().format(
        &Hint::new(),
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL && t.codec_params.sample_rate.is_some())
        .ok_or_else(|| anyhow::anyhow!("no decodable audio track found"))?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|err| anyhow::anyhow!("failed to create decoder: {err}"))?;

    let sample_rate = track.codec_params.sample_rate.unwrap_or(48_000);
    let track_id = track.id;

    let mut interleaved_data = Vec::new();
    let mut channel_count: Option<usize> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(ref err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(err) => return Err(err.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        if let Ok(decoded) = decoder.decode(&packet) {
            let current_channels = decoded.spec().channels.count();
            if let Some(expected_channels) = channel_count {
                anyhow::ensure!(
                    expected_channels == current_channels,
                    "inconsistent channel count in stream"
                );
            } else {
                channel_count = Some(current_channels);
            }

            match decoded {
                AudioBufferRef::F32(buf) => {
                    let frames = buf.frames();
                    let chan_count = current_channels;
                    for i in 0..frames {
                        for c in 0..chan_count {
                            interleaved_data.push(buf.chan(c)[i]);
                        }
                    }
                }
                AudioBufferRef::U8(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::U16(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::U24(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::U32(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::S8(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::S16(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::S24(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::S32(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::F64(buf) => fill_interleaved!(buf, interleaved_data),
            }
        }
    }

    let channel_count = channel_count.ok_or_else(|| anyhow::anyhow!("decoded stream is empty"))?;
    anyhow::ensure!(channel_count != 0, "channel count must be greater than zero");
    anyhow::ensure!(
        interleaved_data.len() % channel_count == 0,
        "decoded sample count is not aligned to channel count"
    );

    let frames_count = interleaved_data.len() / channel_count;

    Ok(RawSource {
        data: interleaved_data.into_boxed_slice(),
        sample_rate,
        frames_count,
        channel_count,
    })
}

pub(crate) fn from_interleaved_f32(
    data: &[f32],
    frames_count: usize,
    channel_count: usize,
    sample_rate: u32,
) -> anyhow::Result<RawSource> {
    anyhow::ensure!(frames_count != 0, "frames_count must be greater than zero");
    anyhow::ensure!(channel_count != 0, "channel_count must be greater than zero");
    anyhow::ensure!(sample_rate != 0, "sample_rate must be greater than zero");

    let sample_count = frames_count
        .checked_mul(channel_count)
        .ok_or_else(|| anyhow::anyhow!("sample count overflow"))?;
    anyhow::ensure!(
        data.len() == sample_count,
        "interleaved data length does not match frame/channel metadata"
    );

    Ok(RawSource {
        frames_count,
        channel_count,
        sample_rate,
        data: data.to_vec().into_boxed_slice(),
    })
}
