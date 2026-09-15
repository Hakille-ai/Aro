import { describe, expect, it } from "vitest";

import { escapeHtml, highlightCode, renderMarkdown, safeMarkdownUrl } from "./markdown";

describe("Markdown safety helpers", () => {
  it("escapes every HTML-significant character used by the renderer", () => {
    expect(escapeHtml(`<button title="x" data-value='y'>&`)).toBe(
      "&lt;button title=&quot;x&quot; data-value=&#39;y&#39;&gt;&amp;",
    );
  });

  it("allows only absolute HTTP(S) URLs", () => {
    expect(safeMarkdownUrl(" https://example.com/docs?q=aro ")).toBe("https://example.com/docs?q=aro");
    expect(safeMarkdownUrl("http://localhost:1420/path")).toBe("http://localhost:1420/path");
    expect(safeMarkdownUrl("javascript:alert(1)")).toBeNull();
    expect(safeMarkdownUrl("data:text/html,hello")).toBeNull();
    expect(safeMarkdownUrl("file:///etc/passwd")).toBeNull();
    expect(safeMarkdownUrl("/relative/path")).toBeNull();
  });

  it("renders raw HTML and privileged links as inert text", () => {
    const html = renderMarkdown(
      `<script>alert("x")</script>\n\n[unsafe](javascript:alert%281%29)\n\n![avatar](data:image/png;base64,AAAA)\n\n[safe](https://example.com/docs)`,
      "en",
    );

    expect(html).toContain("&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;");
    expect(html).toContain("<p>unsafe</p>");
    expect(html).toContain("<p>avatar</p>");
    expect(html).toContain('<a href="https://example.com/docs">safe</a>');
    expect(html).not.toContain("<script>");
    expect(html).not.toContain('href="javascript:');
    expect(html).not.toContain('src="data:');
  });
});

describe("Markdown syntax highlighting", () => {
  it("escapes unrecognized code without adding markup", () => {
    expect(highlightCode(`<tag attr="x">&`, "text")).toBe("&lt;tag attr=\"x\"&gt;&amp;");
  });

  it("preserves comments and strings while highlighting executable tokens", () => {
    expect(highlightCode(`const answer = call(42); // return false`, "ts")).toBe(
      '<span class="hl-keyword">const</span> answer = <span class="hl-function">call</span>(<span class="hl-number">42</span>); <span class="hl-comment">// return false</span>',
    );
    expect(highlightCode(`def greet(name):\n  return "hello" # comment`, "python")).toBe(
      '<span class="hl-keyword">def</span> <span class="hl-function">greet</span>(name):\n  <span class="hl-keyword">return</span> <span class="hl-string">"hello"</span> <span class="hl-comment"># comment</span>',
    );
  });

  it("retains the existing markup classes for HTML and CSS", () => {
    expect(highlightCode(`<div class="card">ok</div>`, "html")).toContain('class="hl-keyword"');
    expect(highlightCode(`<div class="card">ok</div>`, "html")).toContain('class="hl-attr"');
    expect(highlightCode(`.card { color: "red"; }`, "css")).toContain('class="hl-string"');
    expect(highlightCode(`.card { color: "red"; }`, "css")).toContain('class="hl-attr"');
  });
});

describe("Markdown code block rendering", () => {
  it("keeps the code container contract and localized copy label", () => {
    const html = renderMarkdown("```ts\nconst value = 1;\n```", "en");

    expect(html).toContain('<div class="code-block-container" data-code="const%20value%20%3D%201%3B">');
    expect(html).toContain('<span class="code-block-lang">ts</span>');
    expect(html).toContain('onclick="window.__copyCodeBlock(this)"');
    expect(html).toContain("Copy");
    expect(html).not.toContain("code-block-preview-btn");
    expect(html).toContain('<pre><code class="language-ts"><span class="hl-keyword">const</span> value = <span class="hl-number">1</span>;</code></pre>');
  });

  it("keeps the HTML preview button and French labels", () => {
    const html = renderMarkdown("```html\n<h1>ARO</h1>\n```", "fr");

    expect(html).toContain('onclick="window.__previewCodeBlock(this)"');
    expect(html).toContain("Aperçu");
    expect(html).toContain("Copier");
    expect(html).toContain('<code class="language-html">');
  });

  it("uses the unchanged generic language when no fence language is supplied", () => {
    const html = renderMarkdown("```\n<unsafe>&\n```", "en");

    expect(html).toContain('<span class="code-block-lang">code</span>');
    expect(html).toContain('<code class="language-code">&lt;unsafe&gt;&amp;</code>');
  });
});
