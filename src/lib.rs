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
pub struct AudioCombiner {
    files: Vec<SingleAudioFile>,
}
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum SingleAudioFileType {
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

// Helper to wrap raw f32 samples into a 16-bit PCM WAV file
fn create_wav_container(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let mut wav = Vec::new();
    let data_size = (samples.len() * 2) as u32;

    // RIFF Header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&2u16.to_le_bytes()); // Channels (Stereo)
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 4).to_le_bytes()); // Byte rate
    wav.extend_from_slice(&4u16.to_le_bytes()); // Block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // Bits per sample

    // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());

    for &sample in samples {
        // Soft clipping to prevent digital distortion
        let clamped = sample.clamp(-1.0, 1.0);
        let s = (clamped * i16::MAX as f32) as i16;
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}

#[wasm_bindgen]
impl AudioCombiner {
    pub fn new(files: Vec<SingleAudioFile>) -> Self {
        Self { files }
    }
    pub fn combine(&self, volumes: Vec<u8>) -> Result<SingleAudioFile, String> {
        let captured_now = now();
        let mut i = 0_u128;
        while now() < (captured_now + 3_000.0) {
            i += 1;
        }
        let mut master_buffer: Vec<f32> = Vec::new();
        let target_sample_rate = 44100u32; // Standard output rate

        for (i, file) in self.files.iter().enumerate() {
            let volume_factor = *volumes
                .get(i)
                .ok_or(format!("Missing volume for index {}", i))?
                as f32
                / 100.0;

            // 1. Setup Decoder with full identifiers
            let src = std::io::Cursor::new(file.bytes.clone());
            let mss =
                symphonia::core::io::MediaSourceStream::new(Box::new(src), Default::default());
            let mut hint = symphonia::core::probe::Hint::new();

            // Set hint based on type
            match file.r#type {
                SingleAudioFileType::Mpeg => {
                    hint.with_extension("mp3");
                }
                SingleAudioFileType::Ogg => {
                    hint.with_extension("ogg");
                }
            };

            let probed = symphonia::default::get_probe()
                .format(
                    &hint,
                    mss,
                    &symphonia::core::formats::FormatOptions::default(),
                    &symphonia::core::meta::MetadataOptions::default(),
                )
                .map_err(|e| e.to_string())?;

            let mut format = probed.format;
            let track = format
                .default_track()
                .ok_or("No supported audio track found")?;
            let mut decoder = symphonia::default::get_codecs()
                .make(
                    &track.codec_params,
                    &symphonia::core::codecs::DecoderOptions::default(),
                )
                .map_err(|e| e.to_string())?;

            let mut track_index = 0;

            // 2. Decode and Mix
            while let Ok(packet) = format.next_packet() {
                let decoded = decoder.decode(&packet).map_err(|e| e.to_string())?;
                let spec = *decoded.spec();

                let mut sample_buf = symphonia::core::audio::SampleBuffer::<f32>::new(
                    decoded.capacity() as u64,
                    spec,
                );
                sample_buf.copy_interleaved_ref(decoded);

                for &sample in sample_buf.samples() {
                    let processed_sample = sample * volume_factor;

                    if track_index >= master_buffer.len() {
                        master_buffer.push(processed_sample);
                    } else {
                        master_buffer[track_index] += processed_sample;
                    }
                    track_index += 1;
                }
            }
        }

        // 3. Convert Mixed PCM samples back to a playable format (WAV)
        // Since encoding Ogg in Wasm is heavy, WAV is the standard way to return playable audio to JS
        let result_bytes = create_wav_container(&master_buffer, target_sample_rate);

        Ok(SingleAudioFile {
            bytes: result_bytes,
            r#type: SingleAudioFileType::Ogg,
        })
    }

    pub fn print(&self) {
        let max_bytes = self.files.iter().map(|x| x.bytes.len()).max().unwrap();
        alert(
            format!(
                "Got list with length of {} and max bytes is {max_bytes}",
                self.files.len()
            )
            .as_str(),
        );
    }
}
