mod utils;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, wasm!");
}

#[wasm_bindgen]
pub struct FilesMerger {
    files: Vec<Vec<u8>>,
}

#[wasm_bindgen]
impl FilesMerger {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }
    pub fn add_file(&mut self, file: Vec<u8>) {
        self.files.push(file);
    }
    pub fn combine(&self, volumes: Vec<u8>) -> Result<Vec<u8>, String> {
        // 1. Validate and Pair Files with Volumes
        let files_with_volume = self
            .files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let volume = volumes
                    .get(i)
                    .ok_or(format!("Missing volume for file index {}", i))?;
                Ok((file, *volume as f32 / 100.0)) // Normalize volume to 0.0 - 1.0
            })
            .collect::<Result<Vec<_>, String>>()?;

        if files_with_volume.is_empty() {
            return Ok(vec![]);
        }

        // 2. Find the length of the longest file to determine output size
        let max_length = files_with_volume
            .iter()
            .map(|(f, _)| f.len())
            .max()
            .unwrap_or(0);
        let mut result = vec![0u8; max_length];

        // 3. Mix the bytes
        // WARNING: This assumes raw PCM-like data.
        // For actual MP3s, you must decode to samples first.
        for i in 0..max_length {
            let mut mixed_sample = 0.0f32;

            for (file_bytes, volume_scale) in &files_with_volume {
                if let Some(&byte) = file_bytes.get(i) {
                    // Convert byte to float, scale by volume, and add to mix
                    mixed_sample += (byte as f32) * volume_scale;
                }
            }

            // 4. Clamp the result to 0-255 to prevent overflow/distortion
            result[i] = mixed_sample.clamp(0.0, 255.0) as u8;
        }
        _ = result;
        Ok(result)
        // Ok(self.files[0].clone())
    }
    pub fn print(&self) {
        let max_bytes = self.files.iter().max().unwrap().len();
        alert(
            format!(
                "Got list with length of {} and max bytes is {max_bytes}",
                self.files.len()
            )
            .as_str(),
        );
    }
}
