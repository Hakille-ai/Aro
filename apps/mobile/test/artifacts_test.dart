import 'package:flutter_test/flutter_test.dart';
import 'package:aro_mobile/features/artifacts.dart';

void main() {
  test('extractCodeArtifacts finds fenced blocks with language', () {
    final artifacts = extractCodeArtifacts([
      {
        'role': 'assistant',
        'content': 'voici\n```dart\nvoid main() {}\n```\npuis\n```diff\n+ok\n-ok\n+ok2\n```',
      },
      {'role': 'user', 'content': '```dart\nne pas extraire\n```'},
    ]);
    expect(artifacts.length, 2);
    expect(artifacts.first.language, 'dart');
    expect(artifacts.last.isDiff, isTrue);
    expect(artifacts.last.added, 2);
    expect(artifacts.last.removed, 1);
  });

  test('extractCodeArtifacts ignores unclosed fences', () {
    expect(
      extractCodeArtifacts([
        {'role': 'assistant', 'content': '```dart\nsans fin'},
      ]),
      isEmpty,
    );
  });
}
