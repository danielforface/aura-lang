/**
 * Sentinel UI: Grid Layout Visualizer Panel
 * Displays a live preview and inspection of Grid layouts, Image fit modes, and Audio playback state.
 */

import React, { useState, useEffect } from "react";

interface GridCell {
  col: number;
  row: number;
  colSpan: number;
  rowSpan: number;
  content: string;
  bgColor?: string;
}

interface GridLayout {
  columns: number;
  rows?: number;
  gap: number;
  padding: number;
  bgColor: string;
  cells: GridCell[];
}

interface AudioState {
  isLoaded: boolean;
  clipId?: number;
  isPlaying: boolean;
  currentTime: number;
  duration: number;
}

interface LuminaVisualizerState {
  gridLayout?: GridLayout;
  imageFitMode?: "stretch" | "contain" | "cover";
  audioState?: AudioState;
}

/**
 * GridLayoutVisualizer: Renders Grid layout in Sentinel UI
 */
export const GridLayoutVisualizer: React.FC<{ layout: GridLayout }> = ({ layout }) => {
  const cellSize = 60; // pixels per grid unit
  const totalWidth = layout.columns * cellSize + (layout.columns - 1) * layout.gap + layout.padding * 2;
  const totalHeight = (layout.rows || 4) * cellSize + ((layout.rows || 4) - 1) * layout.gap + layout.padding * 2;

  return (
    <div
      style={{
        display: "inline-grid",
        gridTemplateColumns: `repeat(${layout.columns}, ${cellSize}px)`,
        gap: `${layout.gap}px`,
        padding: `${layout.padding}px`,
        backgroundColor: layout.bgColor || "#f0f0f0",
        border: "1px solid #ccc",
        borderRadius: "4px",
        width: totalWidth,
        height: totalHeight,
      }}
    >
      {layout.cells.map((cell, idx) => (
        <div
          key={idx}
          style={{
            gridColumn: `${cell.col + 1} / span ${cell.colSpan}`,
            gridRow: `${cell.row + 1} / span ${cell.rowSpan}`,
            backgroundColor: cell.bgColor || "#e0e0e0",
            border: "1px solid #999",
            borderRadius: "2px",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: "10px",
            fontWeight: "bold",
            color: "#333",
            padding: "4px",
            textAlign: "center",
            overflow: "hidden",
            whiteSpace: "nowrap",
          }}
        >
          {cell.content}
        </div>
      ))}
    </div>
  );
};

/**
 * ImageFitModeVisualizer: Shows how Image fits in different modes
 */
export const ImageFitModeVisualizer: React.FC<{ fitMode: "stretch" | "contain" | "cover" }> = ({
  fitMode,
}) => {
  const modes = {
    stretch: "Ignore aspect ratio, fill container",
    contain: "Preserve aspect ratio, fit within container",
    cover: "Preserve aspect ratio, fill container (crop edges)",
  };

  return (
    <div
      style={{
        border: "1px solid #ccc",
        borderRadius: "4px",
        padding: "8px",
        backgroundColor: "#f5f5f5",
        maxWidth: "300px",
      }}
    >
      <h4 style={{ margin: "0 0 8px 0" }}>Image Fit Mode</h4>
      <div
        style={{
          width: "200px",
          height: "120px",
          backgroundColor: "#e0e0e0",
          border: "2px dashed #999",
          borderRadius: "2px",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          marginBottom: "8px",
          fontSize: "12px",
          color: "#666",
        }}
      >
        [Image Preview: {fitMode}]
      </div>
      <p style={{ margin: "0", fontSize: "12px", color: "#555" }}>
        <strong>{fitMode}:</strong> {modes[fitMode]}
      </p>
    </div>
  );
};

/**
 * AudioPlaybackVisualizer: Shows audio playback state and controls
 */
export const AudioPlaybackVisualizer: React.FC<{ state: AudioState }> = ({ state }) => {
  const [displayTime, setDisplayTime] = useState(state.currentTime);

  useEffect(() => {
    setDisplayTime(state.currentTime);
  }, [state.currentTime]);

  const formatTime = (ms: number) => {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    return `${minutes}:${(seconds % 60).toString().padStart(2, "0")}`;
  };

  return (
    <div
      style={{
        border: "1px solid #ccc",
        borderRadius: "4px",
        padding: "12px",
        backgroundColor: "#f5f5f5",
        maxWidth: "300px",
      }}
    >
      <h4 style={{ margin: "0 0 12px 0" }}>Audio Playback</h4>

      <div style={{ marginBottom: "8px" }}>
        <p style={{ margin: "4px 0", fontSize: "12px", color: "#555" }}>
          <strong>Status:</strong> {state.isPlaying ? "▶ Playing" : "⏸ Stopped"}
        </p>
        {state.isLoaded && (
          <p style={{ margin: "4px 0", fontSize: "12px", color: "#555" }}>
            <strong>Clip ID:</strong> {state.clipId}
          </p>
        )}
      </div>

      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "8px",
          marginBottom: "8px",
          fontSize: "11px",
          color: "#555",
        }}
      >
        <span>{formatTime(displayTime)}</span>
        <div
          style={{
            flex: 1,
            height: "4px",
            backgroundColor: "#ddd",
            borderRadius: "2px",
            position: "relative",
            overflow: "hidden",
          }}
        >
          <div
            style={{
              height: "100%",
              backgroundColor: "#4CAF50",
              width: `${(displayTime / (state.duration || 1)) * 100}%`,
            }}
          />
        </div>
        <span>{formatTime(state.duration)}</span>
      </div>

      <div style={{ display: "flex", gap: "8px" }}>
        <button
          style={{
            padding: "6px 12px",
            backgroundColor: "#4CAF50",
            color: "white",
            border: "none",
            borderRadius: "2px",
            cursor: "pointer",
            fontSize: "11px",
            flex: 1,
          }}
        >
          {state.isPlaying ? "⏸ Pause" : "▶ Play"}
        </button>
        <button
          style={{
            padding: "6px 12px",
            backgroundColor: "#f44336",
            color: "white",
            border: "none",
            borderRadius: "2px",
            cursor: "pointer",
            fontSize: "11px",
            flex: 1,
          }}
        >
          ⏹ Stop
        </button>
      </div>
    </div>
  );
};

/**
 * LuminaVisualizerPanel: Main panel for Sentinel showing all Lumina features
 */
export const LuminaVisualizerPanel: React.FC<{ state: LuminaVisualizerState }> = ({ state }) => {
  return (
    <div
      style={{
        padding: "16px",
        backgroundColor: "#fafafa",
        fontFamily: "system-ui, -apple-system, sans-serif",
        maxHeight: "100vh",
        overflowY: "auto",
      }}
    >
      <h2 style={{ margin: "0 0 16px 0", fontSize: "16px", color: "#333" }}>
        Lumina UI Inspector
      </h2>

      {state.gridLayout && (
        <div style={{ marginBottom: "24px" }}>
          <h3 style={{ margin: "0 0 12px 0", fontSize: "13px", color: "#555" }}>
            Grid Layout
          </h3>
          <GridLayoutVisualizer layout={state.gridLayout} />
          <div style={{ marginTop: "8px", fontSize: "11px", color: "#888" }}>
            <p style={{ margin: "4px 0" }}>
              Columns: {state.gridLayout.columns} | Gap: {state.gridLayout.gap}px |
              Padding: {state.gridLayout.padding}px
            </p>
          </div>
        </div>
      )}

      {state.imageFitMode && (
        <div style={{ marginBottom: "24px" }}>
          <h3 style={{ margin: "0 0 12px 0", fontSize: "13px", color: "#555" }}>
            Image Rendering
          </h3>
          <ImageFitModeVisualizer fitMode={state.imageFitMode} />
        </div>
      )}

      {state.audioState && (
        <div style={{ marginBottom: "24px" }}>
          <h3 style={{ margin: "0 0 12px 0", fontSize: "13px", color: "#555" }}>
            Audio Playback
          </h3>
          <AudioPlaybackVisualizer state={state.audioState} />
        </div>
      )}

      {!state.gridLayout && !state.imageFitMode && !state.audioState && (
        <div
          style={{
            padding: "16px",
            backgroundColor: "#e3f2fd",
            borderRadius: "4px",
            color: "#1976d2",
            fontSize: "12px",
          }}
        >
          No Lumina UI active. Run a program using Grid, Image, or Audio to see live previews here.
        </div>
      )}
    </div>
  );
};

export default LuminaVisualizerPanel;
