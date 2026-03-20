// WAV file parsing, PCM sample reading, amplitude envelope extraction, and concatenation.

use super::AmplitudeEnvelope;

/// WAV file format information extracted from the header
pub(super) struct WavInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub data_offset: usize,
    pub data_size: usize,
}

/// Parse WAV header to extract format information
pub(super) fn parse_wav_header(bytes: &[u8]) -> Result<WavInfo, String> {
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

/// Find the start of the "data" chunk payload in a WAV file.
/// Returns the byte offset where raw PCM data begins (after "data" + 4-byte size field).
pub(super) fn find_wav_data_offset(wav: &[u8]) -> Option<usize> {
    if wav.len() < 12 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12usize;
    while pos + 8 <= wav.len() {
        let chunk_id = &wav[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([wav[pos + 4], wav[pos + 5], wav[pos + 6], wav[pos + 7]]) as usize;
        if chunk_id == b"data" {
            return Some(pos + 8); // byte offset right after "data" + 4-byte size
        }
        pos += 8 + chunk_size;
        if chunk_size % 2 != 0 {
            pos += 1;
        } // WAV chunks are word-aligned
    }
    None
}

/// Concatenate multiple PCM WAV buffers into a single valid WAV.
/// All buffers must share the same sample rate, channels, and bit depth.
/// Returns the first buffer unchanged if the slice has only one element.
pub fn concat_wav_files(wavs: Vec<Vec<u8>>) -> Result<Vec<u8>, String> {
    match wavs.len() {
        0 => return Err("No audio fragments to concatenate".to_string()),
        1 => return Ok(wavs.into_iter().next().unwrap()),
        _ => {}
    }

    let first = &wavs[0];
    let first_data_offset =
        find_wav_data_offset(first).ok_or("First audio fragment is not a valid WAV file")?;

    // Everything before the "data" chunk identifier (RIFF header + fmt chunk + other chunks)
    let prefix_end = first_data_offset - 8; // back up past "data" (4) + data_size (4)

    // Collect raw PCM from all fragments
    let mut all_pcm: Vec<u8> = first[first_data_offset..].to_vec();
    for (idx, wav) in wavs[1..].iter().enumerate() {
        match find_wav_data_offset(wav) {
            Some(offset) => all_pcm.extend_from_slice(&wav[offset..]),
            None => log::warn!("[Audio] Fragment {} is not a valid WAV, skipping", idx + 1),
        }
    }

    // Build combined WAV
    let mut output: Vec<u8> = Vec::with_capacity(prefix_end + 8 + all_pcm.len());
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0u8; 4]); // RIFF size placeholder
    output.extend_from_slice(&first[8..prefix_end]); // "WAVE" + fmt chunk
    output.extend_from_slice(b"data");
    output.extend_from_slice(&(all_pcm.len() as u32).to_le_bytes());
    output.extend_from_slice(&all_pcm);

    // Fix RIFF chunk size (total file size - 8 for the "RIFF" + size fields)
    let riff_size = (output.len() - 8) as u32;
    output[4..8].copy_from_slice(&riff_size.to_le_bytes());

    log::debug!(
        "[Audio] Concatenated {} WAV fragments into {} bytes",
        wavs.len(),
        output.len()
    );

    Ok(output)
}
