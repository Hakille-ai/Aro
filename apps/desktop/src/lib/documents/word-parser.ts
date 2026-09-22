import { readZipEntries, readTextFileFromZip } from "./zip-reader";

export interface WordTextRun {
  text: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
}

export interface WordTableData {
  headers: string[];
  rows: string[][];
}

export interface WordSection {
  type: "title" | "subtitle" | "heading" | "paragraph" | "list-item" | "table";
  level?: number;
  text?: string;
  runs?: WordTextRun[];
  tableData?: WordTableData;
}

export interface WordDocumentData {
  title: string;
  subtitle?: string;
  sections: WordSection[];
}

function decodeXmlEntities(str: string): string {
  return str
    .replace(/&#x([0-9a-fA-F]+);/g, (_, hex) => String.fromCharCode(parseInt(hex, 16)))
    .replace(/&#(\d+);/g, (_, dec) => String.fromCharCode(parseInt(dec, 10)))
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'");
}

/**
 * Parses Word (.docx) ArrayBuffer or Uint8Array into structured WordDocumentData.
 */
export async function parseWordDocxArrayBuffer(input: ArrayBuffer | Uint8Array): Promise<WordDocumentData> {
  const entries = await readZipEntries(input);
  if (entries.size === 0) {
    throw new Error("Impossible de lire le document Word : archive ZIP invalide.");
  }

  const docXml = readTextFileFromZip(entries, "word/document.xml");
  if (!docXml) {
    throw new Error("Document Word corrompu : 'word/document.xml' introuvable.");
  }

  return parseWordDocumentXml(docXml);
}

/**
 * Parses word/document.xml text into WordDocumentData.
 */
export function parseWordDocumentXml(xml: string): WordDocumentData {
  const sections: WordSection[] = [];
  let detectedTitle = "";
  let detectedSubtitle = "";

  // Extract body content
  const bodyMatch = xml.match(/<w:body\b[^>]*>([\s\S]*?)<\/w:body>/i);
  const bodyContent = bodyMatch ? bodyMatch[1] : xml;

  // Split into paragraphs (<w:p>) and tables (<w:tbl>)
  const blockRegex = /(<w:p\b[^>]*>[\s\S]*?<\/w:p>|<w:tbl\b[^>]*>[\s\S]*?<\/w:tbl>)/gi;
  let blockMatch;

  while ((blockMatch = blockRegex.exec(bodyContent)) !== null) {
    const block = blockMatch[1];

    if (block.startsWith("<w:tbl")) {
      // Parse Table
      const tableData = parseDocxTable(block);
      if (tableData.headers.length > 0 || tableData.rows.length > 0) {
        sections.push({
          type: "table",
          tableData,
        });
      }
    } else if (block.startsWith("<w:p")) {
      // Parse Paragraph
      const pStyleMatch = block.match(/<w:pStyle\b[^>]*\bw:val="([^"]*)"/i);
      const styleVal = pStyleMatch ? pStyleMatch[1].toLowerCase() : "";

      const isList = /<w:numPr\b/i.test(block);

      // Parse text runs
      const runs: WordTextRun[] = [];
      const runRegex = /<w:r\b[^>]*>([\s\S]*?)<\/w:r>/gi;
      let runMatch;
      let combinedText = "";

      while ((runMatch = runRegex.exec(block)) !== null) {
        const runXml = runMatch[1];
        const isBold = /<w:b(\b|\/|>)/i.test(runXml);
        const isItalic = /<w:i(\b|\/|>)/i.test(runXml);
        const isUnderline = /<w:u\b/i.test(runXml);

        const tRegex = /<w:t\b[^>]*>([\s\S]*?)<\/w:t>/gi;
        let tMatch;
        let runText = "";
        while ((tMatch = tRegex.exec(runXml)) !== null) {
          runText += decodeXmlEntities(tMatch[1]);
        }

        if (runText) {
          runs.push({
            text: runText,
            bold: isBold,
            italic: isItalic,
            underline: isUnderline,
          });
          combinedText += runText;
        }
      }

      if (!combinedText.trim()) continue;

      if (styleVal.includes("title") || (!detectedTitle && (styleVal === "heading1" || styleVal === "1"))) {
        if (!detectedTitle) {
          detectedTitle = combinedText;
          sections.push({ type: "title", text: combinedText, runs });
        } else {
          sections.push({ type: "heading", level: 1, text: combinedText, runs });
        }
      } else if (styleVal.includes("subtitle")) {
        if (!detectedSubtitle) detectedSubtitle = combinedText;
        sections.push({ type: "subtitle", text: combinedText, runs });
      } else if (styleVal.includes("heading") || styleVal.match(/^[1-6]$/)) {
        const numMatch = styleVal.match(/\d+/);
        const level = numMatch ? Math.min(Math.max(parseInt(numMatch[0], 10), 1), 6) : 2;
        sections.push({ type: "heading", level, text: combinedText, runs });
      } else if (isList) {
        sections.push({ type: "list-item", text: combinedText, runs });
      } else {
        sections.push({ type: "paragraph", text: combinedText, runs });
      }
    }
  }

  return {
    title: detectedTitle || "Document Word",
    subtitle: detectedSubtitle || undefined,
    sections,
  };
}

function parseDocxTable(tblXml: string): WordTableData {
  const rows: string[][] = [];
  const trRegex = /<w:tr\b[^>]*>([\s\S]*?)<\/w:tr>/gi;
  let trMatch;

  while ((trMatch = trRegex.exec(tblXml)) !== null) {
    const trContent = trMatch[1];
    const rowCells: string[] = [];
    const tcRegex = /<w:tc\b[^>]*>([\s\S]*?)<\/w:tc>/gi;
    let tcMatch;

    while ((tcMatch = tcRegex.exec(trContent)) !== null) {
      const tcContent = tcMatch[1];
      const pMatches = tcContent.match(/<w:p\b[^>]*>[\s\S]*?<\/w:p>/gi);
      let cellText = "";
      if (pMatches && pMatches.length > 1) {
        cellText = pMatches
          .map((p) => {
            const tMatches = p.match(/<w:t\b[^>]*>([\s\S]*?)<\/w:t>/gi) || [];
            return tMatches
              .map((t) => decodeXmlEntities(t.replace(/<\/?w:t\b[^>]*>/gi, "")))
              .join("")
              .trim();
          })
          .filter(Boolean)
          .join("\n");
      } else {
        const tRegex = /<w:t\b[^>]*>([\s\S]*?)<\/w:t>/gi;
        let tMatch;
        while ((tMatch = tRegex.exec(tcContent)) !== null) {
          cellText += decodeXmlEntities(tMatch[1]);
        }
      }
      rowCells.push(cellText.trim());
    }

    if (rowCells.length > 0) {
      rows.push(rowCells);
    }
  }

  let headers: string[] = [];
  let dataRows: string[][] = rows;

  if (rows.length > 0) {
    headers = rows[0];
    dataRows = rows.slice(1);
  }

  return {
    headers,
    rows: dataRows,
  };
}
