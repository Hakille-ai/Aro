//! Strict single-file patches. Never guess a location or accept stale context.
pub fn apply_unified_patch(original: &str, patch: &str) -> Result<String, String> {
    if patch.trim().is_empty() {
        return Ok(original.to_owned());
    }
    let source: Vec<&str> = original.lines().collect();
    let lines: Vec<&str> = patch.lines().collect();
    let mut output: Vec<&str> = Vec::new();
    let mut cursor = 0;
    let mut i = 0;
    let mut hunks = 0;
    while i < lines.len() {
        let line = lines[i];
        if !line.starts_with("@@") {
            if hunks > 0 {
                return Err(
                    "Patch invalide : contenu après le dernier bloc ou plusieurs fichiers".into(),
                );
            }
            i += 1;
            continue;
        }
        let header = line
            .strip_prefix("@@ ")
            .and_then(|s| s.split_once(" @@"))
            .ok_or("En-tête de patch invalide")?
            .0;
        let ranges: Vec<_> = header.split_whitespace().collect();
        if ranges.len() != 2 {
            return Err("Plages de patch invalides".into());
        }
        let (old_start, old_count) = range(ranges[0], '-')?;
        let (_, new_count) = range(ranges[1], '+')?;
        let start = if old_count == 0 {
            old_start
        } else {
            old_start.checked_sub(1).ok_or("Ligne de départ invalide")?
        };
        if start < cursor || start > source.len() {
            return Err("Conflit : plage hors du fichier ou blocs superposés".into());
        }
        output.extend_from_slice(&source[cursor..start]);
        cursor = start;
        let mut removed = 0usize;
        let mut added = 0usize;
        i += 1;
        while i < lines.len() && !lines[i].starts_with("@@") {
            let change = lines[i];
            if change == "\\ No newline at end of file" {
                i += 1;
                continue;
            }
            if removed == old_count && added == new_count {
                break;
            }
            let (kind, content) = if change.is_empty() {
                (' ', "") // tolerate an unprefixed empty context line
            } else {
                let kind = change.chars().next().unwrap();
                (kind, &change[kind.len_utf8()..])
            };
            match kind {
                ' ' | '-' => {
                    if source.get(cursor).copied() != Some(content) {
                        return Err(format!("Conflit à la ligne {} : le fichier a changé. Régénérez la proposition.", cursor + 1));
                    }
                    if kind == ' ' {
                        output.push(content);
                        added += 1;
                    }
                    cursor += 1;
                    removed += 1;
                }
                '+' => {
                    output.push(content);
                    added += 1;
                }
                _ => return Err("Ligne de patch invalide".into()),
            }
            if removed > old_count || added > new_count {
                return Err("Nombre de lignes du patch invalide".into());
            }
            i += 1;
        }
        if removed != old_count || added != new_count {
            return Err("Patch incomplet : nombre de lignes différent de l'en-tête".into());
        }
        hunks += 1;
    }
    if hunks == 0 {
        if patch.starts_with("diff --git") || patch.starts_with("--- ") || patch.starts_with("+++ ")
        {
            return Ok(original.to_owned());
        }
        return Err("Un diff unifié est requis ; remplacement intégral refusé".into());
    }
    output.extend_from_slice(&source[cursor..]);
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut result = output.join(newline);
    if original.ends_with('\n') && !output.is_empty() {
        result.push_str(newline);
    }
    Ok(result)
}

fn range(spec: &str, prefix: char) -> Result<(usize, usize), String> {
    let spec = spec.strip_prefix(prefix).ok_or("Signe de plage invalide")?;
    let (start, count) = spec.split_once(',').unwrap_or((spec, "1"));
    Ok((
        start.parse().map_err(|_| "Position de patch invalide")?,
        count.parse().map_err(|_| "Taille de patch invalide")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_stale_deletion_and_context() {
        assert!(
            apply_unified_patch("user-new-value\n", "@@ -1 +1 @@\n-old-value\n+ai-value\n")
                .is_err()
        );
        assert!(apply_unified_patch(
            "new-context\nold\n",
            "@@ -1,2 +1,2 @@\n old-context\n-old\n+new\n"
        )
        .is_err());
    }
    #[test]
    fn rejects_raw_replacement_and_truncated_patch() {
        assert!(apply_unified_patch("user work", "replacement").is_err());
        assert!(apply_unified_patch("a\nb\n", "@@ -1,2 +1,2 @@\n-a\n+A\n").is_err());
    }
    #[test]
    fn inserts_at_zero_length_range_without_deleting_a_line() {
        assert_eq!(
            apply_unified_patch("a\nb\n", "@@ -1,0 +2,1 @@\n+new\n").unwrap(),
            "a\nnew\nb\n"
        );
    }
    #[test]
    fn rejects_overlapping_hunks_and_multiple_files() {
        assert!(apply_unified_patch("a\n", "@@ -1 +1 @@\n-a\n+A\n@@ -1 +1 @@\n-a\n+B\n").is_err());
        assert!(apply_unified_patch("a\n", "@@ -1 +1 @@\n-a\n+A\ndiff --git a/b b/b\n").is_err());
    }
}
