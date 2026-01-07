/// Variable Inspector UI Panel Component
/// React component for displaying and exploring variables in the debugger

import React, { useState, useCallback, useMemo } from 'react';
import './variable_inspector.css';

interface Variable {
  name: string;
  type: string;
  value: string;
  address?: string;
  size?: number;
  scope: 'local' | 'global' | 'parameter';
  children?: Variable[];
  expanded?: boolean;
  depth: number;
}

interface VariableInspectorProps {
  variables: Variable[];
  onSelectVariable?: (variable: Variable) => void;
  onValueChange?: (variable: Variable, newValue: string) => void;
}

interface VariableNodeProps {
  variable: Variable;
  onToggle: (name: string) => void;
  onSelect: (variable: Variable) => void;
  onValueChange: (variable: Variable, newValue: string) => void;
}

const VariableNode: React.FC<VariableNodeProps> = ({
  variable,
  onToggle,
  onSelect,
  onValueChange,
}) => {
  const [isEditing, setIsEditing] = useState(false);
  const [editValue, setEditValue] = useState(variable.value);

  const hasChildren = variable.children && variable.children.length > 0;
  const isExpandable = hasChildren || variable.type.includes('*') || variable.type.includes('[');

  const handleToggle = (e: React.MouseEvent) => {
    e.stopPropagation();
    onToggle(variable.name);
  };

  const handleSelect = () => {
    onSelect(variable);
  };

  const handleValueChange = () => {
    if (editValue !== variable.value) {
      onValueChange(variable, editValue);
    }
    setIsEditing(false);
  };

  const getScopeIcon = (): string => {
    switch (variable.scope) {
      case 'local':
        return '📦';
      case 'global':
        return '🌍';
      case 'parameter':
        return '📥';
      default:
        return '○';
    }
  };

  const getTypeColor = (): string => {
    if (variable.type.includes('int')) return 'type-int';
    if (variable.type.includes('float') || variable.type.includes('double')) return 'type-float';
    if (variable.type.includes('char') || variable.type.includes('str')) return 'type-string';
    if (variable.type.includes('*')) return 'type-pointer';
    if (variable.type.includes('bool')) return 'type-bool';
    return 'type-default';
  };

  return (
    <div className="variable-node" style={{ marginLeft: `${variable.depth * 16}px` }}>
      <div className="variable-header" onClick={handleSelect}>
        {isExpandable && (
          <button
            className={`expand-button ${variable.expanded ? 'expanded' : ''}`}
            onClick={handleToggle}
            title={variable.expanded ? 'Collapse' : 'Expand'}
          >
            ▶
          </button>
        )}
        {!isExpandable && <div className="expand-placeholder" />}

        <span className="scope-icon" title={variable.scope}>
          {getScopeIcon()}
        </span>

        <span className="variable-name">{variable.name}</span>

        <span className={`variable-type ${getTypeColor()}`}>{variable.type}</span>

        {variable.address && (
          <span className="variable-address" title="Memory address">
            @{variable.address}
          </span>
        )}

        {variable.size && (
          <span className="variable-size" title="Size in bytes">
            ({variable.size}B)
          </span>
        )}

        <span className="variable-separator">=</span>

        {isEditing ? (
          <input
            className="variable-value-edit"
            value={editValue}
            onChange={(e) => setEditValue(e.target.value)}
            onBlur={handleValueChange}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleValueChange();
              if (e.key === 'Escape') setIsEditing(false);
            }}
            autoFocus
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          <span
            className="variable-value"
            onDoubleClick={() => {
              setIsEditing(true);
            }}
            title="Double-click to edit"
          >
            {variable.value}
          </span>
        )}
      </div>

      {variable.expanded && hasChildren && (
        <div className="variable-children">
          {variable.children!.map((child) => (
            <VariableNode
              key={child.name}
              variable={child}
              onToggle={onToggle}
              onSelect={onSelect}
              onValueChange={onValueChange}
            />
          ))}
        </div>
      )}
    </div>
  );
};

const VariableInspector: React.FC<VariableInspectorProps> = ({
  variables,
  onSelectVariable,
  onValueChange,
}) => {
  const [expandedVars, setExpandedVars] = useState<Set<string>>(new Set());
  const [selectedVar, setSelectedVar] = useState<Variable | null>(null);
  const [filterScope, setFilterScope] = useState<'all' | 'local' | 'global' | 'parameter'>(
    'all',
  );
  const [searchQuery, setSearchQuery] = useState('');

  const handleToggle = useCallback((name: string) => {
    setExpandedVars((prev) => {
      const next = new Set(prev);
      if (next.has(name)) {
        next.delete(name);
      } else {
        next.add(name);
      }
      return next;
    });
  }, []);

  const handleSelect = useCallback(
    (variable: Variable) => {
      setSelectedVar(variable);
      onSelectVariable?.(variable);
    },
    [onSelectVariable],
  );

  const handleValueChange = useCallback(
    (variable: Variable, newValue: string) => {
      onValueChange?.(variable, newValue);
    },
    [onValueChange],
  );

  const enhancedVariables = useMemo(() => {
    return variables.map((v) => ({
      ...v,
      expanded: expandedVars.has(v.name),
    }));
  }, [variables, expandedVars]);

  const filteredVariables = useMemo(() => {
    return enhancedVariables.filter((v) => {
      // Filter by scope
      if (filterScope !== 'all' && v.scope !== filterScope) {
        return false;
      }

      // Filter by search query
      if (
        searchQuery &&
        !v.name.toLowerCase().includes(searchQuery.toLowerCase()) &&
        !v.type.toLowerCase().includes(searchQuery.toLowerCase()) &&
        !v.value.toLowerCase().includes(searchQuery.toLowerCase())
      ) {
        return false;
      }

      return true;
    });
  }, [enhancedVariables, filterScope, searchQuery]);

  const statsCount = {
    total: variables.length,
    local: variables.filter((v) => v.scope === 'local').length,
    global: variables.filter((v) => v.scope === 'global').length,
    parameter: variables.filter((v) => v.scope === 'parameter').length,
  };

  return (
    <div className="variable-inspector">
      <div className="inspector-header">
        <h2>Variable Inspector</h2>
        <div className="stats-badge">
          {statsCount.total} variables
        </div>
      </div>

      <div className="inspector-controls">
        <input
          type="text"
          className="search-input"
          placeholder="Search variables..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
        />

        <div className="scope-filters">
          {(['all', 'local', 'global', 'parameter'] as const).map((scope) => (
            <button
              key={scope}
              className={`scope-filter ${filterScope === scope ? 'active' : ''}`}
              onClick={() => setFilterScope(scope)}
              title={`Filter by ${scope}`}
            >
              {scope.charAt(0).toUpperCase() + scope.slice(1)}
              {scope !== 'all' && (
                <span className="count">({statsCount[scope === 'parameter' ? 'parameter' : scope]})</span>
              )}
            </button>
          ))}
        </div>
      </div>

      <div className="inspector-content">
        {filteredVariables.length > 0 ? (
          <div className="variables-list">
            {filteredVariables.map((variable) => (
              <VariableNode
                key={variable.name}
                variable={variable}
                onToggle={handleToggle}
                onSelect={handleSelect}
                onValueChange={handleValueChange}
              />
            ))}
          </div>
        ) : (
          <div className="empty-state">
            <div className="empty-icon">○</div>
            <div className="empty-message">No variables match the current filter</div>
          </div>
        )}
      </div>

      {selectedVar && (
        <div className="inspector-details">
          <div className="details-header">Variable Details</div>
          <div className="details-content">
            <div className="detail-row">
              <span className="detail-label">Name:</span>
              <span className="detail-value">{selectedVar.name}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Type:</span>
              <span className="detail-value">{selectedVar.type}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Scope:</span>
              <span className="detail-value">{selectedVar.scope}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Value:</span>
              <span className="detail-value">{selectedVar.value}</span>
            </div>
            {selectedVar.address && (
              <div className="detail-row">
                <span className="detail-label">Address:</span>
                <span className="detail-value">{selectedVar.address}</span>
              </div>
            )}
            {selectedVar.size && (
              <div className="detail-row">
                <span className="detail-label">Size:</span>
                <span className="detail-value">{selectedVar.size} bytes</span>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export default VariableInspector;
