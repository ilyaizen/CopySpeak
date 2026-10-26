// WAV file parsing, PCM sample reading, and amplitude envelope extraction.

use super::AmplitudeEnvelope;

/// WAV file format information extracted from the header
/// `pub(crate)`: `tts::stream` parses a batch WAV header to build format metadata.
pub(crate) struct WavInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub data_offset: usize,
    pub data_size: usize,
}

/// Parse WAV header to extract format information
pub(crate) fn parse_wav_header(bytes: &[u8]) -> Result<WavInfo, String> {
    if bytes.is_empty() {
        return Err("Audio file is empty".to_string());
    }

    if bytes.len() < 44 {
        return Err(format!(
            "Audio file too small ({} bytes). A valid WAV file requires at least 44 bytes. The file is corrupted or incomplete.",
            bytes.len()
        ));
    }

    // Check RIFF header
    if &bytes[0..4] != b"RIFF" {
        return Err("Invalid audio format: not a valid WAV file (missing RIFF header). The file may be corrupted or in the wrong format.".to_string());
    }

    // Check WAVE format
    if &bytes[8..12] != b"WAVE" {
        return Err("Invalid audio format: not a valid WAV file (missing WAVE header). The file may be corrupted or in the wrong format.".to_string());
    }

    // Find fmt chunk
    let mut offset = 12;
    let mut sample_rate = 0u32;
    let mut channels = 0u16;
    let mut bits_per_sample = 0u16;
    let mut data_offset = 0usize;
    let mut data_size = 0usize;

    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;

        if chunk_id == b"fmt " {
            if offset + 8 + chunk_size > bytes.len() {
                return Err("fmt chunk extends beyond file".to_string());
            }

            // Parse fmt chunk (minimum 16 bytes)
            if chunk_size < 16 {
                return Err("fmt chunk too small".to_string());
            }

            let audio_format = u16::from_le_bytes([bytes[offset + 8], bytes[offset + 9]]);
            if audio_format != 1 {
                return Err(format!("Unsupported audio format: {}", audio_format));
            }

            channels = u16::from_le_bytes([bytes[offset + 10], bytes[offset + 11]]);
            sample_rate = u32::from_le_bytes([
                bytes[offset + 12],
                bytes[offset + 13],
                bytes[offset + 14],
                bytes[offset + 15],
            ]);
            bits_per_sample = u16::from_le_bytes([bytes[offset + 22], bytes[offset + 23]]);
        } else if chunk_id == b"data" {
            data_offset = offset + 8;
            data_size = chunk_size;
        }

        offset += 8 + chunk_size;
    }

    if sample_rate == 0 || channels == 0 || bits_per_sample == 0 {
        return Err(format!(
            "Invalid WAV format: missing required format information (sample_rate: {}, channels: {}, bits_per_sample: {}). The file is corrupted.",
            sample_rate, channels, bits_per_sample
        ));
    }

    if data_offset == 0 {
        return Err(
            "Invalid WAV file: no audio data chunk found. The file is corrupted or incomplete."
                .to_string(),
        );
    }

    // Validate format parameters
    if channels == 0 || channels > 8 {
        return Err(format!(
            "Invalid audio format: unsupported channel count ({}). Only 1-8 channels are supported.",
            channels
        ));
    }

    if bits_per_sample != 8
        && bits_per_sample != 16
        && bits_per_sample != 24
        && bits_per_sample != 32
    {
        return Err(format!(
            "Unsupported audio format: {}-bit audio is not supported. Only 8, 16, 24, and 32-bit audio is supported.",
            bits_per_sample
        ));
    }

    if sample_rate < 8000 || sample_rate > 192000 {
        return Err(format!(
            "Invalid sample rate: {} Hz. Valid range is 8000-192000 Hz.",
            sample_rate
        ));
    }

    Ok(WavInfo {
        sample_rate,
        channels,
        bits_per_sample,
        data_offset,
        data_size,
    })
}

/// Read PCM samples from WAV data and convert to mono f32 samples (-1.0 to 1.0)
pub(super) fn read_pcm_samples(bytes: &[u8], info: &WavInfo) -> Result<Vec<f32>, String> {
    let data_end = info.data_offset + info.data_size;
    if data_end > bytes.len() {
        return Err(format!(
            "Corrupted audio file: data chunk extends beyond file (expected {} bytes, file has {} bytes)",
            data_end, bytes.len()
        ));
    }

    if info.data_size == 0 {
        return Err(
            "Audio file contains no audio data. The file is empty or corrupted.".to_string(),
        );
    }

    let data = &bytes[info.data_offset..data_end];
    let bytes_per_sample = (info.bits_per_sample / 8) as usize;
    let num_samples = data.len() / bytes_per_sample / info.channels as usize;

    let mut samples = Vec::with_capacity(num_samples);

    match info.bits_per_sample {
        16 => {
            // 16-bit signed PCM
            for i in 0..num_samples {
                let mut sum = 0.0f32;
                for ch in 0..info.channels as usize {
                    let offset = (i * info.channels as usize + ch) * 2;
                    if offset + 1 >= data.len() {
                        break;
                    }
                    let sample = i16::from_le_bytes([data[offset], data[offset + 1]]);
                    sum += sample as f32 / 32768.0;
                }
                samples.push(sum / info.channels as f32);
            }
        }
        8 => {
            // 8-bit unsigned PCM
            for i in 0..num_samples {
                let mut sum = 0.0f32;
                for ch in 0..info.channels as usize {
                    let offset = i * info.channels as usize + ch;
                    if offset >= data.len() {
                        break;
                    }
                    let sample = data[offset] as i16 - 128;
                    sum += sample as f32 / 128.0;
                }
                samples.push(sum / info.channels as f32);
            }
        }
        24 => {
            // 24-bit signed PCM
            for i in 0..num_samples {
                let mut sum = 0.0f32;
                for ch in 0..info.channels as usize {
                    let offset = (i * info.channels as usize + ch) * 3;
                    if offset + 2 >= data.len() {
                        break;
                    }
                    // Convert 24-bit to 32-bit signed, then normalize
                    let sample =
                        i32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], 0])
                            << 8
                            >> 8;
                    sum += sample as f32 / 8388608.0;
                }
                samples.push(sum / info.channels as f32);
            }
        }
        32 => {
            // 32-bit signed PCM
            for i in 0..num_samples {
                let mut sum = 0.0f32;
                for ch in 0..info.channels as usize {
                    let offset = (i * info.channels as usize + ch) * 4;
                    if offset + 3 >= data.len() {
                        break;
                    }
                    let sample = i32::from_le_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]);
                    sum += sample as f32 / 2147483648.0;
                }
                samples.push(sum / info.channels as f32);
            }
        }
        _ => {
            return Err(format!(
                "Unsupported bits per sample: {}",
                info.bits_per_sample
            ));
        }
    }

    Ok(samples)
}

/// Compute RMS (Root Mean Square) value for a chunk of samples
pub(super) fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_squares: f32 = samples.iter().map(|&s| s * s).sum();
    (sum_squares / samples.len() as f32).sqrt()
}

/// Extract an amplitude envelope from raw audio bytes.
/// Returns `num_bars` normalized RMS values (0.0–1.0).
///
/// This is computed once per TTS output and sent to the HUD — no streaming needed.
/// For non-WAV formats (MP3, etc.), returns a default envelope since we can't
/// easily parse those formats without full decoding.
pub fn extract_envelope(audio_bytes: &[u8], num_bars: usize) -> Result<AmplitudeEnvelope, String> {
    // Check if this is a WAV file (RIFF header)
    if audio_bytes.len() < 12 || &audio_bytes[0..4] != b"RIFF" || &audio_bytes[8..12] != b"WAVE" {
        // Not a WAV file - could be MP3, OGG, FLAC, etc.
        // Return a default envelope with estimated duration
        // For MP3: rough estimate is 1 second per 16KB at 128kbps
        let estimated_duration_ms = if audio_bytes.len() > 100 {
            if audio_bytes[0] == 0xFF && (audio_bytes[1] & 0xE0) == 0xE0 {
                // MP3 format detected
                ((audio_bytes.len() as f64 / 16000.0) * 1000.0) as u64
            } else {
                // Unknown format, use generic estimate
                2000
            }
        } else {
            1000
        };

        log::debug!(
            "Non-WAV audio detected, using default envelope with estimated duration: {}ms",
            estimated_duration_ms
        );

        return Ok(AmplitudeEnvelope {
            values: vec![0.5; num_bars], // Default flat envelope
            duration_ms: estimated_duration_ms,
        });
    }

    // Parse WAV header
    let wav_info = parse_wav_header(audio_bytes)?;

    // Read PCM samples
    let samples = read_pcm_samples(audio_bytes, &wav_info)?;

    // Calculate duration
    let duration_ms = (samples.len() as f64 / wav_info.sample_rate as f64 * 1000.0) as u64;

    // Divide samples into num_bars chunks and compute RMS for each
    let chunk_size = samples.len() / num_bars;
    if chunk_size == 0 {
        return Ok(AmplitudeEnvelope {
            values: vec![0.0; num_bars],
            duration_ms,
        });
    }

    let mut rms_values = Vec::with_capacity(num_bars);
    for i in 0..num_bars {
        let start = i * chunk_size;
        let end = if i == num_bars - 1 {
            samples.len()
        } else {
            (i + 1) * chunk_size
        };

        let chunk = &samples[start..end];
        let rms = compute_rms(chunk);
        rms_values.push(rms);
    }

    // Normalize values to 0.0-1.0 range
    let max_rms = rms_values.iter().cloned().fold(0.0f32, f32::max);
    let normalized = if max_rms > 0.0 {
        rms_values.iter().map(|&v| v / max_rms).collect()
    } else {
        vec![0.0; num_bars]
    };

    Ok(AmplitudeEnvelope {
        values: normalized,
        duration_ms,
    })
}

/// Clamp declared RIFF/data chunk sizes to the bytes actually present.
///
/// Streaming-style providers emit placeholder sizes before knowing the final
/// length: xAI ships `data` = `i32::MAX` (and a RIFF size with the sign bit
/// set), Fish ships `0xFFFFFF00`. Browsers clamp to the real byte count and
/// play fine, but rodio's symphonia decoder trusts the declared size and dies
/// with "end of stream" the moment it reads past EOF, and header-based
/// duration math measures a bogus value. Rewrites both fields in place;
/// honest headers and non-WAV bytes pass through untouched.
pub(crate) fn clamp_chunk_sizes_to_file(bytes: &mut [u8]) {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return;
    }
    let riff_len = (bytes.len() - 8).min(u32::MAX as usize) as u32;
    bytes[4..8].copy_from_slice(&riff_len.to_le_bytes());

    let mut offset = 12usize;
    while offset + 8 <= bytes.len() {
        let declared = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let available = bytes.len() - (offset + 8);
        if &bytes[offset..offset + 4] == b"data" && declared > available {
            // Patching requires declared > available, so available < u32::MAX
            // here and the cast cannot truncate.
            bytes[offset + 4..offset + 8].copy_from_slice(&(available as u32).to_le_bytes());
        }
        // Walk with the (possibly clamped) size; odd chunks carry a pad byte.
        let size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        offset += 8 + size + (size % 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical 44-byte-header WAV, hand-rolled so the test does not depend
    /// on the tts module's writer.
    fn canonical_wav(pcm: &[u8]) -> Vec<u8> {
        let mut wav = Vec::with_capacity(44 + pcm.len());
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&((36 + pcm.len()) as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&24000u32.to_le_bytes());
        wav.extend_from_slice(&48000u32.to_le_bytes()); // byte rate
        wav.extend_from_slice(&2u16.to_le_bytes()); // block align
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        wav.extend_from_slice(pcm);
        wav
    }

    /// xAI's wire shape: RIFF size with the sign bit set, data = i32::MAX.
    fn xai_placeholder_wav(pcm: &[u8]) -> Vec<u8> {
        let mut wav = canonical_wav(pcm);
        let riff: u32 = 0x8000_0000 + (wav.len() as u32 - 8);
        wav[4..8].copy_from_slice(&riff.to_le_bytes());
        wav[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        wav
    }

    #[test]
    fn clamp_fixes_streaming_placeholder_sizes() {
        let pcm = vec![0xABu8; 1600];
        let mut wav = xai_placeholder_wav(&pcm);
        clamp_chunk_sizes_to_file(&mut wav);
        let info = parse_wav_header(&wav).unwrap();
        assert_eq!(info.data_offset, 44);
        assert_eq!(info.data_size, 1600);
        assert_eq!(&wav[44..], &pcm[..], "PCM must be untouched");
    }

    #[test]
    fn clamp_leaves_honest_files_unchanged() {
        let mut wav = canonical_wav(&vec![0x11; 800]);
        let before = wav.clone();
        clamp_chunk_sizes_to_file(&mut wav);
        assert_eq!(wav, before);
    }

    #[test]
    fn clamp_ignores_non_wav_bytes() {
        let mut garbage = vec![1u8, 2, 3];
        clamp_chunk_sizes_to_file(&mut garbage);
        assert_eq!(garbage, vec![1u8, 2, 3]);
    }
}
