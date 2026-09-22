import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../core/workspace.dart';
import 'settings.dart';
import 'settings_catalog.dart';

/// Palette de commande mobile — équivalent tactile du
/// `CommandPalette.svelte` desktop : conversations, projets, dossiers,
/// actions et réglages dans une bottom-sheet de recherche.
Future<void> openCommandPalette({
  required BuildContext context,
  required Workspace workspace,
  required Future<void> Function(String? id) onSelectConversation,
  required VoidCallback onNewConversation,
  required VoidCallback onOpenWorkspace,
}) {
  return showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    showDragHandle: true,
    builder: (_) => _CommandPalette(
      workspace: workspace,
      onSelectConversation: onSelectConversation,
      onNewConversation: onNewConversation,
      onOpenWorkspace: onOpenWorkspace,
    ),
  );
}

class _CommandPalette extends StatefulWidget {
  final Workspace workspace;
  final Future<void> Function(String? id) onSelectConversation;
  final VoidCallback onNewConversation;
  final VoidCallback onOpenWorkspace;
  const _CommandPalette({
    required this.workspace,
    required this.onSelectConversation,
    required this.onNewConversation,
    required this.onOpenWorkspace,
  });

  @override
  State<_CommandPalette> createState() => _CommandPaletteState();
}

class _CommandPaletteState extends State<_CommandPalette> {
  String query = '';
  final search = TextEditingController();

  @override
  void dispose() {
    search.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final w = widget.workspace;
    final q = query.trim().toLowerCase();
    bool matches(String value) =>
        q.isEmpty || value.toLowerCase().contains(q);

    final conversations = w.conversations
        .where((c) => matches('${c['title'] ?? ''}'))
        .take(8)
        .toList();
    final projects = w.projects
        .where((p) => matches('${p['name'] ?? ''}'))
        .take(5)
        .toList();
    final folders = w.folders
        .where((f) => matches('${f['name'] ?? ''}'))
        .take(5)
        .toList();
    final settingHits = settingsSections
        .where(
          (s) =>
              matches(s.title) ||
              matches(s.description) ||
              matches(s.group),
        )
        .take(6)
        .toList();

    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.78,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
              child: TextField(
                controller: search,
                autofocus: true,
                textInputAction: TextInputAction.search,
                decoration: const InputDecoration(
                  hintText: 'Rechercher conversations, projets, actions…',
                  prefixIcon: Icon(LucideIcons.search, size: 16),
                ),
                onChanged: (v) => setState(() => query = v),
              ),
            ),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.fromLTRB(8, 0, 8, 24),
                children: [
                  _SectionLabel('Actions'),
                  ListTile(
                    leading: const Icon(LucideIcons.squarePen, size: 18),
                    title: const Text('Nouvelle conversation'),
                    trailing: const Text(
                      'Ctrl N',
                      style: TextStyle(fontSize: 11, color: Color(0xff86868b)),
                    ),
                    onTap: () {
                      HapticFeedback.selectionClick();
                      Navigator.pop(context);
                      widget.onNewConversation();
                    },
                  ),
                  ListTile(
                    leading: const Icon(LucideIcons.panelRight, size: 18),
                    title: const Text('Ouvrir plans, fichiers et agents'),
                    onTap: () {
                      HapticFeedback.selectionClick();
                      Navigator.pop(context);
                      widget.onOpenWorkspace();
                    },
                  ),
                  ListTile(
                    leading: const Icon(LucideIcons.settings, size: 18),
                    title: const Text('Paramètres'),
                    onTap: () {
                      HapticFeedback.selectionClick();
                      Navigator.pop(context);
                      Navigator.push(
                        context,
                        MaterialPageRoute(
                          builder: (_) => SettingsPage(workspace: w),
                        ),
                      );
                    },
                  ),
                  if (conversations.isNotEmpty) ...[
                    _SectionLabel('Conversations'),
                    for (final c in conversations)
                      ListTile(
                        leading: const Icon(
                          LucideIcons.messageCircle,
                          size: 18,
                        ),
                        title: Text(
                          '${c['title'] ?? 'Conversation'}',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                        trailing: w.activeId == c['id']
                            ? const Icon(LucideIcons.check, size: 16)
                            : null,
                        onTap: () async {
                          HapticFeedback.selectionClick();
                          final id = '${c['id']}';
                          Navigator.pop(context);
                          await widget.onSelectConversation(id);
                        },
                      ),
                  ],
                  if (projects.isNotEmpty) ...[
                    _SectionLabel('Projets'),
                    for (final p in projects)
                      ListTile(
                        leading: const Icon(LucideIcons.folderKanban, size: 18),
                        title: Text(
                          '${p['name'] ?? 'Projet'}',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                        onTap: () {
                          HapticFeedback.selectionClick();
                          Navigator.pop(context);
                          widget.onOpenWorkspace();
                        },
                      ),
                  ],
                  if (folders.isNotEmpty) ...[
                    _SectionLabel('Dossiers'),
                    for (final f in folders)
                      ListTile(
                        leading: const Icon(LucideIcons.folder, size: 18),
                        title: Text(
                          '${f['name'] ?? 'Dossier'}',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                        onTap: () {
                          HapticFeedback.selectionClick();
                          Navigator.pop(context);
                          widget.onOpenWorkspace();
                        },
                      ),
                  ],
                  if (settingHits.isNotEmpty) ...[
                    _SectionLabel('Réglages'),
                    for (final s in settingHits)
                      ListTile(
                        leading: Icon(s.icon, size: 18),
                        title: Text(s.title),
                        subtitle: Text(
                          s.group,
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                        onTap: () {
                          HapticFeedback.selectionClick();
                          Navigator.pop(context);
                          Navigator.push(
                            context,
                            MaterialPageRoute(
                              builder: (_) => SettingsPage(
                                workspace: w,
                                initialSection: s.id,
                              ),
                            ),
                          );
                        },
                      ),
                  ],
                  if (q.isNotEmpty &&
                      conversations.isEmpty &&
                      projects.isEmpty &&
                      folders.isEmpty &&
                      settingHits.isEmpty)
                    const Padding(
                      padding: EdgeInsets.all(24),
                      child: Text(
                        'Aucun résultat. Essayez un autre mot-clé.',
                        textAlign: TextAlign.center,
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

class _SectionLabel extends StatelessWidget {
  final String text;
  const _SectionLabel(this.text);

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.fromLTRB(12, 14, 12, 4),
        child: Text(
          text.toUpperCase(),
          style: const TextStyle(
            fontSize: 10,
            fontWeight: FontWeight.w700,
            letterSpacing: .2,
            color: Color(0xff86868b),
          ),
        ),
      );
}
