<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { EditorView } from "@codemirror/view";
  import { createEditor } from "./lib/editor/createEditor";

  let host: HTMLElement;
  let view: EditorView | undefined;

  const SAMPLE = `# yap

A *live-preview* markdown editor with **rich** rendering and \`inline code\`.

Click into any block and its raw markdown appears. ~~Struck through~~ text,
**bold**, *italic*, and [a link](https://example.com) all live here.

## Second level

### Third level

Setext heading
==============

- a list item with **bold** inside
- see [**bold** link](https://example.com) nested in a list
  - a nested item

> a blockquote
> spanning two lines

\`\`\`rust
fn main() {
    println!("hello");
}
\`\`\`

| col a | col b |
| ----- | ----- |
| **1** | 2     |

Final paragraph.
`;

  onMount(() => {
    view = createEditor({ parent: host, doc: SAMPLE });
    view.focus();
  });

  onDestroy(() => view?.destroy());
</script>

<main bind:this={host}></main>

<style>
  main {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
</style>
