import { readZipEntries, readTextFileFromZip } from "./zip-reader";

export interface SpreadsheetSheet {
  name: string;
  headers: string[];
  rows: (string | number | boolean | null)[][];
}

export interface SpreadsheetData {
  sheets: SpreadsheetSheet[];
  activeSheetIndex: number;
}

/**
 * Converts column letters (A, B, Z, AA, AB) to 0-based column index.
 */
function colLetterToIndex(letters: string): number {
  let index = 0;
  for (let i = 0; i < letters.length; i++) {
    index = index * 26 + (letters.charCodeAt(i) - 64);
  }
  return index - 1;
}

/**
 * Parses XML cell reference like "C4" into { col: 2, row: 3 } (0-indexed).
 */
function parseCellRef(ref: string): { col: number; row: number } {
  const match = ref.match(/^([A-Z]+)([0-9]+)$/i);
  if (!match) return { col: 0, row: 0 };
  const col = colLetterToIndex(match[1].toUpperCase());
  const row = parseInt(match[2], 10) - 1;
  return { col, row };
}

/**
 * Parses shared strings table from xl/sharedStrings.xml
 */
function parseSharedStrings(xml: string): string[] {
  const strings: string[] = [];
  // Use regex to parse XML cleanly without external DOMParser dependency
  const siRegex = /<si\b[^>]*>([\s\S]*?)<\/si>/gi;
  let match;
  while ((match = siRegex.exec(xml)) !== null) {
    const siContent = match[1];
    // Collect all <t>...</t> inside this <si>
    const tRegex = /<t\b[^>]*>([\s\S]*?)<\/t>/gi;
    let tMatch;
    let combined = "";
    while ((tMatch = tRegex.exec(siContent)) !== null) {
      combined += decodeXmlEntities(tMatch[1]);
    }
    strings.push(combined);
  }
  return strings;
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
 * Parses sheet names from xl/workbook.xml
 */
function parseWorkbookSheets(xml: string): { name: string; sheetId: string; rId: string }[] {
  const sheets: { name: string; sheetId: string; rId: string }[] = [];
  const sheetRegex = /<sheet\b([^>]*)\/?>/gi;
  let match;
  while ((match = sheetRegex.exec(xml)) !== null) {
    const attrs = match[1];
    const nameMatch = attrs.match(/name="([^"]*)"/i);
    const idMatch = attrs.match(/sheetId="([^"]*)"/i);
    const rIdMatch = attrs.match(/r:id="([^"]*)"/i);
    sheets.push({
      name: nameMatch ? decodeXmlEntities(nameMatch[1]) : `Sheet ${sheets.length + 1}`,
      sheetId: idMatch ? idMatch[1] : String(sheets.length + 1),
      rId: rIdMatch ? rIdMatch[1] : `rId${sheets.length + 1}`,
    });
  }
  return sheets;
}

/**
 * Parses worksheet XML (e.g. xl/worksheets/sheet1.xml)
 */
function parseWorksheet(xml: string, sharedStrings: string[], sheetName: string): SpreadsheetSheet {
  const rawRows: Map<number, Map<number, string | number | boolean | null>> = new Map();
  let maxCol = 0;

  const rowRegex = /<row\b([^>]*)>([\s\S]*?)<\/row>|<row\b([^>]*)\/>/gi;
  let rowMatch;
  let autoRowIndex = 0;

  while ((rowMatch = rowRegex.exec(xml)) !== null) {
    const rowAttrs = rowMatch[1] || rowMatch[3] || "";
    const rowXml = rowMatch[2] || "";

    const rAttr = rowAttrs.match(/\br="(\d+)"/i);
    const rowIndex = rAttr ? parseInt(rAttr[1], 10) - 1 : autoRowIndex;
    autoRowIndex = rowIndex + 1;

    const cellMap = new Map<number, string | number | boolean | null>();
    const cellRegex = /<c\b([^>]*)>([\s\S]*?)<\/c>|<c\b([^>]*)\/>/gi;
    let cellMatch;
    let autoColIndex = 0;

    while ((cellMatch = cellRegex.exec(rowXml)) !== null) {
      const attrs = cellMatch[1] || cellMatch[3] || "";
      const inner = cellMatch[2] || "";

      const rMatch = attrs.match(/\br="([^"]*)"/i);
      let col: number;
      if (rMatch) {
        col = parseCellRef(rMatch[1]).col;
        autoColIndex = col + 1;
      } else {
        col = autoColIndex;
        autoColIndex++;
      }
      if (col > maxCol) maxCol = col;

      const typeMatch = attrs.match(/\bt="([^"]*)"/i);
      const cellType = typeMatch ? typeMatch[1] : "";

      let val: string | number | boolean | null = null;
      const vMatch = inner.match(/<v\b[^>]*>([\s\S]*?)<\/v>/i);
      const rawVal = vMatch ? decodeXmlEntities(vMatch[1].trim()) : "";

      if (cellType === "s") {
        // Shared string index
        const idx = parseInt(rawVal, 10);
        val = !isNaN(idx) && idx >= 0 && idx < sharedStrings.length ? sharedStrings[idx] : rawVal;
      } else if (cellType === "b") {
        val = rawVal === "1" || rawVal.toLowerCase() === "true";
      } else if (cellType === "inlineStr") {
        const tMatches = inner.match(/<t\b[^>]*>([\s\S]*?)<\/t>/gi);
        if (tMatches) {
          val = tMatches
            .map((t) => decodeXmlEntities(t.replace(/<\/?t\b[^>]*>/gi, "")))
            .join("");
        } else {
          val = "";
        }
      } else if (cellType === "str") {
        val = rawVal;
      } else if (rawVal !== "") {
        const num = Number(rawVal);
        val = !isNaN(num) ? num : rawVal;
      }

      cellMap.set(col, val);
    }
    rawRows.set(rowIndex, cellMap);
  }

  // Convert map to array of rows
  const sortedRowIndices = Array.from(rawRows.keys()).sort((a, b) => a - b);
  const rows: (string | number | boolean | null)[][] = [];

  for (const rIdx of sortedRowIndices) {
    const cellMap = rawRows.get(rIdx);
    const rowArr: (string | number | boolean | null)[] = [];
    for (let c = 0; c <= maxCol; c++) {
      rowArr.push(cellMap?.has(c) ? (cellMap.get(c) ?? null) : null);
    }
    rows.push(rowArr);
  }

  let headers: string[] = [];
  let dataRows: (string | number | boolean | null)[][] = rows;

  if (rows.length > 0) {
    headers = rows[0].map((cell, idx) => (cell !== null && cell !== undefined ? String(cell) : `Col ${idx + 1}`));
    dataRows = rows.slice(1);
  }

  return {
    name: sheetName,
    headers,
    rows: dataRows,
  };
}

/**
 * Parses HTML table text into SpreadsheetData.
 */
export function parseHtmlTableText(html: string, sheetName = "Données"): SpreadsheetData {
  const rows: (string | number | boolean | null)[][] = [];
  const trRegex = /<tr\b[^>]*>([\s\S]*?)<\/tr>/gi;
  let trMatch;

  while ((trMatch = trRegex.exec(html)) !== null) {
    const trContent = trMatch[1];
    const cells: (string | number | boolean | null)[] = [];
    const cellRegex = /<(?:td|th)\b[^>]*>([\s\S]*?)<\/(?:td|th)>/gi;
    let cellMatch;

    while ((cellMatch = cellRegex.exec(trContent)) !== null) {
      const text = decodeXmlEntities(cellMatch[1].replace(/<[^>]*>/g, "").trim());
      if (text === "") {
        cells.push(null);
      } else if (text.toLowerCase() === "true") {
        cells.push(true);
      } else if (text.toLowerCase() === "false") {
        cells.push(false);
      } else {
        const num = Number(text);
        cells.push(!isNaN(num) && text !== "" ? num : text);
      }
    }
    if (cells.length > 0) {
      rows.push(cells);
    }
  }

  let headers: string[] = [];
  let dataRows = rows;
  if (rows.length > 0) {
    headers = rows[0].map((c, i) => (c !== null ? String(c) : `Col ${i + 1}`));
    dataRows = rows.slice(1);
  }

  return {
    sheets: [{ name: sheetName, headers, rows: dataRows }],
    activeSheetIndex: 0,
  };
}

/**
 * Parses an Excel (.xlsx) ArrayBuffer or Uint8Array into structured SpreadsheetData.
 */
export async function parseExcelArrayBuffer(input: ArrayBuffer | Uint8Array): Promise<SpreadsheetData> {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);

  // Check for legacy BIFF8 OLE2 header: 0xD0 0xCF 0x11 0xE0
  if (bytes.length >= 4 && bytes[0] === 0xd0 && bytes[1] === 0xcf && bytes[2] === 0x11 && bytes[3] === 0xe0) {
    throw new Error(
      "Format binaire hérité (.xls BIFF8 / Excel 97-2003). Veuillez enregistrer le document au format moderne .xlsx ou .csv pour l'ouvrir dans le visualiseur."
    );
  }

  // Check for HTML table disguised as .xls
  if (bytes.length > 0 && bytes[0] !== 0x50 && bytes[1] !== 0x4b) {
    const textPreview = new TextDecoder("utf-8").decode(bytes.subarray(0, 1024)).trim().toLowerCase();
    if (textPreview.includes("<html") || textPreview.includes("<table") || textPreview.includes("<tr")) {
      const fullHtml = new TextDecoder("utf-8").decode(bytes);
      return parseHtmlTableText(fullHtml);
    }
    // Check if plain CSV/TSV
    if (textPreview.includes(",") || textPreview.includes(";") || textPreview.includes("\t")) {
      const fullText = new TextDecoder("utf-8").decode(bytes);
      return parseCsvText(fullText);
    }
  }

  const entries = await readZipEntries(input);
  if (entries.size === 0) {
    throw new Error("Impossible de lire l'archive Excel : format non reconnu.");
  }

  const sharedStringsXml = readTextFileFromZip(entries, "xl/sharedStrings.xml") || "";
  const sharedStrings = sharedStringsXml ? parseSharedStrings(sharedStringsXml) : [];

  const workbookXml = readTextFileFromZip(entries, "xl/workbook.xml");
  const parsedSheets = workbookXml ? parseWorkbookSheets(workbookXml) : [];

  // Parse relationships from xl/_rels/workbook.xml.rels to resolve exact worksheet files
  const relsXml = readTextFileFromZip(entries, "xl/_rels/workbook.xml.rels");
  const relMap = new Map<string, string>();
  if (relsXml) {
    const relRegex = /<Relationship\b([^>]*)\/?>/gi;
    let rMatch;
    while ((rMatch = relRegex.exec(relsXml)) !== null) {
      const idMatch = rMatch[1].match(/\bId="([^"]*)"/i);
      const targetMatch = rMatch[1].match(/\bTarget="([^"]*)"/i);
      if (idMatch && targetMatch) {
        relMap.set(idMatch[1], targetMatch[1]);
      }
    }
  }

  const sheets: SpreadsheetSheet[] = [];

  if (parsedSheets.length > 0) {
    for (let i = 0; i < parsedSheets.length; i++) {
      const sheetInfo = parsedSheets[i];
      let targetPath = relMap.get(sheetInfo.rId);
      if (targetPath) {
        if (!targetPath.startsWith("xl/")) {
          targetPath = targetPath.startsWith("/") ? targetPath.slice(1) : `xl/${targetPath}`;
        }
      }
      let sheetXml = targetPath ? readTextFileFromZip(entries, targetPath) : null;
      if (!sheetXml) {
        sheetXml = readTextFileFromZip(entries, `xl/worksheets/sheet${i + 1}.xml`);
      }
      if (!sheetXml) {
        // Fallback search remaining worksheets
        const usedPaths = new Set(sheets.map((_, idx) => `xl/worksheets/sheet${idx + 1}.xml`));
        for (const [name] of entries.entries()) {
          if (name.startsWith("xl/worksheets/") && name.endsWith(".xml") && !usedPaths.has(name)) {
            sheetXml = readTextFileFromZip(entries, name);
            break;
          }
        }
      }
      if (sheetXml) {
        sheets.push(parseWorksheet(sheetXml, sharedStrings, sheetInfo.name));
      }
    }
  } else {
    let idx = 1;
    for (const [name] of entries.entries()) {
      if (name.startsWith("xl/worksheets/") && name.endsWith(".xml")) {
        const sheetXml = readTextFileFromZip(entries, name);
        if (sheetXml) {
          sheets.push(parseWorksheet(sheetXml, sharedStrings, `Feuille ${idx}`));
          idx++;
        }
      }
    }
  }

  if (sheets.length === 0) {
    sheets.push({
      name: "Feuille 1",
      headers: ["Données"],
      rows: [],
    });
  }

  return {
    sheets,
    activeSheetIndex: 0,
  };
}

/**
 * Parses CSV or TSV text into SpreadsheetData.
 * Autodetects delimiter (comma, semicolon, tab).
 */
export function parseCsvText(text: string, sheetName = "Données"): SpreadsheetData {
  const lines = text.split(/\r?\n/).filter((line) => line.trim().length > 0);
  if (lines.length === 0) {
    return {
      sheets: [{ name: sheetName, headers: [], rows: [] }],
      activeSheetIndex: 0,
    };
  }

  // Detect delimiter using first line
  const firstLine = lines[0];
  const commaCount = (firstLine.match(/,/g) || []).length;
  const semiCount = (firstLine.match(/;/g) || []).length;
  const tabCount = (firstLine.match(/\t/g) || []).length;

  let delimiter = ",";
  if (semiCount > commaCount && semiCount > tabCount) {
    delimiter = ";";
  } else if (tabCount > commaCount && tabCount > semiCount) {
    delimiter = "\t";
  }

  const parseLine = (line: string): string[] => {
    const cells: string[] = [];
    let current = "";
    let inQuotes = false;

    for (let i = 0; i < line.length; i++) {
      const char = line[i];
      if (char === '"') {
        if (inQuotes && line[i + 1] === '"') {
          current += '"';
          i++;
        } else {
          inQuotes = !inQuotes;
        }
      } else if (char === delimiter && !inQuotes) {
        cells.push(current.trim());
        current = "";
      } else {
        current += char;
      }
    }
    cells.push(current.trim());
    return cells;
  };

  const allRows: (string | number | boolean | null)[][] = lines.map((line) => {
    const rawCells = parseLine(line);
    return rawCells.map((cell) => {
      if (cell === "") return null;
      if (cell.toLowerCase() === "true") return true;
      if (cell.toLowerCase() === "false") return false;
      const num = Number(cell);
      return !isNaN(num) && cell.trim() !== "" ? num : cell;
    });
  });

  const headers = allRows[0].map((c, i) => (c !== null ? String(c) : `Col ${i + 1}`));
  const rows = allRows.slice(1);

  return {
    sheets: [
      {
        name: sheetName,
        headers,
        rows,
      },
    ],
    activeSheetIndex: 0,
  };
}
