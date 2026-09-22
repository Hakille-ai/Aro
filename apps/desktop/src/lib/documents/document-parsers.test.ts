import { describe, expect, it } from "vitest";
import { parseCsvText, parseHtmlTableText, parseExcelArrayBuffer } from "./excel-parser";
import { parseWordDocumentXml } from "./word-parser";

describe("Spreadsheet and Document Parsers", () => {
  describe("parseCsvText", () => {
    it("parses comma-delimited CSV with numbers and booleans", () => {
      const csv = `Produit,Prix,EnStock\nMacBook Pro,2499.99,true\niPhone 16,1199,false`;
      const result = parseCsvText(csv, "Catalogue");
      expect(result.sheets.length).toBe(1);
      const sheet = result.sheets[0];
      expect(sheet.name).toBe("Catalogue");
      expect(sheet.headers).toEqual(["Produit", "Prix", "EnStock"]);
      expect(sheet.rows.length).toBe(2);
      expect(sheet.rows[0]).toEqual(["MacBook Pro", 2499.99, true]);
      expect(sheet.rows[1]).toEqual(["iPhone 16", 1199, false]);
    });

    it("parses semicolon-delimited CSV (French locale)", () => {
      const csv = `Nom;Département;Salaire\nAlice;Ingénierie;75000\nBob;Design;68000`;
      const result = parseCsvText(csv);
      const sheet = result.sheets[0];
      expect(sheet.headers).toEqual(["Nom", "Département", "Salaire"]);
      expect(sheet.rows[0]).toEqual(["Alice", "Ingénierie", 75000]);
    });

    it("handles quoted cells with commas and quotes", () => {
      const csv = `"Nom, Complet",Statut\n"Martin, Jean",Actif`;
      const result = parseCsvText(csv);
      const sheet = result.sheets[0];
      expect(sheet.headers).toEqual(["Nom, Complet", "Statut"]);
      expect(sheet.rows[0]).toEqual(["Martin, Jean", "Actif"]);
    });

    it("handles empty or single-line CSV safely", () => {
      const empty = parseCsvText("");
      expect(empty.sheets[0].rows).toEqual([]);

      const single = parseCsvText("ColA,ColB");
      expect(single.sheets[0].headers).toEqual(["ColA", "ColB"]);
      expect(single.sheets[0].rows).toEqual([]);
    });
  });

  describe("parseWordDocumentXml", () => {
    it("parses headings, paragraphs, lists and tables from Word XML", () => {
      const xml = `
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
          <w:body>
            <w:p>
              <w:pPr><w:pStyle w:val="Title"/></w:pPr>
              <w:r><w:t>Rapport Annuel 2026</w:t></w:r>
            </w:p>
            <w:p>
              <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
              <w:r><w:t>1. Introduction &amp; Contexte</w:t></w:r>
            </w:p>
            <w:p>
              <w:r><w:rPr><w:b/></w:rPr><w:t>Note : </w:t></w:r>
              <w:r><w:t>Ceci est un document confidentiel.</w:t></w:r>
            </w:p>
            <w:p>
              <w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr>
              <w:r><w:t>Premier point clé</w:t></w:r>
            </w:p>
            <w:tbl>
              <w:tr>
                <w:tc><w:p><w:r><w:t>Métrique</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>Valeur</w:t></w:r></w:p></w:tc>
              </w:tr>
              <w:tr>
                <w:tc><w:p><w:r><w:t>Revenu</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>1.2M €</w:t></w:r></w:p></w:tc>
              </w:tr>
            </w:tbl>
          </w:body>
        </w:document>
      `;

      const result = parseWordDocumentXml(xml);
      expect(result.title).toBe("Rapport Annuel 2026");
      expect(result.sections.length).toBe(5);

      expect(result.sections[0].type).toBe("title");
      expect(result.sections[1].type).toBe("heading");
      expect(result.sections[1].text).toBe("1. Introduction & Contexte");

      expect(result.sections[2].type).toBe("paragraph");
      expect(result.sections[2].runs?.[0].bold).toBe(true);

      expect(result.sections[3].type).toBe("list-item");
      expect(result.sections[3].text).toBe("Premier point clé");

      expect(result.sections[4].type).toBe("table");
      expect(result.sections[4].tableData?.headers).toEqual(["Métrique", "Valeur"]);
      expect(result.sections[4].tableData?.rows).toEqual([["Revenu", "1.2M €"]]);
    });

    it("parses multi-paragraph table cells and decodes numeric entities", () => {
      const xml = `
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
          <w:body>
            <w:p><w:r><w:t>Prix : 100&#160;&#x20AC;</w:t></w:r></w:p>
            <w:tbl>
              <w:tr>
                <w:tc>
                  <w:p><w:r><w:t>Ligne 1</w:t></w:r></w:p>
                  <w:p><w:r><w:t>Ligne 2</w:t></w:r></w:p>
                </w:tc>
              </w:tr>
            </w:tbl>
          </w:body>
        </w:document>
      `;
      const result = parseWordDocumentXml(xml);
      expect(result.sections[0].text).toBe("Prix : 100\u00A0€");
      expect(result.sections[1].tableData?.headers).toEqual(["Ligne 1\nLigne 2"]);
    });
  });

  describe("parseHtmlTableText", () => {
    it("parses HTML tables into SpreadsheetData", () => {
      const html = `<table><tr><th>Produit</th><th>Quantité</th></tr><tr><td>MacBook</td><td>5</td></tr></table>`;
      const result = parseHtmlTableText(html, "HTML Export");
      expect(result.sheets[0].name).toBe("HTML Export");
      expect(result.sheets[0].headers).toEqual(["Produit", "Quantité"]);
      expect(result.sheets[0].rows).toEqual([["MacBook", 5]]);
    });
  });

  describe("parseExcelArrayBuffer legacy and format guards", () => {
    it("rejects legacy BIFF8 .xls binary files with actionable advice", async () => {
      const biff8 = new Uint8Array([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
      await expect(parseExcelArrayBuffer(biff8)).rejects.toThrow("BIFF8");
    });

    it("parses HTML tables disguised with .xls extension", async () => {
      const html = `<html><body><table><tr><th>Col1</th></tr><tr><td>Val1</td></tr></table></body></html>`;
      const bytes = new TextEncoder().encode(html);
      const res = await parseExcelArrayBuffer(bytes);
      expect(res.sheets[0].headers).toEqual(["Col1"]);
      expect(res.sheets[0].rows).toEqual([["Val1"]]);
    });
  });
});
