import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import type { MarkdownExtension } from "@lezer/markdown";

/**
 * `markdownLanguage` is CommonMark + GFM (tables, task lists, strikethrough,
 * autolinks) + sub/superscript + emoji. `codeLanguages` lazily loads grammars
 * for fenced blocks so they highlight in-place.
 *
 * `extensions` is where the custom Lezer parsers (math, footnotes) plug in.
 */
export function yapMarkdown(extensions: MarkdownExtension[] = []) {
  return markdown({
    base: markdownLanguage,
    codeLanguages: languages,
    extensions,
    addKeymap: true,
  });
}
