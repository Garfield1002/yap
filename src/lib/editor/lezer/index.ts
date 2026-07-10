import type { MarkdownExtension } from "@lezer/markdown";
import { mathExtension } from "./math";
import { footnoteExtension } from "./footnotes";

export { mathExtension } from "./math";
export { footnoteExtension } from "./footnotes";

/** Every custom Lezer parser bulletmd plugs into `@codemirror/lang-markdown`. */
export const bulletmdLezerExtensions: MarkdownExtension[] = [mathExtension, footnoteExtension];
