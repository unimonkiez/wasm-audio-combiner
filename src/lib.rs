mod utils;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
    #[wasm_bindgen(js_namespace = Date)]
    fn now() -> f64;
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, wasm!");
}

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum SingleAudioFileType {
    Wav,
    Mpeg,
    Ogg,
}

#[wasm_bindgen]
pub struct SingleAudioFile {
    #[wasm_bindgen(getter_with_clone)]
    pub bytes: Vec<u8>,
    pub r#type: SingleAudioFileType,
}

#[wasm_bindgen]
impl SingleAudioFile {
    pub fn new(bytes: Vec<u8>, r#type: SingleAudioFileType) -> Self {
        Self { bytes, r#type }
    }
}

fn create_wav_container(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let mut wav = Vec::new();
    let data_size = (samples.len() * 2) as u32; // 2 bytes per sample (i16)

    // RIFF Header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes()); // Hardcoded Stereo
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());

    // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let s = (clamped * i16::MAX as f32) as i16;
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}

struct AudioCombinerSingleFile {
    samples: Vec<f32>,
}
#[wasm_bindgen]
pub struct AudioCombiner {
    files: Vec<AudioCombinerSingleFile>,
}

#[wasm_bindgen]
impl AudioCombiner {
    pub fn new(files: Vec<SingleAudioFile>) -> Result<AudioCombiner, String> {
        let mut processed_files = Vec::with_capacity(files.len());

        for file in files {
            let mut decoded_samples = Vec::new();
            let src = std::io::Cursor::new(file.bytes);
            let mss =
                symphonia::core::io::MediaSourceStream::new(Box::new(src), Default::default());

            let mut hint = symphonia::core::probe::Hint::new();
            match file.r#type {
                SingleAudioFileType::Wav => {
                    hint.with_extension("wav");
                }
                SingleAudioFileType::Mpeg => {
                    hint.with_extension("mp3");
                }
                SingleAudioFileType::Ogg => {
                    hint.with_extension("ogg");
                }
            }

            let probed = symphonia::default::get_probe()
                .format(&hint, mss, &Default::default(), &Default::default())
                .map_err(|e| e.to_string())?;

            let mut format = probed.format;
            let track = format.default_track().ok_or("No supported audio track")?;
            let mut decoder = symphonia::default::get_codecs()
                .make(&track.codec_params, &Default::default())
                .map_err(|e| e.to_string())?;

            let mut sample_buf = None;

            while let Ok(packet) = format.next_packet() {
                let decoded = decoder.decode(&packet).map_err(|e| e.to_string())?;
                let spec = *decoded.spec();
                let num_channels = spec.channels.count();

                let buf = sample_buf.get_or_insert_with(|| {
                    symphonia::core::audio::SampleBuffer::<f32>::new(
                        decoded.capacity() as u64,
                        spec,
                    )
                });
                buf.copy_interleaved_ref(decoded);

                // Convert everything to Stereo (2 channels) during ingestion
                for frame in buf.samples().chunks(num_channels) {
                    if num_channels == 1 {
                        decoded_samples.push(frame[0]); // Left
                        decoded_samples.push(frame[0]); // Right
                    } else {
                        decoded_samples.push(frame[0]); // Left
                        decoded_samples.push(frame[1]); // Right
                    }
                }
            }
            processed_files.push(AudioCombinerSingleFile {
                samples: decoded_samples,
            });
        }

        Ok(AudioCombiner {
            files: processed_files,
        })
    }

    pub fn combine(&self, volumes: Vec<u8>) -> Result<SingleAudioFile, String> {
        let target_sample_rate = 44100u32;

        // 1. Determine final length
        let max_len = self
            .files
            .iter()
            .map(|f| f.samples.len())
            .max()
            .unwrap_or(0);

        // 2. Pre-allocate master buffer with zeros
        let mut master_buffer = vec![0.0f32; max_len];

        // 3. Simple addition mix
        for (i, file) in self.files.iter().enumerate() {
            let volume_factor = *volumes.get(i).unwrap_or(&100) as f32 / 100.0;

            // Zip allows the compiler to use SIMD optimizations
            for (m_sample, &f_sample) in master_buffer.iter_mut().zip(file.samples.iter()) {
                *m_sample += f_sample * volume_factor;
            }
        }

        // 4. Wrap in WAV container
        Ok(SingleAudioFile {
            bytes: create_wav_container(&master_buffer, target_sample_rate),
            r#type: SingleAudioFileType::Wav,
        })
    }
}
