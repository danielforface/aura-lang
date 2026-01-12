# ✅ Sentinel App Enhancement Roadmap Added

## Summary

Added **32 detailed steps** to ROADMAP.md for comprehensive Sentinel App stability and advanced file management features (v1.1+ Enhancement).

---

## What Was Added

### File Size Update
- **Before:** 780 lines
- **After:** 953 lines
- **Added:** 173 lines of detailed enhancement steps

### New Section
**Location:** Sentinel IDE → Advanced File Explorer & Virtual File Manager (v1.1+ Enhancement)

---

## 32 Implementation Steps Organized by Category

### Category 1: File Tree Explorer Enhancements (Steps 1-5)

**Step 1: Hierarchical Folder Tree Visualization**
- Display full folder structure on initial folder open
- Lazy-load subfolders (expand on click)
- Show file count indicators per folder
- Support collapsible/expandable folder sections

**Step 2: Visual Folder Hierarchy with Icons**
- Folder icons (open/closed states)
- File type icons (code, document, media, etc.)
- Recursive nested view with proper indentation
- Breadcrumb navigation above tree

**Step 3: Folder Navigation with Drag-and-Drop**
- Drag folders to reorder
- Expand/collapse with arrow clicks
- Double-click to navigate into folder
- Single-click to select folder

**Step 4: File Filtering and Search in Tree**
- Search box above tree (filters visible files)
- Show/hide hidden files toggle
- Filter by file type (*.ts, *.aura, etc.)
- Highlight matching files in real-time

**Step 5: Breadcrumb Navigation**
- Show current folder path
- Click any breadcrumb segment to jump to that folder
- Copy full path button
- Home/root folder quick access

### Category 2: Recent Folders History (Steps 6-10)

**Step 6: Recent Folders List in Sidebar**
- Store up to 10 most recent folders
- Persist to user settings/config file
- Show folder path + last access time
- Pin/unpin favorite folders

**Step 7: Quick Folder Switcher**
- Dropdown menu showing recent folders
- One-click switch between folders
- Star/unstar folder for favorites
- Clear history option

**Step 8: Folder Bookmark Management**
- Bookmarks section in file explorer
- Add current folder to bookmarks button
- Drag bookmarks to reorder
- Remove bookmark option

**Step 9: Folder History Navigation**
- Back/forward buttons for folder history
- Keyboard shortcuts (Alt+Left, Alt+Right)
- History dropdown with hover previews
- Clear history option

**Step 10: Recently Used Folders Indicator**
- Show access frequency/rank
- Display last modified date
- Show folder size info
- Quick stats on hover

### Category 3: Session-Based File Management (Steps 11-15)

**Step 11: Per-Folder File Session Storage**
- Save open files per folder
- Store tab order and scroll positions
- Save active tab indicator
- Persist to JSON config file per folder

**Step 12: Automatic File Restoration on Folder Switch**
- When switching folders, close previous folder's files
- Automatically open saved files for new folder
- Restore tab order and positions
- Restore scroll position for each file

**Step 13: File Tab Memory System**
- Create tab groups per folder
- Remember which tabs were open
- Store file edit state (modified/saved)
- Quick access to recently closed tabs

**Step 14: Session Import/Export**
- Export current folder session as JSON
- Import previous sessions
- Compare session snapshots
- Merge sessions from multiple folders

**Step 15: Session Cleanup and Organization**
- Auto-cleanup sessions for deleted folders
- Merge duplicate sessions
- Archive old sessions
- Session statistics dashboard

### Category 4: Virtual File Manager Operations (Steps 16-20)

**Step 16: Copy File/Folder Operation**
- Right-click context menu option
- Keyboard shortcut (Ctrl+C)
- Copy to clipboard (filepath)
- Paste in same or different folder (Ctrl+V)
- Show progress for large files

**Step 17: Delete File/Folder with Trash**
- Right-click delete option
- Confirmation dialog before delete
- Move to trash (recoverable)
- Permanent delete option
- Empty trash functionality

**Step 18: Rename File/Folder**
- Right-click rename option
- F2 keyboard shortcut
- Inline edit with validation
- Prevent duplicate names
- Keep file extension when renaming

**Step 19: Create New File/Folder**
- Right-click "New File" option
- Right-click "New Folder" option
- Keyboard shortcuts
- Inline naming dialog
- Default file templates (.aura, .ts, etc.)

**Step 20: Drag-and-Drop File Operations**
- Drag file to move
- Drag folder to move with contents
- Ctrl+drag to copy
- Drag between folders
- Visual drop target indicators

### Category 5: Document Change History & Tracking (Steps 21-25)

**Step 21: Document Change History Tracking**
- Track all modifications per document
- Store timestamp, author, change type
- Maintain change log per file
- Persist history to .history folder

**Step 22: Visual Change Indicators in Editor**
- Highlight changed lines in editor gutter
- Show change type (added/modified/deleted)
- Color code: green (new), yellow (modified), red (deleted)
- Hover to show change details

**Step 23: Change Summary Panel**
- Show list of all changes in current file
- Click to jump to changed line
- Show before/after code snippets
- Group changes by type

**Step 24: Change History Viewer**
- Timeline view of all changes
- Show previous versions of file
- Diff view between versions
- Restore previous version option

**Step 25: Change Marks and Annotations**
- Mark important changes
- Add inline comments/annotations
- Tag changes with keywords
- Filter changes by tag/date/author

### Category 6: Clean File View on Folder Switch (Steps 26-28)

**Step 26: Clean State Reset on New Folder**
- Clear all unsaved files when switching folders
- Prompt to save modified files
- Close all tabs from previous folder
- Reset scroll and view positions

**Step 27: Startup File View for New Folders**
- Show welcome panel for empty folders
- Display README file if exists
- Show project structure overview
- Suggest creating first file/folder

**Step 28: View State Isolation Per Folder**
- Each folder has independent window state
- Separate sidebar configuration per folder
- Individual file explorer settings
- Isolated panel layouts

### Category 7: Sentinel App Stability & UX (Steps 29-32)

**Step 29: Robust Error Handling**
- Handle file system errors gracefully
- Show user-friendly error messages
- Retry failed operations
- Log errors for debugging

**Step 30: Performance Monitoring**
- Track file operation times
- Monitor folder switching latency
- Track memory usage
- Profile session restoration

**Step 31: Undo/Redo for File Operations**
- Undo last file operation
- Redo undone operations
- Limit undo history (configurable)
- Show undo/redo history

**Step 32: Configuration Management**
- Settings for auto-save behavior
- History retention settings
- Session management options
- UI customization settings

---

## Implementation Priority & Sequencing

### Phase 1: Core File Explorer (Steps 1-5)
- **Priority:** P0 (Critical for basic functionality)
- **Effort:** 2-3 weeks
- **Dependencies:** File system APIs
- **Testing:** Unit tests for tree rendering, navigation

### Phase 2: Recent Folders & Navigation (Steps 6-10)
- **Priority:** P0 (Essential for productivity)
- **Effort:** 1-2 weeks
- **Dependencies:** Persistent storage, settings
- **Testing:** Integration tests for history, switching

### Phase 3: Session Management (Steps 11-15)
- **Priority:** P1 (Important for workflow)
- **Effort:** 2-3 weeks
- **Dependencies:** Session storage, cleanup logic
- **Testing:** Session tests, data persistence tests

### Phase 4: Virtual File Manager (Steps 16-20)
- **Priority:** P1 (Standard IDE features)
- **Effort:** 3-4 weeks
- **Dependencies:** File system operations, confirmation dialogs
- **Testing:** File operation tests, drag-drop tests

### Phase 5: Change History & Tracking (Steps 21-25)
- **Priority:** P2 (Advanced feature)
- **Effort:** 3-4 weeks
- **Dependencies:** Change detection, diff engine
- **Testing:** Change tracking tests, version comparison

### Phase 6: Folder Switching & Cleanup (Steps 26-28)
- **Priority:** P1 (Stability critical)
- **Effort:** 1-2 weeks
- **Dependencies:** State management, cleanup logic
- **Testing:** State isolation tests, cleanup tests

### Phase 7: Stability & Configuration (Steps 29-32)
- **Priority:** P0 (Critical across all phases)
- **Effort:** Ongoing (throughout all phases)
- **Dependencies:** Logging, monitoring infrastructure
- **Testing:** Error handling tests, performance tests

---

## Technology Stack Recommendations

### Frontend (Sentinel App)
- **Framework:** React with TypeScript
- **State Management:** Redux or Zustand
- **File Tree:** react-simple-tree-menu or custom implementation
- **Drag-Drop:** react-dnd or custom handlers
- **Session Storage:** IndexedDB + localStorage

### Backend (LSP/aura-lsp)
- **File Operations:** std::fs with tokio async
- **Session Persistence:** JSON file format
- **Change Tracking:** Line-based diff algorithm
- **History Storage:** Git-like object store

### Storage Format
```json
{
  "recentFolders": [
    {
      "path": "/path/to/folder",
      "lastAccessed": "2026-01-08T10:30:00Z",
      "frequency": 5,
      "isPinned": true
    }
  ],
  "folderSessions": {
    "/path/to/folder": {
      "openFiles": ["file1.aura", "file2.ts"],
      "activeTab": "file1.aura",
      "scrollPositions": {"file1.aura": 150},
      "fileStates": {"file1.aura": "modified"}
    }
  },
  "changeHistory": {
    "file.aura": [
      {
        "timestamp": "2026-01-08T10:30:00Z",
        "type": "modified",
        "lines": [10, 15],
        "author": "user",
        "snippet": "..."
      }
    ]
  }
}
```

---

## Success Criteria

### Phase 1 (Steps 1-5)
- ✓ Tree rendering performant (<100ms for 1000 files)
- ✓ Navigation smooth (< 50ms per action)
- ✓ All icons displaying correctly
- ✓ Search working in real-time

### Phase 2 (Steps 6-10)
- ✓ Recent list persisted across sessions
- ✓ Folder switching < 200ms
- ✓ History dropdown populated instantly
- ✓ Bookmarks functional and reorderable

### Phase 3 (Steps 11-15)
- ✓ Sessions saved automatically
- ✓ Files restored on folder switch
- ✓ Session data integrity validated
- ✓ History cleanup working

### Phase 4 (Steps 16-20)
- ✓ Copy/delete/rename operations atomic
- ✓ Drag-drop working smoothly
- ✓ Undo stack maintained
- ✓ User confirmation dialogs shown

### Phase 5 (Steps 21-25)
- ✓ Change tracking 100% accurate
- ✓ History viewer responsive
- ✓ Diffs generated correctly
- ✓ Version restore working

### Phase 6 (Steps 26-28)
- ✓ Folder switch clean (<200ms)
- ✓ No file leaks between folders
- ✓ State properly isolated
- ✓ Welcome panel displays

### Phase 7 (Steps 29-32)
- ✓ Error messages clear and actionable
- ✓ Performance metrics logged
- ✓ Undo/redo state consistent
- ✓ Settings persisted

---

## Estimated Development Timeline

| Phase | Steps | Duration | Status |
|-------|-------|----------|--------|
| Phase 1 | 1-5 | 2-3 weeks | Planned (v1.1) |
| Phase 2 | 6-10 | 1-2 weeks | Planned (v1.1) |
| Phase 3 | 11-15 | 2-3 weeks | Planned (v1.1) |
| Phase 4 | 16-20 | 3-4 weeks | Planned (v1.2) |
| Phase 5 | 21-25 | 3-4 weeks | Planned (v1.2) |
| Phase 6 | 26-28 | 1-2 weeks | Planned (v1.1) |
| Phase 7 | 29-32 | Ongoing | All phases |
| **Total** | **32** | **13-19 weeks** | **v1.1-v1.2** |

---

## Integration Points

### LSP Integration (aura-lsp)
- File operations service
- Session persistence service
- Change tracking service
- History management service

### Sentinel UI Integration (editors/sentinel-app)
- File tree component
- Recent folders panel
- Tab group component
- Change tracking panel

### VS Code Extension Integration (editors/aura-vscode)
- File explorer customization
- Session sync across editors
- Shared settings

---

## Documentation Requirements

- [ ] User guide for file explorer
- [ ] Session management documentation
- [ ] Virtual file manager reference
- [ ] Change history usage guide
- [ ] Keyboard shortcuts documentation
- [ ] Configuration options guide

---

## Testing Strategy

### Unit Tests
- File operation logic (copy, delete, rename)
- Session serialization/deserialization
- Change detection algorithm
- History cleanup logic

### Integration Tests
- File explorer + LSP communication
- Session persistence + restoration
- Folder switching workflow
- File operations with undo/redo

### E2E Tests
- Complete workflow: open folder → edit files → switch folder → restore session
- All file operations in sequence
- Large file handling
- Performance benchmarks

### Performance Tests
- File tree rendering (1000+ files)
- Folder switching latency
- Session restoration time
- Change tracking overhead

---

## Summary

Added **32 comprehensive implementation steps** to ROADMAP.md covering:
- ✅ File tree explorer (5 steps)
- ✅ Recent folders history (5 steps)
- ✅ Session-based file management (5 steps)
- ✅ Virtual file manager (5 steps)
- ✅ Change history tracking (5 steps)
- ✅ Clean folder switching (3 steps)
- ✅ Stability & configuration (4 steps)

**Total Roadmap Size:** 953 lines (was 780)  
**Lines Added:** 173 (Sentinel App section)  
**Implementation Duration:** 13-19 weeks (v1.1-v1.2)  
**Status:** Ready for development planning

---

**Update Complete** ✅  
**All Steps Documented** ✅  
**Ready for Implementation Planning** ✅
