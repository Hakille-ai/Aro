/// Modèle pur @mentions — miroir mobile de
/// `apps/desktop/src/features/chat/mention-model.ts` (F4.1/F4.3).
///
/// Reste testable sans Flutter : détection, filtrage, insertion,
/// extraction. L'UI vit dans `features/mention_sheet.dart`.
class MentionTrigger {
  final bool active;
  final String query;
  final int startIndex;
  final int endIndex;
  const MentionTrigger({
    required this.active,
    required this.query,
    required this.startIndex,
    required this.endIndex,
  });
}

class MentionEntry {
  final String name;
  final String path;
  final String kind;
  final String subtitle;
  const MentionEntry({
    required this.name,
    required this.path,
    required this.kind,
    this.subtitle = '',
  });
}

final _emailGuard = RegExp(r'[A-Za-z0-9._%+-]@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$');

/// Détecte `@query` juste avant [cursorIndex].
/// Déclencheur en début de ligne ou après espace, jamais dans un e-mail.
MentionTrigger? detectMentionQuery(String text, int cursorIndex) {
  final safe = cursorIndex.clamp(0, text.length);
  final before = text.substring(0, safe);
  final match = RegExp(r'(?:^|\s)@([a-zA-Z0-9_\-./\\:]*)$').firstMatch(before);
  if (match == null) return null;
  final query = match.group(1) ?? '';
  final token = '@$query';
  final start = before.lastIndexOf(token);
  if (start < 0) return null;
  final charBefore = start > 0 ? before[start - 1] : '';
  if (charBefore.isNotEmpty && RegExp(r'[A-Za-z0-9._%+-]').hasMatch(charBefore)) {
    return null;
  }
  if (_emailGuard.hasMatch(before.substring(0, start + 1))) return null;
  return MentionTrigger(
    active: true,
    query: query,
    startIndex: start,
    endIndex: safe,
  );
}

/// Filtre + classe : exact > préfixe > sous-chaîne > chemin.
List<MentionEntry> filterMentionEntries(
  List<MentionEntry> entries,
  String query, [
  int limit = 12,
]) {
  if (query.trim().isEmpty) return entries.take(limit).toList();
  final q = query.toLowerCase().replaceAll('\\', '/');
  final scored = <({MentionEntry entry, int score})>[];
  for (final entry in entries) {
    final name = entry.name.toLowerCase();
    final path = entry.path.toLowerCase().replaceAll('\\', '/');
    var score = 0;
    if (name == q) {
      score = 100;
    } else if (name.startsWith(q)) {
      score = 60;
    } else if (name.contains(q)) {
      score = 30;
    } else if (path.contains(q)) {
      score = 10;
    }
    if (score > 0) scored.add((entry: entry, score: score));
  }
  scored.sort((a, b) => b.score.compareTo(a.score));
  return scored.take(limit).map((s) => s.entry).toList();
}

/// Remplace la plage du trigger par `@path ` et retourne le curseur.
({String newText, int newCursor}) applyMentionSelection(
  String text,
  MentionTrigger trigger,
  String selectedPath,
) {
  final before = text.substring(0, trigger.startIndex);
  final after = text.substring(trigger.endIndex);
  final inserted = '@$selectedPath ';
  return (
    newText: '$before$inserted$after',
    newCursor: before.length + inserted.length,
  );
}

/// Extrait chaque `@path` d'un message.
List<String> extractMentionedPaths(String text) {
  if (text.isEmpty) return [];
  return RegExp(r'(?:^|\s)@([a-zA-Z0-9_\-./\\:]+)')
      .allMatches(text)
      .map((m) => m.group(1)!)
      .toList();
}

/// Déduplique en gardant le premier ordre d'apparition.
List<String> dedupeMentionedPaths(List<String> paths) {
  final seen = <String>{};
  final out = <String>[];
  for (final p in paths) {
    final key = p.replaceAll('\\', '/');
    if (seen.add(key)) out.add(p);
  }
  return out;
}
