# wordcraft-preview

`wordcraft-preview` is a browser WASM package for displaying WordCraft-supported documents. It
is strictly read-only (hiding the ribbon bar and disabling editing, formulas, save, export, and print).
It accepts `File` or `Blob` values through `loadFile`; calling `setEditable(true)` or
`setReadOnly(false)` throws an error.

```ts
import init, { WordCraftPreview } from 'wordcraft-preview';

await init();
const preview = new WordCraftPreview();
await preview.loadFile(file);
preview.setZoom(125);
console.log(preview.getPageCount());
preview.loadSystemFonts(); // optional Local Font Access API for CJK/system fonts
```

### 在弹窗中打开

WASM runner 使用页面中的 `wordcraft_preview_canvas`。像 GridCraft Preview 一样，可以在打开
文件时把这个 canvas 移到 modal 容器，关闭时再移回原位置：

```html
<canvas id="wordcraft_preview_canvas"></canvas>
<div id="preview_modal" hidden>
  <button id="close_preview">关闭</button>
  <div id="modal_canvas_host"></div>
</div>
<input id="file" type="file">
<script type="module">
  import init, { WordCraftPreview } from 'wordcraft-preview';

  await init();
  const preview = new WordCraftPreview();
  const canvas = document.querySelector('#wordcraft_preview_canvas');
  const modal = document.querySelector('#preview_modal');
  const host = document.querySelector('#modal_canvas_host');

  document.querySelector('#file').onchange = async event => {
    const file = event.target.files?.[0];
    if (!file) return;
    host.appendChild(canvas);
    modal.hidden = false;
    await preview.loadFile(file);
  };
  document.querySelector('#close_preview').onclick = () => {
    document.body.appendChild(canvas);
    modal.hidden = true;
  };
</script>
```

给 modal 内容区设置明确的 `width` 和 `height`，并让 canvas 使用 `width: 100%; height: 100%`。
完整示例见 [`example.html`](example.html)。

Build from this directory with `npm run build`; it requires Trunk and the Rust
`wasm32-unknown-unknown` target. The generated package can be checked with `npm pack --dry-run`.
