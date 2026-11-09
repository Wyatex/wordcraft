# Getting started

Install Trunk and the WebAssembly target once:

```text
cargo install trunk
rustup target add wasm32-unknown-unknown
```

The preview uses the same interface catalogs as `wordcraft-web`. If `../craft-i18n` exists next to
the repository it is detected automatically; otherwise set `CRAFT_I18N_DIR` to the checkout before
building so the **自动** language option can resolve the browser's Chinese locale.

Start the preview page:

```text
cd apps/wordcraft-preview
trunk serve
```

Use **打开文件** or drag a supported document over the canvas. The development page opens the
selected document in a modal and moves the same canvas back when it closes. The toolbar exposes page
navigation, zoom, search, language (`auto`, `en`, `zh-hans`) and theme. The page also requests
installed system fonts when the browser supports Local Font Access, which improves Chinese and other
CJK rendering. The preview is strictly read-only (top ribbon and editing are removed; only viewing,
copying, and finding are supported).

For the NPM package, run `npm run build` in `apps/wordcraft-preview/npm`, then use the generated
`dist/index.js` and `dist/index.d.ts` from an application bundler.
