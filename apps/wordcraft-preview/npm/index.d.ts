export type Language = 'auto' | 'en' | 'zh-hans';

export type Theme = 'light' | 'dark' | 'auto';

export interface FindOptions {
  matchCase?: boolean;
  wholeWord?: boolean;
  regex?: boolean;
}

export class WordCraftPreview {
  constructor();
  loadFile(file: File | Blob, filename?: string): Promise<void>;
  zoomIn(): void;
  zoomOut(): void;
  zoomToFit(): void;
  setZoom(percentage: number): void;
  /** Preview mode is strictly read-only; passing `true` throws an error. */
  setEditable(editable: boolean): void;
  /** Preview mode is strictly read-only; passing `false` throws an error. */
  setReadOnly(readOnly: boolean): void;
  getPageCount(): number;
  getCurrentPage(): number;
  goToPage(page: number): void;
  getSelection(): string;
  selectRange(text: string): void;
  selectText(text: string, occurrence?: number): void;
  find(query: string, options?: FindOptions): number;
  findNext(): boolean;
  findPrevious(): boolean;
  setLanguage(language: Language): void;
  getLanguages(): ReadonlyArray<readonly [string, string]>;
  setTheme(theme: Theme): void;
  loadSystemFonts(): void;
}

export default function init(
  input?: string | URL | Request | Response | BufferSource | WebAssembly.Module | Promise<unknown>
): Promise<void>;
