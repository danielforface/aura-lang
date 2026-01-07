// Minimal Inlay Hint typings for the Aura Sentinel extension.
// (The runtime VS Code API supports these; this file unblocks TypeScript builds.)

declare module "vscode" {
  export enum InlayHintKind {
    Type = 1,
    Parameter = 2,
  }

  export class InlayHint {
    position: Position;
    label: string;
    kind?: InlayHintKind;
    paddingLeft?: boolean;
    paddingRight?: boolean;

    constructor(position: Position, label: string, kind?: InlayHintKind);
  }

  export interface InlayHintsProvider {
    onDidChangeInlayHints?: Event<void>;
    provideInlayHints(
      document: TextDocument,
      range: Range,
      token: CancellationToken
    ): ProviderResult<InlayHint[]>;
  }

  export namespace languages {
    export function registerInlayHintsProvider(
      selector: DocumentSelector,
      provider: InlayHintsProvider
    ): Disposable;
  }
}
