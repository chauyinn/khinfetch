import React, { useMemo } from "react";
import styles from "./ReleaseNotesMarkdown.module.css";

export type AlertKind = "note" | "tip" | "important" | "warning" | "caution";

export type InlineToken =
  | { type: "text"; text: string }
  | { type: "strong"; text: string }
  | { type: "em"; text: string }
  | { type: "code"; text: string }
  | { type: "link"; label: string; href: string };

export type MarkdownBlock =
  | { type: "heading"; level: 1 | 2 | 3 | 4 | 5 | 6; tokens: InlineToken[] }
  | { type: "paragraph"; tokens: InlineToken[] }
  | { type: "list"; ordered: boolean; items: InlineToken[][] }
  | { type: "blockquote"; alert?: AlertKind; paragraphs: InlineToken[][] }
  | { type: "hr" };

const HEADING_RE = /^(#{1,6})\s+(.+?)\s*#*\s*$/;
const UNORDERED_ITEM_RE = /^\s*[-*+]\s+(.+)$/;
const ORDERED_ITEM_RE = /^\s*\d+\.\s+(.+)$/;
const BLOCKQUOTE_RE = /^>\s?(.*)$/;
const HR_RE = /^(?:-{3,}|\*{3,}|_{3,})$/;
const ALERT_RE = /^\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\s*$/i;
const INLINE_RE =
  /`([^`]+)`|\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)|\*\*([^*]+)\*\*|__([^_]+)__|\*([^*]+)\*|(https?:\/\/[^\s<]+)/g;

function trimTrailingPunctuation(url: string): { cleanUrl: string; trailing: string } {
  let cleanUrl = url;
  let trailing = "";

  while (cleanUrl.length > 0) {
    const last = cleanUrl[cleanUrl.length - 1];
    if (last === "." || last === "," || last === ";" || last === ":" || last === "!" || last === "?") {
      trailing = last + trailing;
      cleanUrl = cleanUrl.slice(0, -1);
      continue;
    }
    if (last === ")") {
      const openCount = (cleanUrl.match(/\(/g) ?? []).length;
      const closeCount = (cleanUrl.match(/\)/g) ?? []).length;
      if (closeCount > openCount) {
        trailing = last + trailing;
        cleanUrl = cleanUrl.slice(0, -1);
        continue;
      }
    }
    break;
  }

  return { cleanUrl, trailing };
}

export function parseInlineTokens(text: string): InlineToken[] {
  const tokens: InlineToken[] = [];
  let cursor = 0;

  for (const match of text.matchAll(INLINE_RE)) {
    const start = match.index ?? 0;
    if (start > cursor) {
      tokens.push({ type: "text", text: text.slice(cursor, start) });
    }

    const [full, code, linkLabel, linkHref, boldStar, boldUnder, italic, bareUrl] = match;
    if (code !== undefined) {
      tokens.push({ type: "code", text: code });
    } else if (linkLabel !== undefined && linkHref !== undefined) {
      tokens.push({ type: "link", label: linkLabel, href: linkHref });
    } else if (boldStar !== undefined || boldUnder !== undefined) {
      tokens.push({ type: "strong", text: boldStar ?? boldUnder });
    } else if (italic !== undefined) {
      tokens.push({ type: "em", text: italic });
    } else if (bareUrl !== undefined) {
      const { cleanUrl, trailing } = trimTrailingPunctuation(bareUrl);
      if (cleanUrl) {
        tokens.push({ type: "link", label: cleanUrl, href: cleanUrl });
      }
      if (trailing) {
        tokens.push({ type: "text", text: trailing });
      }
    } else {
      tokens.push({ type: "text", text: full });
    }

    cursor = start + full.length;
  }

  if (cursor < text.length) {
    tokens.push({ type: "text", text: text.slice(cursor) });
  }

  return tokens;
}

function isBlockStarter(line: string): boolean {
  const trimmed = line.trim();
  return (
    trimmed.length === 0 ||
    HEADING_RE.test(trimmed) ||
    HR_RE.test(trimmed) ||
    BLOCKQUOTE_RE.test(trimmed) ||
    UNORDERED_ITEM_RE.test(line) ||
    ORDERED_ITEM_RE.test(line)
  );
}

export function parseReleaseNotes(markdown: string): MarkdownBlock[] {
  const lines = markdown.replace(/\r\n/g, "\n").split("\n");
  const blocks: MarkdownBlock[] = [];
  let index = 0;

  while (index < lines.length) {
    const rawLine = lines[index];
    const trimmed = rawLine.trim();

    if (trimmed.length === 0) {
      index += 1;
      continue;
    }

    const headingMatch = trimmed.match(HEADING_RE);
    if (headingMatch) {
      const level = headingMatch[1].length as 1 | 2 | 3 | 4 | 5 | 6;
      blocks.push({ type: "heading", level, tokens: parseInlineTokens(headingMatch[2]) });
      index += 1;
      continue;
    }

    if (HR_RE.test(trimmed)) {
      blocks.push({ type: "hr" });
      index += 1;
      continue;
    }

    if (BLOCKQUOTE_RE.test(trimmed)) {
      const quoteLines: string[] = [];
      while (index < lines.length) {
        const quoteMatch = lines[index].trim().match(BLOCKQUOTE_RE);
        if (!quoteMatch) break;
        quoteLines.push(quoteMatch[1]);
        index += 1;
      }

      let alert: AlertKind | undefined;
      if (quoteLines.length > 0) {
        const alertMatch = quoteLines[0].trim().match(ALERT_RE);
        if (alertMatch) {
          alert = alertMatch[1].toLowerCase() as AlertKind;
          quoteLines.shift();
        }
      }

      const paragraphs: InlineToken[][] = [];
      let buffer: string[] = [];
      const flushBuffer = () => {
        if (buffer.length > 0) {
          paragraphs.push(parseInlineTokens(buffer.join(" ")));
          buffer = [];
        }
      };

      for (const quoteLine of quoteLines) {
        const content = quoteLine.trim();
        if (content.length === 0) {
          flushBuffer();
        } else {
          buffer.push(content);
        }
      }
      flushBuffer();

      if (paragraphs.length > 0 || alert) {
        blocks.push({ type: "blockquote", alert, paragraphs });
      }
      continue;
    }

    const unorderedMatch = rawLine.match(UNORDERED_ITEM_RE);
    if (unorderedMatch) {
      const items: InlineToken[][] = [];
      while (index < lines.length) {
        const itemMatch = lines[index].match(UNORDERED_ITEM_RE);
        if (!itemMatch) break;
        items.push(parseInlineTokens(itemMatch[1].trim()));
        index += 1;
      }
      blocks.push({ type: "list", ordered: false, items });
      continue;
    }

    const orderedMatch = rawLine.match(ORDERED_ITEM_RE);
    if (orderedMatch) {
      const items: InlineToken[][] = [];
      while (index < lines.length) {
        const itemMatch = lines[index].match(ORDERED_ITEM_RE);
        if (!itemMatch) break;
        items.push(parseInlineTokens(itemMatch[1].trim()));
        index += 1;
      }
      blocks.push({ type: "list", ordered: true, items });
      continue;
    }

    const paragraphLines: string[] = [trimmed];
    index += 1;
    while (index < lines.length && !isBlockStarter(lines[index])) {
      paragraphLines.push(lines[index].trim());
      index += 1;
    }
    blocks.push({ type: "paragraph", tokens: parseInlineTokens(paragraphLines.join(" ")) });
  }

  return blocks;
}

const ALERT_LABELS: Record<AlertKind, string> = {
  note: "Note",
  tip: "Tip",
  important: "Important",
  warning: "Warning",
  caution: "Caution",
};

interface ReleaseNotesMarkdownProps {
  content: string;
  onOpenLink: (url: string) => void;
}

export const ReleaseNotesMarkdown: React.FC<ReleaseNotesMarkdownProps> = ({
  content,
  onOpenLink,
}) => {
  const blocks = useMemo(() => parseReleaseNotes(content), [content]);

  const renderInline = (tokens: InlineToken[], keyPrefix: string) =>
    tokens.map((token, idx) => {
      const key = `${keyPrefix}-${idx}`;
      switch (token.type) {
        case "text":
          return <React.Fragment key={key}>{token.text}</React.Fragment>;
        case "strong":
          return (
            <strong key={key} className={styles.strong}>
              {token.text}
            </strong>
          );
        case "em":
          return <em key={key}>{token.text}</em>;
        case "code":
          return (
            <code key={key} className={styles.code}>
              {token.text}
            </code>
          );
        case "link":
          return (
            <a
              key={key}
              href={token.href}
              target="_blank"
              rel="noreferrer"
              className={styles.link}
              onClick={(event) => {
                event.preventDefault();
                onOpenLink(token.href);
              }}
            >
              {token.label}
            </a>
          );
      }
    });

  return (
    <div className={styles.markdown}>
      {blocks.map((block, index) => {
        const key = `block-${index}`;
        switch (block.type) {
          case "heading":
            return block.level <= 2 ? (
              <h5 key={key} className={styles.heading}>
                {renderInline(block.tokens, key)}
              </h5>
            ) : (
              <h6 key={key} className={styles.subheading}>
                {renderInline(block.tokens, key)}
              </h6>
            );
          case "paragraph":
            return (
              <p key={key} className={styles.paragraph}>
                {renderInline(block.tokens, key)}
              </p>
            );
          case "list": {
            const ListTag = block.ordered ? "ol" : "ul";
            return (
              <ListTag key={key} className={styles.list}>
                {block.items.map((itemTokens, itemIndex) => (
                  <li key={`${key}-item-${itemIndex}`} className={styles.listItem}>
                    {renderInline(itemTokens, `${key}-item-${itemIndex}`)}
                  </li>
                ))}
              </ListTag>
            );
          }
          case "blockquote":
            return (
              <blockquote
                key={key}
                className={`${styles.blockquote} ${
                  block.alert ? styles[`alert_${block.alert}`] : ""
                }`}
              >
                {block.alert && (
                  <div className={styles.alertTitle}>{ALERT_LABELS[block.alert]}</div>
                )}
                {block.paragraphs.map((paragraphTokens, pIndex) => (
                  <p key={`${key}-p-${pIndex}`} className={styles.blockquoteParagraph}>
                    {renderInline(paragraphTokens, `${key}-p-${pIndex}`)}
                  </p>
                ))}
              </blockquote>
            );
          case "hr":
            return <hr key={key} className={styles.hr} />;
        }
      })}
    </div>
  );
};
