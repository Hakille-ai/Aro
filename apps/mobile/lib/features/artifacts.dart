import 'package:crypto/crypto.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../core/api.dart';
import '../core/workspace.dart';

/// Livrables extraits des réponses — miroir mobile de
/// `desktop/src/lib/artifacts.ts` + `CodeDiffViewer.svelte` (version unifiée,
/// adaptée au tactile : pas de split-screen sur téléphone).
enum ArtifactStatus { pending, applied, rejected }

class CodeArtifact {
  final String id;
  final String title;
  final String language;
  final String code;
  ArtifactStatus status;
  CodeArtifact({
    required this.id,
    required this.title,
    required this.language,
    required this.code,
    this.status = ArtifactStatus.pending,
  });

  bool get isDiff =>
      language.toLowerCase() == 'diff' || _looksLikeDiff(code);
  int get added =>
      isDiff ? code.split('\n').where((l) => l.startsWith('+') && !l.startsWith('+++')).length : 0;
  int get removed =>
      isDiff ? code.split('\n').where((l) => l.startsWith('-') && !l.startsWith('---')).length : 0;

  static bool _looksLikeDiff(String code) {
    var plus = 0, minus = 0;
    for (final line in code.split('\n').take(40)) {
      if (line.startsWith('+') && !line.startsWith('+++')) plus++;
      if (line.startsWith('-') && !line.startsWith('---')) minus++;
    }
    return plus + minus >= 3;
  }
}

/// Extrait chaque bloc ``` des messages assistants.
List<CodeArtifact> extractCodeArtifacts(List<Json> messages) {
  final out = <CodeArtifact>[];
  final fence = RegExp(r'```(\w[\w+-]*vast)?\s*\n([\s\S]*?)```');
  // Regex simple ci-dessus ne couvre pas tout ; on parse manuellement.
  var index = 0;
  for (final message in messages) {
    if (message['role'] != 'assistant') continue;
    final content = '${message['content'] ?? ''}';
    var cursor = 0;
    while (true) {
      final start = content.indexOf('```', cursor);
      if (start < 0) break;
      final headerEnd = content.indexOf('\n', start);
      if (headerEnd < 0) break;
      final language = content.substring(start + 3, headerEnd).trim().toLowerCase();
      final end = content.indexOf('```', headerEnd + 1);
      if (end < 0) break;
      final code = content.substring(headerEnd + 1, end).trim();
      if (code.isNotEmpty) {
        final firstLine = code.split('\n').first.trim();
        final title = language.isEmpty
            ? (firstLine.length > 42 ? '${firstLine.substring(0, 42)}…' : firstLine)
            : '$language · ${firstLine.length > 32 ? '${firstLine.substring(0, 32)}…' : firstLine}';
        out.add(
          CodeArtifact(
            id: 'artifact-$index',
            title: title.isEmpty ? 'Extrait ${index + 1}' : title,
            language: language.isEmpty ? 'code' : language,
            code: code,
          ),
        );
        index++;
      }
      cursor = end + 3;
    }
  }
  // `fence` garde la parité documentaire avec le desktop (détection stricte).
  assert(fence.pattern.isNotEmpty);
  return out;
}

String _extensionFor(String language) => switch (language.toLowerCase()) {
      'dart' => 'dart',
      'ts' || 'typescript' => 'ts',
      'js' || 'javascript' => 'js',
      'py' || 'python' => 'py',
      'rs' || 'rust' => 'rs',
      'json' => 'json',
      'md' || 'markdown' => 'md',
      'html' => 'html',
      'css' => 'css',
      'sql' => 'sql',
      'yaml' || 'yml' => 'yaml',
      _ => 'txt',
    };

/// Fiche détail : diff unifié + Copier / Enregistrer / Rejeter.
Future<void> openArtifactDetail({
  required BuildContext context,
  required Workspace workspace,
  required CodeArtifact artifact,
  required VoidCallback onChanged,
}) {
  return showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    showDragHandle: true,
    builder: (_) => _ArtifactDetail(
      workspace: workspace,
      artifact: artifact,
      onChanged: onChanged,
    ),
  );
}

class _ArtifactDetail extends StatefulWidget {
  final Workspace workspace;
  final CodeArtifact artifact;
  final VoidCallback onChanged;
  const _ArtifactDetail({
    required this.workspace,
    required this.artifact,
    required this.onChanged,
  });

  @override
  State<_ArtifactDetail> createState() => _ArtifactDetailState();
}

class _ArtifactDetailState extends State<_ArtifactDetail> {
  bool busy = false;

  CodeArtifact get a => widget.artifact;

  Future<void> _copy() async {
    await Clipboard.setData(ClipboardData(text: a.code));
    if (!mounted) return;
    HapticFeedback.selectionClick();
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(content: Text('Code copié')),
    );
  }

  Future<void> _save() async {
    setState(() => busy = true);
    try {
      final bytes = Uint8List.fromList(a.code.codeUnits);
      final name = 'aro-${DateTime.now().millisecondsSinceEpoch}.${_extensionFor(a.language)}';
      final created = await widget.workspace.api.upload(
        name,
        bytes,
        sha256.convert(bytes).toString(),
      );
      if (!mounted) return;
      setState(() {
        a.status = ArtifactStatus.applied;
        busy = false;
      });
      widget.onChanged();
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Enregistré : ${created['originalName'] ?? name}')),
      );
      Navigator.pop(context);
    } catch (e) {
      if (!mounted) return;
      setState(() => busy = false);
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.82,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
              child: Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          a.title,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: Theme.of(context).textTheme.titleMedium,
                        ),
                        const SizedBox(height: 4),
                        Row(
                          children: [
                            _StatusBadge(status: a.status),
                            const SizedBox(width: 8),
                            Text(
                              '.${_extensionFor(a.language)}',
                              style: Theme.of(context).textTheme.bodySmall,
                            ),
                            if (a.isDiff) ...[
                              const SizedBox(width: 8),
                              Text(
                                '+${a.added} −${a.removed}',
                                style: TextStyle(
                                  fontSize: 11,
                                  color: scheme.secondary,
                                  fontFamily: 'monospace',
                                ),
                              ),
                            ],
                          ],
                        ),
                      ],
                    ),
                  ),
                  IconButton(
                    tooltip: 'Copier le code',
                    onPressed: _copy,
                    icon: const Icon(LucideIcons.copy, size: 18),
                  ),
                ],
              ),
            ),
            const Divider(height: 1),
            Expanded(
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(12),
                child: Container(
                  decoration: BoxDecoration(
                    color: scheme.onSurface.withValues(alpha: .04),
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(color: scheme.outlineVariant),
                  ),
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      for (var i = 0; i < a.code.split('\n').length; i++)
                        _CodeRow(
                          number: i + 1,
                          line: a.code.split('\n')[i],
                        ),
                    ],
                  ),
                ),
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
              child: Row(
                children: [
                  Expanded(
                    child: OutlinedButton(
                      onPressed: busy
                          ? null
                          : () {
                              setState(
                                () => a.status = ArtifactStatus.rejected,
                              );
                              widget.onChanged();
                              Navigator.pop(context);
                            },
                      child: const Text('Rejeter'),
                    ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: FilledButton(
                      onPressed: busy || a.status == ArtifactStatus.applied
                          ? null
                          : _save,
                      child: busy
                          ? const SizedBox(
                              width: 18,
                              height: 18,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Text('Enregistrer'),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _CodeRow extends StatelessWidget {
  final int number;
  final String line;
  const _CodeRow({required this.number, required this.line});

  @override
  Widget build(BuildContext context) {
    final isAdd = line.startsWith('+') && !line.startsWith('+++');
    final isDel = line.startsWith('-') && !line.startsWith('---');
    final bg = isAdd
        ? const Color(0xff28c840).withValues(alpha: .1)
        : isDel
            ? const Color(0xffff3b30).withValues(alpha: .1)
            : null;
    final fg = isAdd
        ? const Color(0xff1a7f37)
        : isDel
            ? const Color(0xffb3261e)
            : Theme.of(context).colorScheme.onSurface;
    return Container(
      color: bg,
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 1),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 34,
            child: Text(
              '$number',
              style: TextStyle(
                fontSize: 10,
                color: Theme.of(context).colorScheme.onSurfaceVariant,
                fontFamily: 'monospace',
              ),
            ),
          ),
          Expanded(
            child: SelectableText(
              line.isEmpty ? ' ' : line,
              style: TextStyle(fontSize: 12, fontFamily: 'monospace', color: fg),
            ),
          ),
        ],
      ),
    );
  }
}

class _StatusBadge extends StatelessWidget {
  final ArtifactStatus status;
  const _StatusBadge({required this.status});

  @override
  Widget build(BuildContext context) {
    final (label, color) = switch (status) {
      ArtifactStatus.pending => ('En attente', const Color(0xffff9500)),
      ArtifactStatus.applied => ('Enregistré', const Color(0xff28c840)),
      ArtifactStatus.rejected => ('Rejeté', const Color(0xffff3b30)),
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: .1),
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: color.withValues(alpha: .3)),
      ),
      child: Text(
        label,
        style: TextStyle(fontSize: 10, fontWeight: FontWeight.w700, color: color),
      ),
    );
  }
}
