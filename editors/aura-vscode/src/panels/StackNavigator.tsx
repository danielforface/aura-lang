/// Stack Frame Navigation UI Component
/// React component for navigating call stacks and frame inspection

import React, { useState, useCallback } from 'react';
import './stack_navigator.css';

interface StackFrame {
  id: number;
  level: number;
  func: string;
  file: string;
  fullname: string;
  line: number;
  addr: string;
  args: Array<{ name: string; value: string }>;
  locals?: Array<{ name: string; value: string; type: string }>;
}

interface StackNavigatorProps {
  frames: StackFrame[];
  currentFrame?: number;
  onFrameSelect?: (frame: StackFrame) => void;
  onJumpToLocation?: (file: string, line: number) => void;
}

const StackFrameCard: React.FC<{
  frame: StackFrame;
  isActive: boolean;
  onSelect: (frame: StackFrame) => void;
  onJump: (file: string, line: number) => void;
}> = ({ frame, isActive, onSelect, onJump }) => {
  const [expanded, setExpanded] = useState(isActive);

  const handleLocationClick = () => {
    onJump(frame.fullname, frame.line);
  };

  return (
    <div className={`frame-card ${isActive ? 'active' : ''}`} onClick={() => onSelect(frame)}>
      <div className="frame-header">
        <div className="frame-level">{frame.level}</div>
        <div className="frame-function">
          <span className="function-name">{frame.func}</span>
          {frame.args.length > 0 && (
            <span className="arg-count">
              ({frame.args.length} {frame.args.length === 1 ? 'arg' : 'args'})
            </span>
          )}
        </div>
        <div className="frame-location" onClick={handleLocationClick} title="Click to jump to source">
          <span className="file-name">{frame.file}</span>
          <span className="line-number">:{frame.line}</span>
        </div>
        <button
          className={`expand-button ${expanded ? 'expanded' : ''}`}
          onClick={(e) => {
            e.stopPropagation();
            setExpanded(!expanded);
          }}
          title={expanded ? 'Collapse' : 'Expand'}
        >
          ▼
        </button>
      </div>

      {expanded && (
        <div className="frame-details">
          {/* Arguments Section */}
          {frame.args.length > 0 && (
            <div className="frame-section">
              <div className="section-header">
                <span className="section-title">Arguments</span>
                <span className="count">{frame.args.length}</span>
              </div>
              <div className="parameters-list">
                {frame.args.map((arg, idx) => (
                  <div key={idx} className="parameter-item">
                    <span className="param-name">{arg.name}</span>
                    <span className="param-separator">=</span>
                    <span className="param-value">{arg.value}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Local Variables Section */}
          {frame.locals && frame.locals.length > 0 && (
            <div className="frame-section">
              <div className="section-header">
                <span className="section-title">Local Variables</span>
                <span className="count">{frame.locals.length}</span>
              </div>
              <div className="locals-list">
                {frame.locals.map((local, idx) => (
                  <div key={idx} className="local-item">
                    <span className="local-name">{local.name}</span>
                    <span className="local-type">{local.type}</span>
                    <span className="local-separator">=</span>
                    <span className="local-value">{local.value}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Address Section */}
          <div className="frame-section">
            <div className="address-info">
              <span className="address-label">Memory:</span>
              <span className="address-value">{frame.addr}</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

const StackNavigator: React.FC<StackNavigatorProps> = ({
  frames,
  currentFrame = 0,
  onFrameSelect,
  onJumpToLocation,
}) => {
  const [activeFrame, setActiveFrame] = useState(currentFrame);
  const [filter, setFilter] = useState<'all' | 'aura' | 'system'>('all');

  const handleFrameSelect = useCallback(
    (frame: StackFrame) => {
      setActiveFrame(frame.level);
      onFrameSelect?.(frame);
    },
    [onFrameSelect],
  );

  const handleJump = useCallback(
    (file: string, line: number) => {
      onJumpToLocation?.(file, line);
    },
    [onJumpToLocation],
  );

  const isAuraFrame = (frame: StackFrame): boolean => {
    return frame.file.endsWith('.aura') || frame.fullname.includes('.aura');
  };

  const filteredFrames = frames.filter((frame) => {
    if (filter === 'all') return true;
    if (filter === 'aura') return isAuraFrame(frame);
    if (filter === 'system') return !isAuraFrame(frame);
    return true;
  });

  const stats = {
    total: frames.length,
    aura: frames.filter(isAuraFrame).length,
    system: frames.filter((f) => !isAuraFrame(f)).length,
  };

  return (
    <div className="stack-navigator">
      {/* Header */}
      <div className="navigator-header">
        <h2>Call Stack</h2>
        <div className="stack-stats">
          <span className="stat">
            {stats.total} frame{stats.total !== 1 ? 's' : ''}
          </span>
          {stats.aura > 0 && <span className="stat-aura">{stats.aura} Aura</span>}
          {stats.system > 0 && <span className="stat-system">{stats.system} System</span>}
        </div>
      </div>

      {/* Filters */}
      <div className="navigator-controls">
        <div className="frame-filters">
          {(['all', 'aura', 'system'] as const).map((filterType) => (
            <button
              key={filterType}
              className={`filter-button ${filter === filterType ? 'active' : ''}`}
              onClick={() => setFilter(filterType)}
              title={`Show ${filterType} frames`}
            >
              {filterType.charAt(0).toUpperCase() + filterType.slice(1)}
              {filterType !== 'all' && (
                <span className="filter-count">
                  {filterType === 'aura' ? stats.aura : stats.system}
                </span>
              )}
            </button>
          ))}
        </div>
      </div>

      {/* Stack Frames */}
      <div className="frames-container">
        {filteredFrames.length > 0 ? (
          <div className="frames-list">
            {filteredFrames.map((frame) => (
              <StackFrameCard
                key={frame.id}
                frame={frame}
                isActive={activeFrame === frame.level}
                onSelect={handleFrameSelect}
                onJump={handleJump}
              />
            ))}
          </div>
        ) : (
          <div className="empty-state">
            <div className="empty-icon">⚪</div>
            <div className="empty-message">No frames match the current filter</div>
          </div>
        )}
      </div>

      {/* Info Panel */}
      {filteredFrames.length > 0 && (
        <div className="navigator-info">
          <div className="info-header">Keyboard Navigation</div>
          <div className="info-items">
            <div className="info-item">
              <kbd>↑</kbd>
              <span>Previous frame</span>
            </div>
            <div className="info-item">
              <kbd>↓</kbd>
              <span>Next frame</span>
            </div>
            <div className="info-item">
              <kbd>Enter</kbd>
              <span>Jump to source</span>
            </div>
            <div className="info-item">
              <kbd>Ctrl+Click</kbd>
              <span>Expand details</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default StackNavigator;
