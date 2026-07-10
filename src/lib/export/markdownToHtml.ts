/**
 * A small, dependency-free markdown to HTML converter for the Edit > Copy HTML
 * action. It is deliberately not a full CommonMark implementation: it covers
 * the constructs bulletmd renders (headings, emphasis, code, links, images, lists,
 * quotes, rules) well enough to paste into an email or a doc. Math and
 * footnotes fall through as their literal source, which is acceptable on the
 * clipboard.
 */

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// A NUL never occurs in a text buffer, so it is a safe placeholder marker.
const CODE_SENTINEL = String.fromCharCode(0);

/** Inline spans: code, images, links, emphasis, strikethrough. */
export interface MarkdownHtmlOptions {
  resolveImageSrc?: (src: string) => string;
  renderMath?: (tex: string, displayMode: boolean) => string;
  /** Let a caller turn a complete source line into an HTML block. */
  renderLine?: (line: string) => string | undefined;
}

function inline(raw: string, options: MarkdownHtmlOptions): string {
  let text = escapeHtml(raw);

  // Pull code spans out first so their contents dodge the other rules.
  const codes: string[] = [];
  text = text.replace(/`([^`]+)`/g, (_, code: string) => {
    codes.push(code);
    return `${CODE_SENTINEL}${codes.length - 1}${CODE_SENTINEL}`;
  });

  text = text
    // Images before links: both open with a bracket.
    .replace(
      /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g,
      (_, alt: string, src: string) => `<img src="${options.resolveImageSrc?.(src) ?? src}" alt="${alt}">`,
    )
    .replace(
      /\[([^\]]+)\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g,
      (_, label: string, url: string) => `<a href="${url}">${label}</a>`,
    )
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/__([^_]+)__/g, "<strong>$1</strong>")
    .replace(/(^|[^*])\*([^*]+)\*/g, "$1<em>$2</em>")
    .replace(/(^|[^_])_([^_]+)_/g, "$1<em>$2</em>")
    .replace(/~~([^~]+)~~/g, "<del>$1</del>");

  if (options.renderMath) {
    text = text.replace(/\$([^$\n]+)\$/g, (_, tex: string) => options.renderMath!(tex, false));
  }

  // Restore the code spans, now safely wrapped.
  return text.replace(
    new RegExp(`${CODE_SENTINEL}(\\d+)${CODE_SENTINEL}`, "g"),
    (_, i: string) => `<code>${codes[Number(i)]}</code>`,
  );
}

const HR = /^\s*([-*_])(?:\s*\1){2,}\s*$/;
const HEADING = /^(#{1,6})\s+(.*?)\s*#*\s*$/;
const FENCE = /^```(\w*)\s*$/;
const FENCE_END = /^```\s*$/;
const BLOCKQUOTE = /^>\s?/;
const UL_ITEM = /^\s*[-*+]\s+(.*)$/;
const OL_ITEM = /^\s*\d+\.\s+(.*)$/;

export function markdownToHtml(md: string, options: MarkdownHtmlOptions = {}): string {
  const lines = md.replace(/\r\n?/g, "\n").split("\n");
  const out: string[] = [];
  let paragraph: string[] = [];
  let i = 0;

  const flushParagraph = () => {
    if (paragraph.length) {
      out.push(`<p>${inline(paragraph.join(" "), options)}</p>`);
      paragraph = [];
    }
  };

  while (i < lines.length) {
    const line = lines[i];

    const renderedLine = options.renderLine?.(line);
    if (renderedLine !== undefined) {
      flushParagraph();
      out.push(renderedLine);
      i++;
      continue;
    }

    if (options.renderMath && line.trim() === "$$") {
      flushParagraph();
      i++;
      const body: string[] = [];
      while (i < lines.length && lines[i].trim() !== "$$") body.push(lines[i++]);
      if (i < lines.length) {
        out.push(options.renderMath(body.join("\n"), true));
        i++;
        continue;
      }
      // An unclosed delimiter remains ordinary source, as it does in the editor.
      paragraph.push(`$$ ${body.join(" ")}`);
      break;
    }

    const fence = line.match(FENCE);
    if (fence) {
      flushParagraph();
      i++;
      const body: string[] = [];
      while (i < lines.length && !FENCE_END.test(lines[i])) body.push(lines[i++]);
      i++; // skip the closing fence
      const cls = fence[1] ? ` class="language-${fence[1]}"` : "";
      out.push(`<pre><code${cls}>${escapeHtml(body.join("\n"))}</code></pre>`);
      continue;
    }

    if (/^\s*$/.test(line)) {
      flushParagraph();
      i++;
      continue;
    }

    if (HR.test(line)) {
      flushParagraph();
      out.push("<hr>");
      i++;
      continue;
    }

    const heading = line.match(HEADING);
    if (heading) {
      flushParagraph();
      const level = heading[1].length;
      out.push(`<h${level}>${inline(heading[2], options)}</h${level}>`);
      i++;
      continue;
    }

    if (BLOCKQUOTE.test(line)) {
      flushParagraph();
      const body: string[] = [];
      while (i < lines.length && BLOCKQUOTE.test(lines[i])) {
        body.push(lines[i].replace(BLOCKQUOTE, ""));
        i++;
      }
      out.push(`<blockquote>${markdownToHtml(body.join("\n"), options)}</blockquote>`);
      continue;
    }

    const ordered = OL_ITEM.test(line);
    if (ordered || UL_ITEM.test(line)) {
      flushParagraph();
      const item = ordered ? OL_ITEM : UL_ITEM;
      const items: string[] = [];
      while (i < lines.length && item.test(lines[i])) {
        items.push(lines[i].match(item)![1]);
        i++;
      }
      const tag = ordered ? "ol" : "ul";
      out.push(`<${tag}>${items.map((t) => `<li>${inline(t, options)}</li>`).join("")}</${tag}>`);
      continue;
    }

    paragraph.push(line.trim());
    i++;
  }

  flushParagraph();
  return out.join("\n");
}
