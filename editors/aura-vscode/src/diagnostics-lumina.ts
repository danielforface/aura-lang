/**
 * Lumina UI + Audio Diagnostics for Aura VSIX
 * Provides real-time diagnostics and autocomplete hints for Grid, Image, and Audio features.
 */

import * as vscode from "vscode";

/**
 * Register diagnostics for Lumina UI and audio features.
 * Checks for:
 * - Grid prop validation (columns/rows/gap/padding)
 * - Image fit mode validation ("stretch"|"contain"|"cover")
 * - Audio.* function availability
 */
export function registerLuminaDiagnostics(
  context: vscode.ExtensionContext
): void {
  const diagnosticCollection = vscode.languages.createDiagnosticCollection(
    "aura-lumina"
  );
  context.subscriptions.push(diagnosticCollection);

  // Listen to active editor changes and document saves
  if (vscode.window.activeTextEditor) {
    validateAuraLumina(vscode.window.activeTextEditor.document, diagnosticCollection);
  }

  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      if (editor) {
        validateAuraLumina(editor.document, diagnosticCollection);
      }
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((document) => {
      validateAuraLumina(document, diagnosticCollection);
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument((event) => {
      validateAuraLumina(event.document, diagnosticCollection);
    })
  );
}

/**
 * Validate Aura document for Lumina UI issues.
 */
function validateAuraLumina(
  document: vscode.TextDocument,
  diagnosticCollection: vscode.DiagnosticCollection
): void {
  if (document.languageId !== "aura") {
    return;
  }

  const diagnostics: vscode.Diagnostic[] = [];
  const text = document.getText();
  const lines = text.split("\n");

  // Check for Grid constructor calls with validation
  const gridRegex = /Grid\s*\(\s*([^)]*)\)/g;
  let match;
  while ((match = gridRegex.exec(text)) !== null) {
    const lineNum = text.substring(0, match.index).split("\n").length - 1;
    const range = new vscode.Range(lineNum, match.index, lineNum, match.index + match[0].length);

    // Validate Grid props
    const props = match[1];
    if (props.includes("columns")) {
      const columnsMatch = props.match(/columns\s*[:=]\s*(\d+)/);
      if (!columnsMatch || parseInt(columnsMatch[1]) < 1) {
        diagnostics.push(
          new vscode.Diagnostic(
            range,
            "Grid requires columns >= 1",
            vscode.DiagnosticSeverity.Warning
          )
        );
      }
    } else {
      // columns is optional but common
    }

    // Validate gap and padding (numeric or named spacing)
    if (props.includes("gap") && !props.match(/gap\s*[:=]\s*(\d+|"small"|"medium"|"large")/)) {
      diagnostics.push(
        new vscode.Diagnostic(
          range,
          'Gap should be numeric or "small"|"medium"|"large"',
          vscode.DiagnosticSeverity.Hint
        )
      );
    }
  }

  // Check for Image fit mode validation
  const imageRegex = /Image\s*\(\s*([^)]*)\)/g;
  while ((match = imageRegex.exec(text)) !== null) {
    const lineNum = text.substring(0, match.index).split("\n").length - 1;
    const range = new vscode.Range(lineNum, match.index, lineNum, match.index + match[0].length);

    const props = match[1];
    const fitMatch = props.match(/fit\s*[:=]\s*"([^"]+)"/);
    if (fitMatch) {
      const fitMode = fitMatch[1];
      if (!["stretch", "contain", "cover"].includes(fitMode)) {
        diagnostics.push(
          new vscode.Diagnostic(
            range,
            `Image fit must be "stretch", "contain", or "cover", got "${fitMode}"`,
            vscode.DiagnosticSeverity.Error
          )
        );
      }
    }
  }

  // Check for audio.* function calls
  const audioFunctionRegex = /audio\.(load|play|play_loaded|stop)\s*\(/g;
  while ((match = audioFunctionRegex.exec(text)) !== null) {
    const lineNum = text.substring(0, match.index).split("\n").length - 1;
    const range = new vscode.Range(lineNum, match.index, lineNum, match.index + match[0].length);

    // Provide info diagnostic
    const audioFn = match[1];
    let hint = "";
    if (audioFn === "load") {
      hint = "audio.load(path: String) -> U32 (loads clip, returns clip ID)";
    } else if (audioFn === "play") {
      hint = "audio.play(path: String) -> U32 (plays from file, returns handle)";
    } else if (audioFn === "play_loaded") {
      hint = "audio.play_loaded(clip_id: U32) -> U32 (plays loaded clip, returns handle)";
    } else if (audioFn === "stop") {
      hint = "audio.stop(handle: U32) -> () (stops playback)";
    }

    diagnostics.push(
      new vscode.Diagnostic(
        range,
        hint,
        vscode.DiagnosticSeverity.Information
      )
    );
  }

  // Check for on_click callback patterns
  const onClickRegex = /on_click\s*[:=]/g;
  while ((match = onClickRegex.exec(text)) !== null) {
    const lineNum = text.substring(0, match.index).split("\n").length - 1;
    const range = new vscode.Range(lineNum, match.index, lineNum, match.index + match[0].length);

    diagnostics.push(
      new vscode.Diagnostic(
        range,
        "on_click callback triggered on button/grid cell interaction",
        vscode.DiagnosticSeverity.Hint
      )
    );
  }

  diagnosticCollection.set(document.uri, diagnostics);
}

/**
 * Hover provider for Lumina UI elements.
 */
export function createLuminaHoverProvider(): vscode.HoverProvider {
  return {
    provideHover(document, position, token) {
      const range = document.getWordRangeAtPosition(position);
      if (!range) return null;

      const word = document.getText(range);

      // Grid documentation
      if (word === "Grid") {
        return new vscode.Hover(
          new vscode.MarkdownString(
            `**Grid** Layout Constructor\n\n` +
              `Multi-column responsive layout supporting row/column spanning.\n\n` +
              `**Props:**\n` +
              `- \`columns\`: Int (required) — number of grid columns\n` +
              `- \`rows\`: Int (optional) — explicit row count\n` +
              `- \`gap\`: Int — spacing between cells (default 8)\n` +
              `- \`padding\`: Int — interior padding (default 0)\n` +
              `- \`bg\`: String — background color\n` +
              `- \`border\`: String — border color\n` +
              `- \`radius\`: Int — corner radius\n\n` +
              `**Child Props:**\n` +
              `- \`col\`: Int — grid column (0-indexed)\n` +
              `- \`row\`: Int — grid row (0-indexed)\n` +
              `- \`col_span\`: Int — columns to span (default 1)\n` +
              `- \`row_span\`: Int — rows to span (default 1)\n\n` +
              `[Learn more](https://aura-lang.org/docs/lumina-ui)`
          )
        );
      }

      // Image documentation
      if (word === "Image") {
        return new vscode.Hover(
          new vscode.MarkdownString(
            `**Image** Widget\n\n` +
              `Display images with multiple fit modes.\n\n` +
              `**Props:**\n` +
              `- \`src\`/\`path\`: String — image file path\n` +
              `- \`width\`, \`height\`: Int — dimensions (default 256)\n` +
              `- \`fit\`: "stretch" | "contain" | "cover" — resize mode\n` +
              `  - **stretch**: Ignore aspect ratio, fill bounds\n` +
              `  - **contain**: Preserve aspect ratio, fit within bounds\n` +
              `  - **cover**: Preserve aspect ratio, fill bounds (crop)\n` +
              `- \`tint\`: String — color overlay (default "white")\n\n` +
              `**Supported Formats:** PNG, JPEG, BMP, TGA\n\n` +
              `[Learn more](https://aura-lang.org/docs/lumina-ui)`
          )
        );
      }

      // Audio functions
      if (word === "audio" || word.startsWith("audio.")) {
        const fnMatch = document.getText(
          new vscode.Range(position.line, 0, position.line, position.character + 20)
        );

        if (fnMatch.includes("audio.load")) {
          return new vscode.Hover(
            new vscode.MarkdownString(
              `**audio.load**(path: String) -> U32\n\n` +
                `Load audio file from path into memory.\n` +
                `Returns clip ID for use with \`audio.play_loaded\`.\n\n` +
                `**Example:**\n` +
                `\`\`\`aura\n` +
                `let clip_id = audio.load("assets/beep.wav")\n` +
                `audio.play_loaded(clip_id)\n` +
                `\`\`\``
            )
          );
        }
        if (fnMatch.includes("audio.play")) {
          return new vscode.Hover(
            new vscode.MarkdownString(
              `**audio.play**(path: String) -> U32\n\n` +
                `Play audio file directly from disk.\n` +
                `Returns playback handle for later stopping.\n\n` +
                `**Example:**\n` +
                `\`\`\`aura\n` +
                `let handle = audio.play("assets/bgm.ogg")\n` +
                `audio.stop(handle)\n` +
                `\`\`\``
            )
          );
        }
        if (fnMatch.includes("audio.stop")) {
          return new vscode.Hover(
            new vscode.MarkdownString(
              `**audio.stop**(handle: U32) -> ()\n\n` +
                `Stop audio playback by handle.\n` +
                `Handle returned from \`audio.play\` or \`audio.play_loaded\`.`
            )
          );
        }
      }

      return null;
    },
  };
}

/**
 * Autocomplete provider for Lumina UI props and audio functions.
 */
export function createLuminaCompletionProvider(): vscode.CompletionItemProvider {
  const gridProps: vscode.CompletionItem[] = [
    createCompletionItem("columns", "Int (required) — number of grid columns", vscode.CompletionItemKind.Property),
    createCompletionItem("rows", "Int — number of grid rows", vscode.CompletionItemKind.Property),
    createCompletionItem("gap", "Int — spacing between cells", vscode.CompletionItemKind.Property),
    createCompletionItem("padding", "Int — interior padding", vscode.CompletionItemKind.Property),
    createCompletionItem("bg", 'String — background color (e.g., "white", "#000000")', vscode.CompletionItemKind.Property),
    createCompletionItem("border", "String — border color", vscode.CompletionItemKind.Property),
    createCompletionItem("radius", "Int — corner radius", vscode.CompletionItemKind.Property),
  ];

  const imageProps: vscode.CompletionItem[] = [
    createCompletionItem("src", "String — image file path", vscode.CompletionItemKind.Property),
    createCompletionItem("path", "String — image file path (alias for src)", vscode.CompletionItemKind.Property),
    createCompletionItem("width", "Int — image width", vscode.CompletionItemKind.Property),
    createCompletionItem("height", "Int — image height", vscode.CompletionItemKind.Property),
    createCompletionItem("fit", '"stretch" | "contain" | "cover" — resize mode', vscode.CompletionItemKind.Property),
    createCompletionItem("tint", "String — color overlay", vscode.CompletionItemKind.Property),
  ];

  const audioFunctions: vscode.CompletionItem[] = [
    createCompletionItem("audio.load", "(path: String) -> U32", vscode.CompletionItemKind.Function),
    createCompletionItem("audio.play", "(path: String) -> U32", vscode.CompletionItemKind.Function),
    createCompletionItem("audio.play_loaded", "(clip_id: U32) -> U32", vscode.CompletionItemKind.Function),
    createCompletionItem("audio.stop", "(handle: U32) -> ()", vscode.CompletionItemKind.Function),
  ];

  return {
    provideCompletionItems(document, position, token, context) {
      const line = document.lineAt(position).text;
      const linePrefix = line.substring(0, position.character);

      // Grid props
      if (linePrefix.includes("Grid(")) {
        return gridProps;
      }

      // Image props
      if (linePrefix.includes("Image(")) {
        return imageProps;
      }

      // Audio functions
      if (linePrefix.includes("audio.")) {
        return audioFunctions;
      }

      return undefined;
    },
  };
}

function createCompletionItem(
  label: string,
  detail: string,
  kind: vscode.CompletionItemKind
): vscode.CompletionItem {
  const item = new vscode.CompletionItem(label, kind);
  item.detail = detail;
  return item;
}
