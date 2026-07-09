import { describe, expect, it } from "vitest";
import { markdownToHtml } from "./markdownToHtml";

describe("markdownToHtml", () => {
  it("renders ATX headings at the right level", () => {
    expect(markdownToHtml("# Title")).toBe("<h1>Title</h1>");
    expect(markdownToHtml("### Deep")).toBe("<h3>Deep</h3>");
  });

  it("wraps prose in paragraphs and joins wrapped lines", () => {
    expect(markdownToHtml("one\ntwo\n\nthree")).toBe("<p>one two</p>\n<p>three</p>");
  });

  it("renders inline emphasis, strong, strike and code", () => {
    expect(markdownToHtml("a **b** _c_ ~~d~~ `e`")).toBe(
      "<p>a <strong>b</strong> <em>c</em> <del>d</del> <code>e</code></p>",
    );
  });

  it("does not treat markup inside inline code", () => {
    expect(markdownToHtml("`a **b**`")).toBe("<p><code>a **b**</code></p>");
  });

  it("renders links and images", () => {
    expect(markdownToHtml("[t](http://x.dev)")).toBe('<p><a href="http://x.dev">t</a></p>');
    expect(markdownToHtml("![a](p.png)")).toBe('<p><img src="p.png" alt="a"></p>');
  });

  it("uses print renderers for images and inline or display math", () => {
    const options = {
      resolveImageSrc: (src: string) => `asset:///${src}`,
      renderMath: (tex: string, display: boolean) => `<math data-display="${display}">${tex}</math>`,
    };
    expect(markdownToHtml("![a](p.png) and $x^2$", options)).toBe(
      '<p><img src="asset:///p.png" alt="a"> and <math data-display="false">x^2</math></p>',
    );
    expect(markdownToHtml("$$\nx^2\n$$", options)).toBe('<math data-display="true">x^2</math>');
  });

  it("lets an exporter render an otherwise ordinary source line", () => {
    expect(markdownToHtml("before\n<!--page-break-->\nafter", {
      renderLine: (line) => (line === "<!--page-break-->" ? "<break>" : undefined),
    })).toBe(
      "<p>before</p>\n<break>\n<p>after</p>",
    );
  });

  it("escapes HTML in prose but not the tags it emits", () => {
    expect(markdownToHtml("a < b & c")).toBe("<p>a &lt; b &amp; c</p>");
  });

  it("escapes the contents of a code span", () => {
    expect(markdownToHtml("`<script>`")).toBe("<p><code>&lt;script&gt;</code></p>");
  });

  it("renders a fenced code block with a language class", () => {
    expect(markdownToHtml("```rust\nlet x = 1 < 2;\n```")).toBe(
      '<pre><code class="language-rust">let x = 1 &lt; 2;</code></pre>',
    );
  });

  it("renders unordered and ordered lists", () => {
    expect(markdownToHtml("- a\n- b")).toBe("<ul><li>a</li><li>b</li></ul>");
    expect(markdownToHtml("1. a\n2. b")).toBe("<ol><li>a</li><li>b</li></ol>");
  });

  it("renders a horizontal rule and a blockquote", () => {
    expect(markdownToHtml("---")).toBe("<hr>");
    expect(markdownToHtml("> quoted")).toBe("<blockquote><p>quoted</p></blockquote>");
  });

  it("keeps a fenced block's markup literal", () => {
    expect(markdownToHtml("```\n# not a heading\n```")).toBe(
      "<pre><code># not a heading</code></pre>",
    );
  });
});
