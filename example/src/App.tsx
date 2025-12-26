import React, { useState, useRef } from "react";
import * as wasm from "wasm";
import "./App.css";

interface AudioFile {
  id: string;
  name: string;
  duration: string;
  volume: number;
}

function App() {
  const [files, setFiles] = useState<AudioFile[]>([]);
  const [combinedUrl, setCombinedUrl] = useState<string | null>(null);

  // Persist the WASM instance and the Audio element
  const mergerRef = useRef<wasm.FilesMerger>(null); // Replace 'any' with wasm.FilesMerger type if available
  const audioRef = useRef<HTMLAudioElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Helper to update the player source and maintain playback position
  const updateAudioSource = (combinedData: ArrayBuffer) => {
    const blob = new Blob([combinedData], {
      type: "audio/mpeg",
    });
    const url = URL.createObjectURL(blob);

    if (audioRef.current) {
      const currentTime = audioRef.current.currentTime;
      const isPaused = audioRef.current.paused;

      // Revoke old URL to free memory
      if (combinedUrl) URL.revokeObjectURL(combinedUrl);

      setCombinedUrl(url);

      // Restore position on next tick
      setTimeout(() => {
        if (audioRef.current) {
          audioRef.current.currentTime = currentTime;
          if (!isPaused) audioRef.current.play().catch(() => {});
        }
      }, 0);
    } else {
      setCombinedUrl(url);
    }
  };

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFiles = Array.from(e.target.files ?? []);
    if (selectedFiles.length === 0) return;

    // Initialize/Reset the merger instance
    const filesMerger = wasm.FilesMerger.new();

    const newFiles = await Promise.all(
      selectedFiles.map(async (file) => {
        const bytes = new Uint8Array(await file.arrayBuffer());
        filesMerger.add_file(bytes); // Add to WASM memory

        const duration = await getAudioDuration(file);
        return {
          id: Math.random().toString(36).substring(2, 9),
          name: file.name,
          duration: duration,
          volume: 100,
        };
      })
    );

    // Save the merger and the files list
    mergerRef.current = filesMerger;
    setFiles(newFiles);

    // Initial combine
    const initialVolumes = new Uint8Array(newFiles.map(() => 100));
    const combinedFile = filesMerger.combine(initialVolumes);
    updateAudioSource(combinedFile.buffer as ArrayBuffer);
  };

  const updateVolume = (id: string, value: number) => {
    // 1. Update the state for the UI
    const updatedFiles = files.map((f) =>
      f.id === id ? { ...f, volume: value } : f
    );
    setFiles(updatedFiles);

    // 2. Use the persisted merger to get new audio data
    if (mergerRef.current) {
      const volumes = new Uint8Array(updatedFiles.map((f) => f.volume));
      const combinedFile = mergerRef.current.combine(volumes);
      updateAudioSource(combinedFile.buffer as ArrayBuffer);
    }
  };

  const getAudioDuration = async (file: File): Promise<string> => {
    const buffer = await file.arrayBuffer();
    const view = new DataView(buffer);
    let offset = 0;

    if (
      view.getUint8(0) === 0x49 &&
      view.getUint8(1) === 0x44 &&
      view.getUint8(2) === 0x33
    ) {
      const size =
        (view.getUint8(6) << 21) |
        (view.getUint8(7) << 14) |
        (view.getUint8(8) << 7) |
        view.getUint8(9);
      offset = size + 10;
    }

    let durationSeconds = 0;
    while (offset < view.byteLength - 4) {
      if (
        view.getUint8(offset) === 0xff &&
        (view.getUint8(offset + 1) & 0xe0) === 0xe0
      ) {
        const byte2 = view.getUint8(offset + 2);
        const bitrates = [
          0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
        ];
        const bitrate = bitrates[(byte2 >> 4) & 15];
        if (bitrate > 0) durationSeconds = (file.size * 8) / (bitrate * 1000);
        break;
      }
      offset++;
    }

    const h = Math.floor(durationSeconds / 3600)
      .toString()
      .padStart(2, "0");
    const m = Math.floor((durationSeconds % 3600) / 60)
      .toString()
      .padStart(2, "0");
    const s = Math.floor(durationSeconds % 60)
      .toString()
      .padStart(2, "0");
    return `${h}:${m}:${s}`;
  };

  const reset = () => {
    setFiles([]);
    setCombinedUrl(null);
    mergerRef.current = null;
    if (fileInputRef.current) fileInputRef.current.value = "";
  };

  return (
    <div className="container">
      <h1>MP3 Controller</h1>
      <div
        className="upload-section"
        style={{ display: files.length === 0 ? "initial" : "none" }}
      >
        <input
          type="file"
          accept=".mp3"
          multiple
          onChange={handleFileChange}
          ref={fileInputRef}
          id="file-upload"
        />
        <label htmlFor="file-upload" className="custom-upload">
          Choose MP3 Files
        </label>
      </div>
      {files.length === 0 ? null : (
        <div className="list-container">
          <div
            className="player-section"
            style={{
              marginBottom: "20px",
              padding: "15px",
              background: "#f4f4f4",
              borderRadius: "8px",
            }}
          >
            <h3 style={{ marginTop: 0 }}>Global Preview</h3>
            {combinedUrl && (
              <audio
                ref={audioRef}
                controls
                src={combinedUrl}
                style={{ width: "100%" }}
              />
            )}
          </div>

          <ul className="file-list">
            {files.map((file) => (
              <li key={file.id} className="file-item">
                <div className="file-info">
                  <span className="file-name">{file.name}</span>
                  <span className="file-duration">{file.duration}</span>
                </div>
                <div className="slider-container">
                  <input
                    type="range"
                    min="0"
                    max="100"
                    value={file.volume}
                    onChange={(e) =>
                      updateVolume(file.id, parseInt(e.target.value))
                    }
                  />
                  <span className="volume-label">{file.volume}%</span>
                </div>
              </li>
            ))}
          </ul>
          <button type="button" className="discard-btn" onClick={reset}>
            Discard and Go Back
          </button>
        </div>
      )}
    </div>
  );
}

export default App;
