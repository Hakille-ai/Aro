import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../core/api.dart';
import '../core/workspace.dart';
import '../ui/design.dart';

/// Aperçu fichier cloud — miroir mobile de `WorkspaceFileViewer.svelte` :
/// breadcrumb, copie, texte/code (tronqué à 200 000 caractères comme desktop),
/// images natives. Les binaires non-texte affichent un état honnête.
Future<void> openFilePreview({
  required BuildContext context,
  required Workspace workspace,
  required Json file,
}) {
  return showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    showDragHandle: true,
    builder: (_) => _FilePreview(workspace: workspace, file: file),
  );
}

class _FilePreview extends StatefulWidget {
  final Workspace workspace;
  final Json file;
  const _FilePreview({required this.workspace, required this.file});

  @override
  State<_FilePreview> createState() => _FilePreviewState();
}

class _FilePreviewState extends State<_FilePreview> {
  bool loading = true;
  String? error;
  String? text;
  Uint8List? bytes;
  bool truncated = false;

  String get name =>
      '${widget.file['originalName'] ?? widget.file['name'] ?? 'Fichier'}';

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final id = '${widget.file['id'] ?? ''}';
      if (id.isEmpty) throw const ApiException(404, 'Fichier introuvable.');
      final data = await widget.workspace.api.downloadBytes(id);
      if (!mounted) return;
      setState(() {
        bytes = data;
        if (_isImage(name)) {
          text = null;
        } else {
          try {
            final decoded = utf8.decode(data, allowMalformed: false);
            if (decoded.length > 200000) {
              text = '${decoded.substring(0, 200000)}\n…[tronqué]';
              truncated = true;
            } else {
              text = decoded;
            }
          } catch (_) {
            text = null;
          }
        }
        loading = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        error = '$e';
        loading = false;
      });
    }
  }

  bool _isImage(String filename) {
    final lower = filename.toLowerCase();
    return lower.endsWith('.png') ||
        lower.endsWith('.jpg') ||
        lower.endsWith('.jpeg') ||
        lower.endsWith('.gif') ||
        lower.endsWith('.webp');
  }

  @override
  Widget build(BuildContext context) {
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
                          name,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: Theme.of(context).textTheme.titleMedium,
                        ),
                        const SizedBox(height: 4),
                        Text(
                          '${widget.file['sizeBytes'] ?? bytes?.length ?? 0} octets${truncated ? ' · tronqué' : ''}',
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                      ],
                    ),
                  ),
                  if (text != null)
                    IconButton(
                      tooltip: 'Copier le contenu',
                      onPressed: () async {
                        await Clipboard.setData(ClipboardData(text: text!));
                        if (!context.mounted) return;
                        HapticFeedback.selectionClick();
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(content: Text('Contenu copié')),
                        );
                      },
                      icon: const Icon(LucideIcons.copy, size: 18),
                    ),
                ],
              ),
            ),
            const Divider(height: 1),
            Expanded(
              child: loading
                  ? const Center(child: CircularProgressIndicator())
                  : error != null
                      ? Padding(
                          padding: const EdgeInsets.all(24),
                          child: Notice(error!, error: true, retry: _load),
                        )
                      : bytes != null && _isImage(name)
                          ? SingleChildScrollView(
                              padding: const EdgeInsets.all(16),
                              child: ClipRRect(
                                borderRadius: BorderRadius.circular(12),
                                child: Image.memory(bytes!, fit: BoxFit.contain),
                              ),
                            )
                          : text != null
                              ? SingleChildScrollView(
                                  padding: const EdgeInsets.all(16),
                                  child: SelectableText(
                                    text!,
                                    style: const TextStyle(
                                      fontSize: 12,
                                      fontFamily: 'monospace',
                                      height: 1.5,
                                    ),
                                  ),
                                )
                              : EmptyState(
                                  icon: LucideIcons.fileWarning,
                                  title: 'Aperçu indisponible',
                                  subtitle:
                                      'Ce fichier binaire ne peut pas être affiché en texte sur ce téléphone.',
                                ),
            ),
          ],
        ),
      ),
    );
  }
}
