/// Fast project indexing with incremental updates using merkle-based dependency tracking.
///
/// This module enables the Sentinel IDE to:
/// 1. Lazy-load only visible files initially (not whole project)
/// 2. Track file dependencies via merkle hashes (quick diffs)
/// 3. Invalidate only affected modules on file change

import { EventEmitter } from 'events';

export interface FileMetadata {
  path: string;
  module_name: string;
  mtime: number;
  merkle_hash: string;
  dependencies: Set<string>;
}

export interface ProjectIndex {
  files: Map<string, FileMetadata>;
  root_path: string;
  is_dirty: boolean;
}

export class IncrementalProjectIndexer extends EventEmitter {
  private index: ProjectIndex;
  private visible_paths: Set<string>;
  private pending_scans: Map<string, NodeJS.Timeout>;

  constructor(root_path: string) {
    super();
    this.index = {
      files: new Map(),
      root_path,
      is_dirty: false,
    };
    this.visible_paths = new Set();
    this.pending_scans = new Map();
  }

  /**
   * Load only the visible files (from tree view) initially.
   * This avoids scanning the entire project on startup.
   */
  async lazy_load_visible(paths: string[]): Promise<Map<string, FileMetadata>> {
    const loaded = new Map<string, FileMetadata>();

    for (const path of paths) {
      const metadata = await this.scan_file(path);
      if (metadata) {
        this.index.files.set(path, metadata);
        this.visible_paths.add(path);
        loaded.set(path, metadata);
      }
    }

    this.emit('lazy_load_complete', { count: loaded.size });
    return loaded;
  }

  /**
   * Scan a single file and extract dependencies + compute merkle hash.
   */
  private async scan_file(path: string): Promise<FileMetadata | null> {
    try {
      // In real implementation, use fs.stat and file content parsing
      const mtime = Date.now();
      const merkle_hash = await this.compute_merkle_hash(path);

      return {
        path,
        module_name: this.extract_module_name(path),
        mtime,
        merkle_hash,
        dependencies: new Set(),
      };
    } catch (e) {
      return null;
    }
  }

  /**
   * Compute a merkle-like hash of file content for quick equality checks.
   * In real impl, would read file and compute hash; here simulated.
   */
  private async compute_merkle_hash(path: string): Promise<string> {
    // Simulated hash - in practice, read file and compute SHA256 or similar
    return `hash_${path}_${Date.now()}`;
  }

  /**
   * Extract module name from file path (e.g., src/math.aura -> math).
   */
  private extract_module_name(path: string): string {
    const parts = path.split(/[\\/]/);
    const file = parts[parts.length - 1];
    return file.replace(/\.aura$/, '');
  }

  /**
   * Register a file path as visible (e.g., in tree view).
   * If not yet indexed, load it. Otherwise, check for changes.
   */
  async mark_visible(path: string): Promise<void> {
    this.visible_paths.add(path);

    if (!this.index.files.has(path)) {
      const metadata = await this.scan_file(path);
      if (metadata) {
        this.index.files.set(path, metadata);
      }
    } else {
      // Check if file has changed since last scan
      await this.check_file_changed(path);
    }
  }

  /**
   * Mark a file as no longer visible (e.g., collapsed in tree).
   * Can optionally unload from memory to free resources.
   */
  mark_invisible(path: string): void {
    this.visible_paths.delete(path);
    this.emit('file_unloaded', { path });
  }

  /**
   * Check if a file's merkle hash has changed since last indexed.
   * If changed, invalidate dependents.
   */
  private async check_file_changed(path: string): Promise<void> {
    const current_hash = await this.compute_merkle_hash(path);
    const cached = this.index.files.get(path);

    if (cached && cached.merkle_hash !== current_hash) {
      // File has changed - invalidate all modules that depend on it
      this.invalidate_dependents(path);
      cached.merkle_hash = current_hash;
      this.index.is_dirty = true;
      this.emit('file_changed', { path });
    }
  }

  /**
   * Find all files that depend on `file_path` and mark them for re-verification.
   */
  private invalidate_dependents(file_path: string): void {
    const to_invalidate = new Set<string>();

    for (const [path, meta] of this.index.files.entries()) {
      if (meta.dependencies.has(file_path)) {
        to_invalidate.add(path);
      }
    }

    for (const path of to_invalidate) {
      this.emit('invalidate', { path });
    }
  }

  /**
   * Incrementally re-index changed files.
   * Uses debouncing to avoid excessive rescans during rapid edits.
   */
  schedule_rescan(path: string, delay_ms: number = 500): void {
    // Cancel any pending rescan for this file
    if (this.pending_scans.has(path)) {
      clearTimeout(this.pending_scans.get(path)!);
    }

    // Schedule new scan after delay
    const timeout = setTimeout(async () => {
      await this.check_file_changed(path);
      this.pending_scans.delete(path);
    }, delay_ms);

    this.pending_scans.set(path, timeout);
  }

  /**
   * Get index statistics for display (e.g., in status bar).
   */
  get_stats(): { total_files: number; visible_files: number; dirty: boolean } {
    return {
      total_files: this.index.files.size,
      visible_files: this.visible_paths.size,
      dirty: this.index.is_dirty,
    };
  }

  /**
   * Clear dirty flag after full re-index completes.
   */
  mark_clean(): void {
    this.index.is_dirty = false;
    this.emit('reindex_complete');
  }
}

/**
 * Export for testing and usage in main Sentinel app.
 */
export default IncrementalProjectIndexer;
