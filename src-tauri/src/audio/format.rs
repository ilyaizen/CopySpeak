// Audio format conversion via ffmpeg.

use crate::config::{AudioFormat, FormatConfig};
use std::process::Command;

/// Check if ffmpeg is available on the system.
pub fn check_ffmpeg_available() -> Result<(), String> {
    let output = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map_err(|e| {
            format!(
                "ffmpeg not found. Please install ffmpeg to use audio format conversion: {}",
                e
            )
        })?;

    if !output.status.success() {
        return Err(
            "ffmpeg command failed. Please ensure ffmpeg is properly installed.".to_string(),
        );
    }

    Ok(())
}

/// Convert WAV audio bytes to the target format using ffmpeg.
/// Returns the converted audio bytes.
pub fn convert_audio_format(
    wav_bytes: &[u8],
    format_config: &FormatConfig,
) -> Result<Vec<u8>, String> {
    // Validate input audio data
    if wav_bytes.is_empty() {
        return Err("Cannot convert empty audio data".to_string());
    }

    if wav_bytes.len() < 44 {
        return Err(format!(
            "Audio data too small for conversion ({} bytes). The audio may be corrupted.",
            wav_bytes.len()
        ));
    }

    // Check if conversion is needed
    if format_config.format == AudioFormat::Wav {
        return Ok(wav_bytes.to_vec());
    }

    // Verify ffmpeg is available
    check_ffmpeg_available()?;

    // Create temp files for input and output
    let temp_dir = std::env::temp_dir();
    let input_path = temp_dir.join("copyspeak_convert_input.wav");
    let output_path = temp_dir.join(format!(
        "copyspeak_convert_output.{}",
        format_config.format.default_extension()
    ));

    // Write input WAV to temp file
    std::fs::write(&input_path, wav_bytes)
        .map_err(|e| format!("Failed to write temp input file: {}", e))?;

    // Build ffmpeg command based on format
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y") // Overwrite output
        .arg("-i")
        .arg(&input_path);

    match format_config.format {
        AudioFormat::Mp3 => {
            cmd.arg("-c:a")
                .arg("libmp3lame")
                .arg("-b:a")
                .arg(format!("{}k", format_config.mp3_bitrate));
        }
        AudioFormat::Ogg => {
            cmd.arg("-c:a")
                .arg("libvorbis")
                .arg("-b:a")
                .arg(format!("{}k", format_config.ogg_bitrate));
        }
        AudioFormat::Flac => {
            cmd.arg("-c:a")
                .arg("flac")
                .arg("-compression_level")
                .arg(format_config.flac_compression.to_string());
        }
        AudioFormat::Wav => unreachable!(),
    }

    cmd.arg(&output_path);

    // Execute ffmpeg
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute ffmpeg: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg conversion failed: {}", stderr));
    }

    // Read converted output with validation
    let result = std::fs::read(&output_path).map_err(|e| {
        // Clean up on failure
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);

        match e.kind() {
            std::io::ErrorKind::NotFound => {
                format!("Audio conversion failed: output file not created by ffmpeg. Check ffmpeg installation.")
            }
            std::io::ErrorKind::PermissionDenied => {
                format!("Permission denied reading converted audio file. Check file system permissions.")
            }
            _ => {
                format!("Failed to read converted audio file: {}. The conversion may have failed.", e)
            }
        }
    })?;

    // Validate converted output
    if result.is_empty() {
        // Clean up
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);
        return Err("Audio conversion produced empty output. The conversion failed.".to_string());
    }

    // Clean up temp files
    let _ = std::fs::remove_file(&input_path);
    let _ = std::fs::remove_file(&output_path);

    log::info!(
        "Audio converted successfully from WAV to {} ({} bytes)",
        format_config.format.default_extension(),
        result.len()
    );

    Ok(result)
}
