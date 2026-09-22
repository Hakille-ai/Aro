import 'package:flutter_test/flutter_test.dart';
import 'package:aro_mobile/core/mentions.dart';

void main() {
  test('detectMentionQuery finds @query before cursor, not emails', () {
    final trigger = detectMentionQuery('bonjour @doc', 12);
    expect(trigger, isNotNull);
    expect(trigger!.query, 'doc');
    expect(detectMentionQuery('user@example.com', 16), isNull);
    expect(detectMentionQuery('sans mention', 12), isNull);
  });

  test('filterMentionEntries ranks exact > prefix > substring', () {
    const entries = [
      MentionEntry(name: 'doc.pdf', path: 'doc.pdf', kind: 'Fichiers'),
      MentionEntry(name: 'docker', path: 'docker', kind: 'Skills'),
      MentionEntry(name: 'readme', path: 'docs/readme', kind: 'Fichiers'),
    ];
    final ranked = filterMentionEntries(entries, 'doc');
    expect(ranked.first.name, 'doc.pdf');
    expect(ranked.length, 3);
  });

  test('applyMentionSelection replaces trigger and moves cursor', () {
    const text = 'voir @do';
    final trigger = detectMentionQuery(text, text.length)!;
    final applied = applyMentionSelection(text, trigger, 'doc.pdf');
    expect(applied.newText, 'voir @doc.pdf ');
    expect(applied.newCursor, applied.newText.length);
  });

  test('extract + dedupe keep first order', () {
    const text = 'voir @doc et @skill:x puis @doc';
    expect(dedupeMentionedPaths(extractMentionedPaths(text)), ['doc', 'skill:x']);
  });
}
