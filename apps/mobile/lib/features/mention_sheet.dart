import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../core/api.dart';
import '../core/mentions.dart';
import '../core/workspace.dart';
import '../ui/design.dart';

/// Bottom-sheet @mentions façon mobile : recherche + chips de catégories,
/// insertion tactile, même sources que le desktop (fichiers, skills,
/// agents, MCP, plugins, modèles, profils).
Future<String?> openMentionSheet({
  required BuildContext context,
  required Workspace workspace,
  required String initialQuery,
}) {
  return showModalBottomSheet<String>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    showDragHandle: true,
    builder: (_) => _MentionSheet(
      workspace: workspace,
      initialQuery: initialQuery,
    ),
  );
}

class _MentionSheet extends StatefulWidget {
  final Workspace workspace;
  final String initialQuery;
  const _MentionSheet({required this.workspace, required this.initialQuery});

  @override
  State<_MentionSheet> createState() => _MentionSheetState();
}

class _MentionSheetState extends State<_MentionSheet> {
  late String query = widget.initialQuery;
  String tab = 'Tout';
  List<MentionEntry> entries = [];
  bool loading = true;
  String? error;
  final search = TextEditingController();

  @override
  void initState() {
    super.initState();
    search.text = widget.initialQuery;
    _load();
  }

  @override
  void dispose() {
    search.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final w = widget.workspace;
      final results = await Future.wait([
        w.api.request('GET', '/files').catchError((_) => <dynamic>[]),
        w.api.request('GET', '/collections/skills').catchError((_) => <dynamic>[]),
        w.api
            .request('GET', '/collections/agent-definitions')
            .catchError((_) => <dynamic>[]),
        w.api.request('GET', '/collections/mcp-servers').catchError((_) => <dynamic>[]),
        w.api.request('GET', '/plugins').catchError((_) => <dynamic>[]),
        w.api
            .request('GET', '/collections/personalities')
            .catchError((_) => <dynamic>[]),
      ]);
      final out = <MentionEntry>[];
      for (final f in records(results[0]).take(60)) {
        final name = '${f['originalName'] ?? f['name'] ?? 'Fichier'}';
        out.add(
          MentionEntry(
            name: name,
            path: name,
            kind: 'Fichiers',
            subtitle: '${f['sizeBytes'] ?? 0} octets',
          ),
        );
      }
      for (final s in records(results[1]).take(40)) {
        final name = '${s['name'] ?? 'Skill'}';
        out.add(
          MentionEntry(
            name: name,
            path: 'skill:$name',
            kind: 'Skills',
            subtitle: '${s['description'] ?? ''}',
          ),
        );
      }
      for (final a in records(results[2]).take(40)) {
        final name = '${a['name'] ?? 'Agent'}';
        out.add(
          MentionEntry(
            name: name,
            path: 'agent:$name',
            kind: 'Agents',
            subtitle: '${a['description'] ?? ''}',
          ),
        );
      }
      for (final m in records(results[3]).take(40)) {
        final name = '${m['name'] ?? 'MCP'}';
        out.add(
          MentionEntry(
            name: name,
            path: 'mcp:$name',
            kind: 'MCP',
            subtitle: '${m['url'] ?? m['command'] ?? ''}',
          ),
        );
      }
      for (final p in records(results[4]).take(40)) {
        final name = '${p['name'] ?? 'Plugin'}';
        out.add(
          MentionEntry(
            name: name,
            path: 'plugin:$name',
            kind: 'Plugins',
            subtitle: '${p['description'] ?? ''}',
          ),
        );
      }
      for (final p in records(results[5]).take(40)) {
        final name = '${p['name'] ?? 'Profil'}';
        out.add(
          MentionEntry(
            name: name,
            path: 'profil:$name',
            kind: 'Profils',
            subtitle: '${p['description'] ?? ''}',
          ),
        );
      }
      final model = object(widget.workspace.settings['model']);
      for (final provider in records(model['providers'])) {
        for (final ref in records(provider['models'])) {
          final label = '${ref['label'] ?? ref['modelId'] ?? ''}';
          if (label.isEmpty) continue;
          out.add(
            MentionEntry(
              name: label,
              path: 'modele:$label',
              kind: 'Modèles',
              subtitle: '${provider['displayName'] ?? provider['kind'] ?? ''}',
            ),
          );
        }
      }
      if (!mounted) return;
      setState(() {
        entries = out;
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

  @override
  Widget build(BuildContext context) {
    const tabs = ['Tout', 'Fichiers', 'Skills', 'Agents', 'MCP', 'Plugins', 'Modèles'];
    final scoped = (tab == 'Tout' ? entries : entries.where((e) => e.kind == tab).toList());
    final ranked = filterMentionEntries(scoped, query, 30);
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.72,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
              child: TextField(
                controller: search,
                autofocus: true,
                textInputAction: TextInputAction.search,
                decoration: const InputDecoration(
                  hintText: 'Mentionner un fichier, skill, agent…',
                  prefixIcon: Icon(LucideIcons.atSign, size: 16),
                ),
                onChanged: (v) => setState(() => query = v),
              ),
            ),
            SizedBox(
              height: 40,
              child: ListView.separated(
                scrollDirection: Axis.horizontal,
                padding: const EdgeInsets.symmetric(horizontal: 16),
                itemCount: tabs.length,
                separatorBuilder: (_, _) => const SizedBox(width: 8),
                itemBuilder: (_, i) {
                  final selected = tab == tabs[i];
                  return ChoiceChip(
                    label: Text(tabs[i], style: const TextStyle(fontSize: 12)),
                    selected: selected,
                    onSelected: (_) {
                      HapticFeedback.selectionClick();
                      setState(() => tab = tabs[i]);
                    },
                  );
                },
              ),
            ),
            const SizedBox(height: 8),
            Expanded(
              child: loading
                  ? const Center(child: CircularProgressIndicator())
                  : error != null
                      ? Padding(
                          padding: const EdgeInsets.all(24),
                          child: Notice(error!, error: true, retry: _load),
                        )
                      : ranked.isEmpty
                          ? EmptyState(
                              icon: LucideIcons.atSign,
                              title: 'Aucune référence',
                              subtitle: query.isEmpty
                                  ? 'Tapez pour filtrer vos fichiers et outils.'
                                  : 'Aucun résultat pour « $query ».',
                            )
                          : ListView.builder(
                              itemCount: ranked.length,
                              itemBuilder: (_, i) {
                                final e = ranked[i];
                                return ListTile(
                                  leading: Icon(_iconFor(e.kind), size: 18),
                                  title: Text(
                                    e.name,
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                  subtitle: e.subtitle.isEmpty
                                      ? Text(
                                          e.kind,
                                          style: Theme.of(context)
                                              .textTheme
                                              .bodySmall,
                                        )
                                      : Text(
                                          '${e.kind} · ${e.subtitle}',
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis,
                                        ),
                                  trailing: Text(
                                    '@${e.path}',
                                    style: Theme.of(context).textTheme.bodySmall,
                                  ),
                                  onTap: () {
                                    HapticFeedback.selectionClick();
                                    Navigator.pop(context, e.path);
                                  },
                                );
                              },
                            ),
            ),
          ],
        ),
      ),
    );
  }

  IconData _iconFor(String kind) => switch (kind) {
        'Fichiers' => LucideIcons.fileText,
        'Skills' => LucideIcons.terminal,
        'Agents' => LucideIcons.bot,
        'MCP' => LucideIcons.network,
        'Plugins' => LucideIcons.puzzle,
        'Profils' => LucideIcons.user,
        'Modèles' => LucideIcons.cpu,
        _ => LucideIcons.atSign,
      };
}
