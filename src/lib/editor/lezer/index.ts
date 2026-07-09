import type { MarkdownExtension } from "@lezer/markdown";
import { mathExtension } from "./math";
import { footnoteExtension } from "./footnotes";

export { mathExtension } from "./math";
export { footnoteExtension } from "./footnotes";

/** Every custom Lezer parser yap plugs into `@codemirror/lang-markdown`. */
export const yapLezerExtensions: MarkdownExtension[] = [mathExtension, footnoteExtension];
