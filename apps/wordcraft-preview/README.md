# WordCraft Preview

This app is the browser preview host for WordCraft. It uses the same `Session`, file readers,
layout engine and egui canvas as the desktop application, compiled as a slim strictly read-only
viewer (no top ribbon, no editing, no formulas/equations, no proofing, no export or print). DOCX,
DOCM, DOTX, DOTM, DOC, DOT, TXT and XML files are supported by default (RTF, Markdown and
HTML/MHT are optional via `--features extra-formats`), and can be selected or dropped onto the page.

The JavaScript API is packaged as [`wordcraft-preview`](npm). Preview mode is locked read-only:
calling `setEditable(true)` or `setReadOnly(false)` throws an error, and the viewer only permits
previewing, scrolling, zooming, selecting/copying, and finding. Save, export and print services are
intentionally absent. Rendering uses WebGL2 (`glow`).
The development page requests the browser's Local Font Access API after startup so CJK documents
can use installed system fonts; browsers that deny the optional permission keep using bundled and
substitute fonts.
Selecting a document on the development page moves the same canvas into a modal preview, matching
the embedding pattern used by `gridcraft-preview`. NPM hosts can use the same pattern; see
[`npm/example.html`](npm/example.html).

Run the development page with `trunk serve --release` from this directory. The page is served at
`http://127.0.0.1:8773` and the release output is written to `dist/preview` at the repository root.
Interface catalogs are picked up from a `../craft-i18n` checkout next to the repository when
`CRAFT_I18N_DIR` is not set; set that variable to another checkout when needed so **自动** follows
the browser's locale, including Chinese.
