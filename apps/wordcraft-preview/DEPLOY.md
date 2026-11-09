# Deploying the preview

`trunk build --release` emits hashed JavaScript and WebAssembly files in `dist/preview`. Serve that
directory as static content with the generated files at the same relative paths; `public_url =
"./"` lets the output be hosted below a path or from a CDN origin. WebAssembly MIME types should
be `application/wasm` and the server should allow the page's module and worker requests.

The NPM package is built with `npm run build` and checked with `npm pack --dry-run`. It contains the
generated wasm-bindgen module, snippets, TypeScript definitions and the project MIT/Apache license
files. No server-side document endpoint or iframe messaging protocol is required: callers pass a
browser `File` or `Blob` to `loadFile`.

For a modal, place a canvas with id `wordcraft_preview_canvas` in the page before calling `init()`.
When a file is selected, move that canvas into the modal's content element and move it back when the
modal closes. The canvas must have explicit dimensions in both locations. See
[`npm/example.html`](npm/example.html) for a complete example.
