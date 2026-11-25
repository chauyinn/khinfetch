import { describe, expect, it } from "vitest";
import { parseInlineTokens, parseReleaseNotes } from "./ReleaseNotesMarkdown";

describe("parseInlineTokens", () => {
  it("parses strong, em, code, markdown links, and bare URLs", () => {
    const tokens = parseInlineTokens(
      "Use **Shift** or *Alt* with `ArrowLeft`, see [docs](https://example.com/docs) or https://github.com/chauyinn/khinfetch.",
    );

    expect(tokens).toEqual([
      { type: "text", text: "Use " },
      { type: "strong", text: "Shift" },
      { type: "text", text: " or " },
      { type: "em", text: "Alt" },
      { type: "text", text: " with " },
      { type: "code", text: "ArrowLeft" },
      { type: "text", text: ", see " },
      { type: "link", label: "docs", href: "https://example.com/docs" },
      { type: "text", text: " or " },
      {
        type: "link",
        label: "https://github.com/chauyinn/khinfetch",
        href: "https://github.com/chauyinn/khinfetch",
      },
      { type: "text", text: "." },
    ]);
  });

  it("does not turn non-http(s) links into clickable link tokens", () => {
    const tokens = parseInlineTokens("Do not click [bad](javascript:alert(1)).");
    expect(tokens).toEqual([{ type: "text", text: "Do not click [bad](javascript:alert(1))." }]);
  });
});

describe("parseReleaseNotes", () => {
  it("parses headings, lists, GitHub alerts, and changelog links", () => {
    const markdown = [
      "## Highlights",
      "",
      "- **Split Pane**: Use `ArrowLeft` to resize.",
      "- **Window**: Wider default layout.",
      "",
      "> [!WARNING]",
      "> **Unsigned Application Builds**",
      "> Packages are not code-signed.",
      ">",
      "> Updater artifacts remain signed.",
      "",
      "**Full Changelog**: https://github.com/chauyinn/khinfetch/compare/v0.1.3...v0.1.4",
    ].join("\r\n");

    const blocks = parseReleaseNotes(markdown);

    expect(blocks).toEqual([
      {
        type: "heading",
        level: 2,
        tokens: [{ type: "text", text: "Highlights" }],
      },
      {
        type: "list",
        ordered: false,
        items: [
          [
            { type: "strong", text: "Split Pane" },
            { type: "text", text: ": Use " },
            { type: "code", text: "ArrowLeft" },
            { type: "text", text: " to resize." },
          ],
          [
            { type: "strong", text: "Window" },
            { type: "text", text: ": Wider default layout." },
          ],
        ],
      },
      {
        type: "blockquote",
        alert: "warning",
        paragraphs: [
          [
            { type: "strong", text: "Unsigned Application Builds" },
            { type: "text", text: " Packages are not code-signed." },
          ],
          [{ type: "text", text: "Updater artifacts remain signed." }],
        ],
      },
      {
        type: "paragraph",
        tokens: [
          { type: "strong", text: "Full Changelog" },
          { type: "text", text: ": " },
          {
            type: "link",
            label: "https://github.com/chauyinn/khinfetch/compare/v0.1.3...v0.1.4",
            href: "https://github.com/chauyinn/khinfetch/compare/v0.1.3...v0.1.4",
          },
        ],
      },
    ]);
  });

  it("parses ordered lists and horizontal rules", () => {
    const blocks = parseReleaseNotes("1. First\n2. Second\n\n---\n\nDone.");
    expect(blocks).toEqual([
      {
        type: "list",
        ordered: true,
        items: [[{ type: "text", text: "First" }], [{ type: "text", text: "Second" }]],
      },
      { type: "hr" },
      { type: "paragraph", tokens: [{ type: "text", text: "Done." }] },
    ]);
  });
});
