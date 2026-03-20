// Streaming WAV source for non-seekable readers (e.g., process stdout).

use rodio::Source;
use std::io::Read;

/// A streaming WAV source that reads from a non-seekable reader (e.g., process stdout).
/// Parses the WAV header first, then provides PCM samples for playback.
pub struct WavStreamSource<R: Read> {
    reader: R,
    sample_rate: u32,
    channels: u16,
    bytes_per_sample: usize,
    buffer: Vec<u8>,
    buffer_pos: usize,
    exhausted: bool,
}

impl<R: Read> WavStreamSource<R> {
    pub fn new(mut reader: R) -> Result<Self, String> {
        let mut header_buf = [0u8; 44];
        reader
            .read_exact(&mut header_buf)
            .map_err(|e| {
                log::error!("Failed to read WAV header from stream: {}", e);
                format!("Failed to read audio stream header: {}. The audio stream may be corrupted or incomplete.", e)
            })?;

        if &header_buf[0..4] != b"RIFF" {
            log::error!("Invalid streaming audio format: missing RIFF header");
            return Err(
                "Invalid audio stream: not a valid WAV format (missing RIFF header).".to_string(),
            );
        }
        if &header_buf[8..12] != b"WAVE" {
            log::error!("Invalid streaming audio format: missing WAVE header");
            return Err(
                "Invalid audio stream: not a valid WAV format (missing WAVE header).".to_string(),
            );
        }

        let mut sample_rate = 0u32;
        let mut channels = 0u16;
        let mut bits_per_sample = 0u16;

        let mut offset = 12;
        while offset + 8 <= 44 {
            let chunk_id = &header_buf[offset..offset + 4];
            let chunk_size = u32::from_le_bytes([
                header_buf[offset + 4],
                header_buf[offset + 5],
                header_buf[offset + 6],
                header_buf[offset + 7],
            ]) as usize;

            if chunk_id == b"fmt " {
                if offset + 8 + chunk_size > 44 {
                    let mut fmt_buf = vec![0u8; chunk_size];
                    fmt_buf[..44 - offset - 8].copy_from_slice(&header_buf[offset + 8..44]);
                    reader
                        .read_exact(&mut fmt_buf[44 - offset - 8..])
                        .map_err(|e| format!("Failed to read fmt chunk: {}", e))?;

                    channels = u16::from_le_bytes([fmt_buf[2], fmt_buf[3]]);
                    sample_rate =
                        u32::from_le_bytes([fmt_buf[4], fmt_buf[5], fmt_buf[6], fmt_buf[7]]);
                    bits_per_sample = u16::from_le_bytes([fmt_buf[14], fmt_buf[15]]);
                } else {
                    channels =
                        u16::from_le_bytes([header_buf[offset + 10], header_buf[offset + 11]]);
                    sample_rate = u32::from_le_bytes([
                        header_buf[offset + 12],
                        header_buf[offset + 13],
                        header_buf[offset + 14],
                        header_buf[offset + 15],
                    ]);
                    bits_per_sample =
                        u16::from_le_bytes([header_buf[offset + 22], header_buf[offset + 23]]);
                }
            } else if chunk_id == b"data" {
                break;
            }

            offset += 8 + chunk_size;
        }

        if sample_rate == 0 || channels == 0 || bits_per_sample == 0 {
            log::error!(
                "Invalid WAV format parameters - sample_rate: {}, channels: {}, bits_per_sample: {}",
                sample_rate, channels, bits_per_sample
            );
            return Err("Invalid audio stream: missing or invalid WAV format information. The stream is corrupted.".to_string());
        }

        if bits_per_sample != 8
            && bits_per_sample != 16
            && bits_per_sample != 24
            && bits_per_sample != 32
        {
            log::error!("Unsupported bits per sample: {}", bits_per_sample);
            return Err(format!(
                "Unsupported audio format: {}-bit audio is not supported. Only 8, 16, 24, and 32-bit audio is supported.",
                bits_per_sample
            ));
        }

        let bytes_per_sample = (bits_per_sample / 8) as usize;

        let mut buffer = vec![0u8; 8192];
        let initial_read = reader.read(&mut buffer).map_err(|e| {
            log::error!("Failed to read initial audio data from stream: {}", e);
            format!(
                "Failed to read audio stream data: {}. The stream may be corrupted or incomplete.",
                e
            )
        })?;

        if initial_read == 0 {
            log::error!("No audio data in stream");
            return Err("No audio data in stream. The audio file or stream is empty.".to_string());
        }

        buffer.truncate(initial_read);

        Ok(Self {
            reader,
            sample_rate,
            channels,
            bytes_per_sample,
            buffer,
            buffer_pos: 0,
            exhausted: false,
        })
    }

    fn refill_buffer(&mut self) {
        if self.exhausted {
            return;
        }

        let mut new_data = vec![0u8; 8192];
        match self.reader.read(&mut new_data) {
            Ok(0) => {
                self.exhausted = true;
            }
            Ok(n) => {
                new_data.truncate(n);
                self.buffer = new_data;
                self.buffer_pos = 0;
            }
            Err(e) => {
                log::warn!("Error reading from stream: {}", e);
                self.exhausted = true;
            }
        }
    }

    fn read_sample(&mut self) -> Option<f32> {
        let bytes_needed = self.bytes_per_sample * self.channels as usize;

        if self.buffer_pos + bytes_needed > self.buffer.len() {
            if self.exhausted {
                return None;
            }
            self.refill_buffer();
            if self.buffer.is_empty() {
                return None;
            }
        }

        if self.buffer_pos + bytes_needed > self.buffer.len() {
            return None;
        }

        let sample = match self.bytes_per_sample {
            2 => {
                let mut sum = 0.0f32;
                for ch in 0..self.channels as usize {
                    let offset = self.buffer_pos + ch * 2;
                    let s = i16::from_le_bytes([self.buffer[offset], self.buffer[offset + 1]]);
                    sum += s as f32 / 32768.0;
                }
                sum / self.channels as f32
            }
            1 => {
                let mut sum = 0.0f32;
                for ch in 0..self.channels as usize {
                    let offset = self.buffer_pos + ch;
                    let s = self.buffer[offset] as i16 - 128;
                    sum += s as f32 / 128.0;
                }
                sum / self.channels as f32
            }
            3 => {
                let mut sum = 0.0f32;
                for ch in 0..self.channels as usize {
                    let offset = self.buffer_pos + ch * 3;
                    let s = i32::from_le_bytes([
                        self.buffer[offset],
                        self.buffer[offset + 1],
                        self.buffer[offset + 2],
                        0,
                    ]) << 8
                        >> 8;
                    sum += s as f32 / 8388608.0;
                }
                sum / self.channels as f32
            }
            4 => {
                let mut sum = 0.0f32;
                for ch in 0..self.channels as usize {
                    let offset = self.buffer_pos + ch * 4;
                    let s = i32::from_le_bytes([
                        self.buffer[offset],
                        self.buffer[offset + 1],
                        self.buffer[offset + 2],
                        self.buffer[offset + 3],
                    ]);
                    sum += s as f32 / 2147483648.0;
                }
                sum / self.channels as f32
            }
            _ => return None,
        };

        self.buffer_pos += bytes_needed;
        Some(sample)
    }
}

impl<R: Read> Iterator for WavStreamSource<R> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.read_sample()
    }
}

impl<R: Read> Source for WavStreamSource<R> {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}
