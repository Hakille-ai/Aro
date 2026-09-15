use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub struct ZipBuilder {
    files: Vec<(String, Vec<u8>)>,
}

impl Default for ZipBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ZipBuilder {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn add_file(&mut self, path: impl Into<String>, content: impl Into<Vec<u8>>) {
        self.files.push((path.into(), content.into()));
    }

    pub fn finish(self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central_dir = Vec::new();

        for (name, data) in &self.files {
            let offset = out.len() as u32;
            let name_bytes = name.as_bytes();
            let crc = crc32(data);
            let size = data.len() as u32;

            out.extend_from_slice(&0x04034b50u32.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0x0800u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(data);

            central_dir.extend_from_slice(&0x02014b50u32.to_le_bytes());
            central_dir.extend_from_slice(&20u16.to_le_bytes());
            central_dir.extend_from_slice(&20u16.to_le_bytes());
            central_dir.extend_from_slice(&0x0800u16.to_le_bytes());
            central_dir.extend_from_slice(&0u16.to_le_bytes());
            central_dir.extend_from_slice(&0u16.to_le_bytes());
            central_dir.extend_from_slice(&0u16.to_le_bytes());
            central_dir.extend_from_slice(&crc.to_le_bytes());
            central_dir.extend_from_slice(&size.to_le_bytes());
            central_dir.extend_from_slice(&size.to_le_bytes());
            central_dir.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            central_dir.extend_from_slice(&0u16.to_le_bytes());
            central_dir.extend_from_slice(&0u16.to_le_bytes());
            central_dir.extend_from_slice(&0u16.to_le_bytes());
            central_dir.extend_from_slice(&0u16.to_le_bytes());
            central_dir.extend_from_slice(&0u32.to_le_bytes());
            central_dir.extend_from_slice(&offset.to_le_bytes());
            central_dir.extend_from_slice(name_bytes);
        }

        let central_dir_offset = out.len() as u32;
        let central_dir_size = central_dir.len() as u32;
        let total_entries = self.files.len() as u16;

        out.extend_from_slice(&central_dir);

        out.extend_from_slice(&0x06054b50u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&total_entries.to_le_bytes());
        out.extend_from_slice(&total_entries.to_le_bytes());
        out.extend_from_slice(&central_dir_size.to_le_bytes());
        out.extend_from_slice(&central_dir_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());

        out
    }
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

fn col_name(mut index: usize) -> String {
    let mut name = String::new();
    loop {
        let rem = index % 26;
        name.insert(0, (b'A' + rem as u8) as char);
        if index < 26 {
            break;
        }
        index = (index / 26) - 1;
    }
    name
}

pub fn create_excel_xlsx(sheet_name: &str, headers: &[String], rows: &[Vec<Value>]) -> Vec<u8> {
    let clean_sheet_name = if sheet_name.trim().is_empty() {
        "Sheet1"
    } else {
        sheet_name.trim()
    };

    let mut zip = ZipBuilder::new();

    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
  <Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStringItem+xml"/>
</Types>"#;
    zip.add_file("[Content_Types].xml", content_types.as_bytes());

    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#;
    zip.add_file("_rels/.rels", root_rels.as_bytes());

    let wb_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>
</Relationships>"#;
    zip.add_file("xl/_rels/workbook.xml.rels", wb_rels.as_bytes());

    let workbook_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="{}" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#,
        escape_xml(clean_sheet_name)
    );
    zip.add_file("xl/workbook.xml", workbook_xml.as_bytes());

    let styles_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="2">
    <font><sz val="11"/><color theme="1"/><name val="Calibri"/><family val="2"/></font>
    <font><b/><sz val="11"/><color theme="1"/><name val="Calibri"/><family val="2"/></font>
  </fonts>
  <fills count="2">
    <fill><patternFill patternType="none"/></fill>
    <fill><patternFill patternType="gray125"/></fill>
  </fills>
  <borders count="1">
    <border><left/><right/><top/><bottom/><diagonal/></border>
  </borders>
  <cellStyleXfs count="1">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>
  </cellStyleXfs>
  <cellXfs count="2">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
    <xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/>
  </cellXfs>
</styleSheet>"#;
    zip.add_file("xl/styles.xml", styles_xml.as_bytes());

    let mut string_indices: BTreeMap<String, usize> = BTreeMap::new();
    let mut strings_list: Vec<String> = Vec::new();

    let mut add_string = |text: &str| {
        if !string_indices.contains_key(text) {
            let idx = strings_list.len();
            string_indices.insert(text.to_string(), idx);
            strings_list.push(text.to_string());
        }
    };

    for h in headers {
        add_string(h);
    }

    for row in rows {
        for cell in row {
            match cell {
                Value::String(s) => add_string(s),
                Value::Number(_) | Value::Bool(_) | Value::Null => {}
                other => add_string(&other.to_string()),
            }
        }
    }

    let mut sst_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="{}" uniqueCount="{}">"#,
        strings_list.len(),
        strings_list.len()
    );
    for s in &strings_list {
        sst_xml.push_str(&format!("<si><t>{}</t></si>", escape_xml(s)));
    }
    sst_xml.push_str("</sst>");
    zip.add_file("xl/sharedStrings.xml", sst_xml.as_bytes());

    let mut ws_xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>"#,
    );

    let mut row_idx = 1;

    if !headers.is_empty() {
        ws_xml.push_str(&format!(r#"<row r="{row_idx}">"#));
        for (col_i, header) in headers.iter().enumerate() {
            let c_ref = format!("{}{row_idx}", col_name(col_i));
            let s_idx = string_indices.get(header).copied().unwrap_or(0);
            ws_xml.push_str(&format!(r#"<c r="{c_ref}" s="1" t="s"><v>{s_idx}</v></c>"#));
        }
        ws_xml.push_str("</row>");
        row_idx += 1;
    }

    for row in rows {
        ws_xml.push_str(&format!(r#"<row r="{row_idx}">"#));
        for (col_i, cell) in row.iter().enumerate() {
            let c_ref = format!("{}{row_idx}", col_name(col_i));
            match cell {
                Value::Number(num) => {
                    ws_xml.push_str(&format!(r#"<c r="{c_ref}"><v>{num}</v></c>"#));
                }
                Value::Bool(b) => {
                    let b_val = if *b { 1 } else { 0 };
                    ws_xml.push_str(&format!(r#"<c r="{c_ref}" t="b"><v>{b_val}</v></c>"#));
                }
                Value::String(s) => {
                    let s_idx = string_indices.get(s).copied().unwrap_or(0);
                    ws_xml.push_str(&format!(r#"<c r="{c_ref}" t="s"><v>{s_idx}</v></c>"#));
                }
                Value::Null => {}
                other => {
                    let s = other.to_string();
                    let s_idx = string_indices.get(&s).copied().unwrap_or(0);
                    ws_xml.push_str(&format!(r#"<c r="{c_ref}" t="s"><v>{s_idx}</v></c>"#));
                }
            }
        }
        ws_xml.push_str("</row>");
        row_idx += 1;
    }

    ws_xml.push_str("</sheetData></worksheet>");
    zip.add_file("xl/worksheets/sheet1.xml", ws_xml.as_bytes());

    zip.finish()
}

pub fn create_csv(headers: &[String], rows: &[Vec<Value>]) -> String {
    let mut out = String::new();

    fn escape_csv_val(s: &str) -> String {
        if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }

    if !headers.is_empty() {
        let line = headers
            .iter()
            .map(|h| escape_csv_val(h))
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&line);
        out.push('\n');
    }

    for row in rows {
        let line = row
            .iter()
            .map(|val| match val {
                Value::String(s) => escape_csv_val(s),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => String::new(),
                other => escape_csv_val(&other.to_string()),
            })
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&line);
        out.push('\n');
    }

    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxSection {
    #[serde(default)]
    pub heading: Option<String>,
    #[serde(default)]
    pub level: Option<u8>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub bullets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxTable {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub headers: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
}

pub fn create_word_docx(
    title: &str,
    subtitle: Option<&str>,
    sections: &[DocxSection],
    tables: &[DocxTable],
) -> Vec<u8> {
    let mut zip = ZipBuilder::new();

    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>"#;
    zip.add_file("[Content_Types].xml", content_types.as_bytes());

    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;
    zip.add_file("_rels/.rels", root_rels.as_bytes());

    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#;
    zip.add_file("word/_rels/document.xml.rels", doc_rels.as_bytes());

    let styles_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/>
        <w:sz w:val="22"/>
        <w:color w:val="262626"/>
      </w:rPr>
    </w:rPrDefault>
  </w:docDefaults>
</w:styles>"#;
    zip.add_file("word/styles.xml", styles_xml.as_bytes());

    let mut doc_xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>"#,
    );

    if !title.trim().is_empty() {
        doc_xml.push_str(&format!(
            r#"<w:p>
  <w:pPr>
    <w:jc w:val="center"/>
    <w:spacing w:before="360" w:after="120"/>
  </w:pPr>
  <w:r>
    <w:rPr>
      <w:rFonts w:ascii="Segoe UI" w:hAnsi="Segoe UI"/>
      <w:b/>
      <w:sz w:val="52"/>
      <w:color w:val="0F172A"/>
    </w:rPr>
    <w:t>{}</w:t>
  </w:r>
</w:p>"#,
            escape_xml(title.trim())
        ));
    }

    if let Some(sub) = subtitle {
        if !sub.trim().is_empty() {
            doc_xml.push_str(&format!(
                r#"<w:p>
  <w:pPr>
    <w:jc w:val="center"/>
    <w:spacing w:before="60" w:after="360"/>
  </w:pPr>
  <w:r>
    <w:rPr>
      <w:rFonts w:ascii="Segoe UI" w:hAnsi="Segoe UI"/>
      <w:i/>
      <w:sz w:val="26"/>
      <w:color w:val="64748B"/>
    </w:rPr>
    <w:t>{}</w:t>
  </w:r>
</w:p>"#,
                escape_xml(sub.trim())
            ));
        }
    }

    for sec in sections {
        if let Some(ref h) = sec.heading {
            let level = sec.level.unwrap_or(1);
            let (sz_val, color_val, before_space) = match level {
                1 => ("36", "1E293B", "360"),
                2 => ("28", "334155", "240"),
                _ => ("24", "475569", "180"),
            };
            doc_xml.push_str(&format!(
                r#"<w:p>
  <w:pPr>
    <w:spacing w:before="{before_space}" w:after="120"/>
  </w:pPr>
  <w:r>
    <w:rPr>
      <w:rFonts w:ascii="Segoe UI" w:hAnsi="Segoe UI"/>
      <w:b/>
      <w:sz w:val="{sz_val}"/>
      <w:color w:val="{color_val}"/>
    </w:rPr>
    <w:t>{}</w:t>
  </w:r>
</w:p>"#,
                escape_xml(h.trim())
            ));
        }

        if let Some(ref text) = sec.text {
            for line in text.split("\n\n") {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    doc_xml.push_str(&format!(
                        r#"<w:p>
  <w:pPr>
    <w:spacing w:after="160" w:line="280" w:lineRule="auto"/>
  </w:pPr>
  <w:r>
    <w:rPr>
      <w:sz w:val="22"/>
    </w:rPr>
    <w:t>{}</w:t>
  </w:r>
</w:p>"#,
                        escape_xml(trimmed)
                    ));
                }
            }
        }

        for bullet in &sec.bullets {
            let trimmed = bullet.trim();
            if !trimmed.is_empty() {
                doc_xml.push_str(&format!(
                    r#"<w:p>
  <w:pPr>
    <w:ind w:left="480" w:hanging="240"/>
    <w:spacing w:after="80"/>
  </w:pPr>
  <w:r>
    <w:rPr><w:b/></w:rPr>
    <w:t>• </w:t>
  </w:r>
  <w:r>
    <w:rPr><w:sz w:val="22"/></w:rPr>
    <w:t>{}</w:t>
  </w:r>
</w:p>"#,
                    escape_xml(trimmed)
                ));
            }
        }
    }

    for tbl in tables {
        if let Some(ref title) = tbl.title {
            doc_xml.push_str(&format!(
                r#"<w:p>
  <w:pPr><w:spacing w:before="240" w:after="80"/></w:pPr>
  <w:r><w:rPr><w:b/><w:sz w:val="24"/></w:rPr><w:t>{}</w:t></w:r>
</w:p>"#,
                escape_xml(title)
            ));
        }

        doc_xml.push_str(
            r#"<w:tbl>
  <w:tblPr>
    <w:tblW w:w="0" w:type="auto"/>
    <w:tblBorders>
      <w:top w:val="single" w:sz="6" w:space="0" w:color="CBD5E1"/>
      <w:left w:val="none"/>
      <w:bottom w:val="single" w:sz="6" w:space="0" w:color="CBD5E1"/>
      <w:right w:val="none"/>
      <w:insideH w:val="single" w:sz="4" w:space="0" w:color="E2E8F0"/>
      <w:insideV w:val="none"/>
    </w:tblBorders>
    <w:tblCellMar>
      <w:top w:w="120" w:type="dxa"/>
      <w:bottom w:w="120" w:type="dxa"/>
      <w:left w:w="160" w:type="dxa"/>
      <w:right w:w="160" w:type="dxa"/>
    </w:tblCellMar>
  </w:tblPr>"#,
        );

        if !tbl.headers.is_empty() {
            doc_xml.push_str(r#"<w:tr><w:trPr><w:tblHeader/></w:trPr>"#);
            for h in &tbl.headers {
                doc_xml.push_str(&format!(
                    r#"<w:tc>
  <w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="F1F5F9"/></w:tcPr>
  <w:p><w:r><w:rPr><w:b/><w:sz w:val="22"/><w:color w:val="0F172A"/></w:rPr><w:t>{}</w:t></w:r></w:p>
</w:tc>"#,
                    escape_xml(h)
                ));
            }
            doc_xml.push_str("</w:tr>");
        }

        for row in &tbl.rows {
            doc_xml.push_str("<w:tr>");
            for cell in row {
                doc_xml.push_str(&format!(
                    r#"<w:tc>
  <w:p><w:r><w:rPr><w:sz w:val="22"/></w:rPr><w:t>{}</w:t></w:r></w:p>
</w:tc>"#,
                    escape_xml(cell)
                ));
            }
            doc_xml.push_str("</w:tr>");
        }

        doc_xml.push_str("</w:tbl>");
    }

    doc_xml.push_str(
        r#"<w:sectPr>
    <w:pgSz w:w="11906" w:h="16838"/>
    <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
  </w:sectPr>
</w:body></w:document>"#,
    );

    zip.add_file("word/document.xml", doc_xml.as_bytes());

    zip.finish()
}

pub fn create_markdown_doc(title: &str, sections: &[DocxSection], tables: &[DocxTable]) -> String {
    let mut out = String::new();
    if !title.trim().is_empty() {
        out.push_str(&format!("# {}\n\n", title.trim()));
    }

    for sec in sections {
        if let Some(ref h) = sec.heading {
            let level = sec.level.unwrap_or(2);
            let hashes = "#".repeat(level as usize);
            out.push_str(&format!("{} {}\n\n", hashes, h.trim()));
        }
        if let Some(ref text) = sec.text {
            out.push_str(text.trim());
            out.push_str("\n\n");
        }
        for b in &sec.bullets {
            out.push_str(&format!("- {}\n", b.trim()));
        }
        if !sec.bullets.is_empty() {
            out.push('\n');
        }
    }

    for tbl in tables {
        if let Some(ref t_title) = tbl.title {
            out.push_str(&format!("### {}\n\n", t_title.trim()));
        }
        if !tbl.headers.is_empty() {
            out.push_str(&format!("| {} |\n", tbl.headers.join(" | ")));
            let sep = tbl
                .headers
                .iter()
                .map(|_| "---")
                .collect::<Vec<_>>()
                .join(" | ");
            out.push_str(&format!("| {} |\n", sep));
            for row in &tbl.rows {
                out.push_str(&format!("| {} |\n", row.join(" | ")));
            }
            out.push('\n');
        }
    }

    out
}

pub fn create_html_doc(title: &str, sections: &[DocxSection], tables: &[DocxTable]) -> String {
    let mut out = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{}</title>
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; color: #1e293b; max-width: 900px; margin: 40px auto; padding: 0 20px; }}
h1 {{ color: #0f172a; border-bottom: 2px solid #e2e8f0; padding-bottom: 12px; }}
h2 {{ color: #1e293b; margin-top: 32px; }}
h3 {{ color: #334155; margin-top: 24px; }}
p {{ margin: 16px 0; }}
ul {{ padding-left: 24px; }}
li {{ margin: 6px 0; }}
table {{ border-collapse: collapse; width: 100%; margin: 24px 0; }}
th, td {{ border: 1px solid #cbd5e1; padding: 10px 14px; text-align: left; }}
th {{ background-color: #f1f5f9; font-weight: 600; color: #0f172a; }}
tr:nth-child(even) {{ background-color: #f8fafc; }}
</style>
</head>
<body>
<h1>{}</h1>
"#,
        escape_xml(title),
        escape_xml(title)
    );

    for sec in sections {
        if let Some(ref h) = sec.heading {
            let level = sec.level.unwrap_or(2).clamp(1, 4);
            out.push_str(&format!("<h{level}>{}</h{level}>\n", escape_xml(h)));
        }
        if let Some(ref text) = sec.text {
            out.push_str(&format!("<p>{}</p>\n", escape_xml(text)));
        }
        if !sec.bullets.is_empty() {
            out.push_str("<ul>\n");
            for b in &sec.bullets {
                out.push_str(&format!("  <li>{}</li>\n", escape_xml(b)));
            }
            out.push_str("</ul>\n");
        }
    }

    for tbl in tables {
        if let Some(ref t_title) = tbl.title {
            out.push_str(&format!("<h3>{}</h3>\n", escape_xml(t_title)));
        }
        if !tbl.headers.is_empty() {
            out.push_str("<table>\n<thead><tr>\n");
            for h in &tbl.headers {
                out.push_str(&format!("  <th>{}</th>\n", escape_xml(h)));
            }
            out.push_str("</tr></thead>\n<tbody>\n");
            for row in &tbl.rows {
                out.push_str("<tr>\n");
                for cell in row {
                    out.push_str(&format!("  <td>{}</td>\n", escape_xml(cell)));
                }
                out.push_str("</tr>\n");
            }
            out.push_str("</tbody>\n</table>\n");
        }
    }

    out.push_str("</body>\n</html>");
    out
}
