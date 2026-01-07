/// Progress indicator component for project indexing.
/// Shows a bar during initial load and updates on file changes.

export interface ProgressBar {
  element: HTMLElement;
  show(): void;
  hide(): void;
  update_progress(current: number, total: number): void;
  set_message(msg: string): void;
}

export function create_progress_bar(): ProgressBar {
  const container = document.createElement('div');
  container.id = 'project-indexing-progress';
  container.style.cssText = `
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 3px;
    background-color: transparent;
    z-index: 9999;
    transition: opacity 0.3s ease;
    opacity: 0;
  `;

  const bar = document.createElement('div');
  bar.style.cssText = `
    height: 100%;
    background: linear-gradient(90deg, #4CAF50, #8BC34A);
    width: 0%;
    transition: width 0.2s ease;
  `;
  container.appendChild(bar);

  const message = document.createElement('div');
  message.style.cssText = `
    position: absolute;
    top: 5px;
    left: 10px;
    font-size: 12px;
    color: #999;
    font-family: monospace;
  `;
  container.appendChild(message);

  document.body.appendChild(container);

  return {
    element: container,
    show(): void {
      container.style.opacity = '1';
    },
    hide(): void {
      container.style.opacity = '0';
    },
    update_progress(current: number, total: number): void {
      const percent = total > 0 ? (current / total) * 100 : 0;
      bar.style.width = `${percent}%`;
      message.textContent = `Indexing: ${current}/${total}`;
    },
    set_message(msg: string): void {
      message.textContent = msg;
    },
  };
}

/**
 * Hook progress bar updates to indexer events.
 */
export function wire_progress_to_indexer(
  indexer: any,
  progress: ProgressBar,
): void {
  indexer.on('lazy_load_complete', (evt: any) => {
    progress.update_progress(evt.count, evt.count);
    progress.set_message(`Initial load: ${evt.count} files`);
  });

  indexer.on('file_changed', (evt: any) => {
    progress.show();
    progress.set_message(`Indexing changes in ${evt.path}...`);
  });

  indexer.on('invalidate', (evt: any) => {
    progress.set_message(`Invalidating ${evt.path}...`);
  });

  indexer.on('reindex_complete', () => {
    progress.hide();
  });
}
