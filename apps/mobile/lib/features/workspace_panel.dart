import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../core/api.dart';
import '../core/plans.dart';
import '../core/workspace.dart';
import '../ui/design.dart';
import 'artifacts.dart';
import 'file_preview.dart';
import 'settings.dart';
import 'settings_catalog.dart';
import 'package:url_launcher/url_launcher.dart';

class WorkspacePanel extends StatefulWidget {
  final Workspace workspace;
  final VoidCallback? onClose;
  const WorkspacePanel({super.key, required this.workspace, this.onClose});
  @override
  State<WorkspacePanel> createState() => _WorkspacePanelState();
}

class _WorkspacePanelState extends State<WorkspacePanel> {
  List<Json> plans = [], files = [], runs = [];
  bool loading = true, overview = true;
  final address = TextEditingController();
  final outputSearch = TextEditingController();
  final fileSearch = TextEditingController();
  String outputFilter = 'Tous';
  final Map<String, ArtifactStatus> artifactStatus = {};
  @override
  void dispose() {
    address.dispose();
    outputSearch.dispose();
    fileSearch.dispose();
    super.dispose();
  }

  String? error;
  Workspace get w => widget.workspace;
  @override
  void initState() {
    super.initState();
    load();
  }

  Future<void> load() async {
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final results = await Future.wait([
        w.api.request('GET', '/collections/plans'),
        w.api.request('GET', '/files'),
        w.api.request('GET', '/agent/runs'),
      ]);
      plans = records(
        results[0],
      ).where((p) => p['conversationId'] == w.activeId).toList();
      files = records(results[1]);
      runs = records(results[2])
          .where((r) => w.activeId == null || r['conversationId'] == w.activeId)
          .toList();
    } catch (e) {
      error = '$e';
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  @override
  Widget build(BuildContext context) => DefaultTabController(
    length: 6,
    child: Scaffold(
      // Header épuré : pas de titre, pas d'actualiser.
      // Le fermer vit à droite des onglets, à portée de pouce.
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(10, 10, 10, 6),
            child: Row(
              children: [
                Expanded(
                  child: TabBar(
                    onTap: (_) => setState(() => overview = false),
                    labelColor: Theme.of(context).colorScheme.primary,
                    unselectedLabelColor: const Color(0xff86868b),
                    labelPadding: EdgeInsets.zero,
                    indicatorSize: TabBarIndicatorSize.tab,
                    indicator: BoxDecoration(
                      color: Theme.of(context).colorScheme.surface,
                      borderRadius: BorderRadius.circular(8),
                      border: Border.all(
                        color: Theme.of(context).colorScheme.outlineVariant,
                      ),
                    ),
                    dividerHeight: 0,
                    tabs: const [
                      Tab(
                        icon: Tooltip(
                          message: 'Sorties & Artefacts',
                          child: Icon(LucideIcons.fileText, size: 16),
                        ),
                      ),
                      Tab(
                        icon: Tooltip(
                          message: 'Sous-Agents',
                          child: Icon(LucideIcons.bot, size: 16),
                        ),
                      ),
                      Tab(
                        icon: Tooltip(
                          message: 'Fichiers du Workspace',
                          child: Icon(LucideIcons.folderOpen, size: 16),
                        ),
                      ),
                      Tab(
                        icon: Tooltip(
                          message: 'Plan de Travail',
                          child: Icon(LucideIcons.clipboardList, size: 16),
                        ),
                      ),
                      Tab(
                        icon: Tooltip(
                          message: 'Navigateur Web',
                          child: Icon(LucideIcons.globe, size: 16),
                        ),
                      ),
                      Tab(
                        icon: Tooltip(
                          message: 'Sources & Références',
                          child: Icon(LucideIcons.bookOpen, size: 16),
                        ),
                      ),
                    ],
                  ),
                ),
                if (widget.onClose != null) ...[
                  const SizedBox(width: 10),
                  Material(
                    color: Theme.of(
                      context,
                    ).colorScheme.onSurface.withValues(alpha: 0.06),
                    borderRadius: BorderRadius.circular(12),
                    child: InkWell(
                      borderRadius: BorderRadius.circular(12),
                      onTap: () {
                        HapticFeedback.selectionClick();
                        widget.onClose!();
                      },
                      child: const SizedBox(
                        width: 38,
                        height: 38,
                        child: Icon(LucideIcons.x, size: 17),
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
          Expanded(
            child: overview
                ? Builder(builder: hub)
                : loading
                ? const Center(child: CircularProgressIndicator())
                : error != null
                ? Padding(
                    padding: const EdgeInsets.all(24),
                    child: Notice(error!, error: true, retry: load),
                  )
                : TabBarView(
                    children: [
                      outputsView(),
                      ListView(
                        padding: const EdgeInsets.all(20),
                  children: [
                    Row(
                      children: [
                        Expanded(
                          child: TextButton.icon(
                            onPressed: loading ? null : load,
                            icon: const Icon(LucideIcons.refreshCw, size: 14),
                            label: const Text('Actualiser'),
                          ),
                        ),
                        Expanded(
                          child: TextButton(
                            onPressed: () => Navigator.push(
                              context,
                              MaterialPageRoute(
                                builder: (_) => SettingsDetail(
                                  workspace: w,
                                  section: settingsSections.firstWhere(
                                    (s) => s.id == 'agents',
                                  ),
                                ),
                              ),
                            ),
                            child: const Text('Mes Agents'),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 12),
                    Row(
                      children: [
                        runCounter(
                          'ACTIFS',
                          runs
                              .where(
                                (r) =>
                                    ['running', 'active'].contains(r['status']),
                              )
                              .length,
                          const Color(0xff28c840),
                        ),
                        const SizedBox(width: 8),
                        runCounter(
                          'EN FILE',
                          runs
                              .where(
                                (r) =>
                                    ['queued', 'pending'].contains(r['status']),
                              )
                              .length,
                          const Color(0xffff9500),
                        ),
                        const SizedBox(width: 8),
                        runCounter(
                          'FINIS',
                          runs
                              .where(
                                (r) => [
                                  'completed',
                                  'failed',
                                  'cancelled',
                                ].contains(r['status']),
                              )
                              .length,
                          const Color(0xff86868b),
                        ),
                      ],
                    ),
                    const SizedBox(height: 40),
                    if (runs.isEmpty)
                      const EmptyState(
                        icon: LucideIcons.bot,
                        title: 'Aucun sous-agent',
                        subtitle:
                            'Les exécutions de votre espace apparaîtront ici.',
                      ),
                    for (final run in runs)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 12),
                        child: Card(
                          child: Column(
                            children: [
                              ListTile(
                                leading: const Icon(LucideIcons.bot),
                                title: Text(
                                  '${run['goal'] ?? run['title'] ?? 'Tâche'}',
                                ),
                                subtitle: Text('${run['status'] ?? ''}'),
                              ),
                              Wrap(
                                spacing: 8,
                                children: [
                                  for (final action in [
                                    ('pause', 'Pause'),
                                    ('resume', 'Reprendre'),
                                    ('cancel', 'Annuler'),
                                  ])
                                    TextButton(
                                      onPressed: () => perform(context, () async {
                                        await w.api.request(
                                          'POST',
                                          '/agent/runs/${run['id']}/${action.$1}',
                                          body: {},
                                        );
                                        await load();
                                      }),
                                      child: Text(action.$2),
                                    ),
                                ],
                              ),
                            ],
                          ),
                        ),
                      ),
                  ],
                ),
                ListView(
                  padding: const EdgeInsets.all(20),
                  children: [
                    const Notice(
                      'Fichiers de votre espace cloud. Touchez un fichier pour l’apercevoir. Les fichiers du PC restent sur leur appareil.',
                    ),
                    const SizedBox(height: 12),
                    TextField(
                      controller: fileSearch,
                      decoration: const InputDecoration(
                        hintText: 'Rechercher un fichier… (quick open)',
                        prefixIcon: Icon(LucideIcons.search, size: 16),
                        isDense: true,
                      ),
                      onChanged: (_) => setState(() {}),
                    ),
                    const SizedBox(height: 12),
                    if (_visibleFiles().isEmpty && files.isNotEmpty)
                      const EmptyState(
                        icon: LucideIcons.search,
                        title: 'Aucun fichier trouvé',
                        subtitle: 'Modifiez la recherche.',
                      ),
                    if (files.isEmpty)
                      const EmptyState(
                        icon: LucideIcons.fileText,
                        title: 'Vos fichiers, au même endroit',
                        subtitle:
                            'Joignez un document depuis le chat pour le retrouver ici.',
                      ),
                    for (final file in _visibleFiles())
                      Card(
                        child: ListTile(
                          leading: Icon(
                            _fileIcon(
                              '${file['originalName'] ?? file['name'] ?? ''}',
                            ),
                            size: 18,
                          ),
                          title: Text(
                            '${file['originalName'] ?? file['name'] ?? 'Fichier'}',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          subtitle: Text(
                            '${file['status'] ?? ''} · ${file['sizeBytes'] ?? 0} octets',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          trailing: IconButton(
                            tooltip: 'Supprimer le fichier',
                            onPressed: () async {
                              if (await confirmDelete(
                                    context,
                                    '${file['originalName'] ?? 'Fichier'}',
                                  ) &&
                                  context.mounted) {
                                await perform(context, () async {
                                  await w.api.request(
                                    'DELETE',
                                    '/files/${file['id']}',
                                  );
                                  await load();
                                });
                              }
                            },
                            icon: const Icon(LucideIcons.trash2, size: 18),
                          ),
                          onTap: () {
                            HapticFeedback.selectionClick();
                            openFilePreview(
                              context: context,
                              workspace: w,
                              file: file,
                            );
                          },
                        ),
                      ),
                  ],
                ),
                ListView(
                  padding: const EdgeInsets.all(20),
                  children: [
                    Row(
                      children: [
                        Expanded(
                          child: FilledButton.icon(
                            onPressed: createPlan,
                            icon: const Icon(LucideIcons.plus, size: 18),
                            label: const Text('Nouveau plan'),
                          ),
                        ),
                        const SizedBox(width: 10),
                        Expanded(
                          child: OutlinedButton.icon(
                            onPressed: generateAiPlan,
                            icon: const Icon(LucideIcons.sparkles, size: 18),
                            label: const Text('Générer avec l’IA'),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 20),
                    if (plans.isEmpty)
                      const EmptyState(
                        icon: LucideIcons.listTodo,
                        title: 'Aucun plan de projet',
                        subtitle:
                            'Créez un plan ou demandez à l’assistant d’établir une feuille de route.',
                      ),
                    for (final plan in plans)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 16),
                        child: Card(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              ListTile(
                                title: Text(
                                  '${plan['title']}',
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis,
                                ),
                                subtitle: Text(
                                  _planProgressLabel(plan),
                                  style: const TextStyle(fontSize: 11),
                                ),
                                trailing: Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: [
                                    IconButton(
                                      tooltip: 'Ajouter une étape',
                                      onPressed: () => addStep(plan),
                                      icon: const Icon(
                                        LucideIcons.plus,
                                        size: 18,
                                      ),
                                    ),
                                    PopupMenuButton<String>(
                                      tooltip: 'Actions du plan',
                                      onSelected: (v) {
                                        if (v == 'status') {
                                          updatePlanStatus(plan);
                                        } else if (v == 'delete') {
                                          deletePlan(plan);
                                        }
                                      },
                                      itemBuilder: (_) => const [
                                        PopupMenuItem(
                                          value: 'status',
                                          child: Text('Changer le statut'),
                                        ),
                                        PopupMenuItem(
                                          value: 'delete',
                                          child: Text('Supprimer'),
                                        ),
                                      ],
                                    ),
                                  ],
                                ),
                              ),
                              Padding(
                                padding: const EdgeInsets.fromLTRB(
                                  16,
                                  0,
                                  16,
                                  8,
                                ),
                                child: ClipRRect(
                                  borderRadius: BorderRadius.circular(999),
                                  child: LinearProgressIndicator(
                                    value: _planProgressValue(plan),
                                    minHeight: 6,
                                  ),
                                ),
                              ),
                              for (final task in records(plan['tasks']))
                                ListTile(
                                  leading: IconButton(
                                    tooltip:
                                        'Statut : ${taskStatusLabel('${task['status'] ?? (task['completed'] == true ? 'completed' : 'pending')}')} (toucher pour changer)',
                                    onPressed: () => cycleTask(plan, task),
                                    icon: Icon(
                                      taskStatusIcon(
                                        '${task['status'] ?? (task['completed'] == true ? 'completed' : 'pending')}',
                                      ),
                                      size: 20,
                                      color: taskStatusColor(
                                        '${task['status'] ?? (task['completed'] == true ? 'completed' : 'pending')}',
                                      ),
                                    ),
                                  ),
                                  title: Text(
                                    '${task['text']}',
                                    style: (task['status'] == 'completed' ||
                                            task['completed'] == true)
                                        ? const TextStyle(
                                            decoration:
                                                TextDecoration.lineThrough,
                                            color: Color(0xff86868b),
                                          )
                                        : null,
                                  ),
                                  subtitle:
                                      (task['status'] == 'error' &&
                                          '${task['error'] ?? ''}'.isNotEmpty)
                                      ? Text(
                                          '${task['error']}',
                                          style: const TextStyle(
                                            fontSize: 11,
                                            color: Color(0xffff3b30),
                                          ),
                                        )
                                      : Text(
                                          taskStatusLabel(
                                            '${task['status'] ?? (task['completed'] == true ? 'completed' : 'pending')}',
                                          ),
                                          style: const TextStyle(fontSize: 11),
                                        ),
                                  trailing: IconButton(
                                    tooltip: 'Supprimer l’étape',
                                    onPressed: () => deleteStep(plan, task),
                                    icon: const Icon(
                                      LucideIcons.trash2,
                                      size: 16,
                                    ),
                                  ),
                                  onTap: () => cycleTask(plan, task),
                                ),
                            ],
                          ),
                        ),
                      ),
                  ],
                ),
                browserView(),
                sourcesView(),
              ],
            ),
          ),
        ],
      ),
    ),
  );
  Widget runCounter(String label, int count, Color color) => Expanded(
    child: Container(
      padding: const EdgeInsets.symmetric(vertical: 12),
      decoration: BoxDecoration(
        color: color.withValues(alpha: .035),
        border: Border.all(color: color.withValues(alpha: .13)),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        children: [
          Text(
            '$count',
            style: TextStyle(
              fontSize: 18,
              fontWeight: FontWeight.w700,
              color: color,
            ),
          ),
          Text(label, style: TextStyle(fontSize: 9, color: color)),
        ],
      ),
    ),
  );
  Widget hub(BuildContext context) => ListView(
    padding: const EdgeInsets.all(18),
    children: [
      Container(
        padding: const EdgeInsets.all(20),
        decoration: BoxDecoration(
          gradient: const LinearGradient(
            colors: [Color(0xff182638), Color(0xff0071e3)],
          ),
          borderRadius: BorderRadius.circular(14),
        ),
        child: const Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Espace de travail',
              style: TextStyle(
                color: Colors.white,
                fontSize: 16,
                fontWeight: FontWeight.w700,
              ),
            ),
            SizedBox(height: 5),
            Text(
              'Gérez vos livrables, visualisez les sous-agents et parcourez votre code.',
              style: TextStyle(color: Colors.white, fontSize: 12),
            ),
          ],
        ),
      ),
      const SizedBox(height: 16),
      GridView.count(
        shrinkWrap: true,
        physics: const NeverScrollableScrollPhysics(),
        crossAxisCount: 2,
        crossAxisSpacing: 12,
        mainAxisSpacing: 12,
        childAspectRatio: MediaQuery.sizeOf(context).width < 400 ? .9 : 1.25,
        children: [
          hubCard(
            context,
            0,
            LucideIcons.fileText,
            'Sorties & Artefacts',
            'Vos fichiers générés',
            const Color(0xff007aff),
          ),
          hubCard(
            context,
            1,
            LucideIcons.bot,
            'Sous-Agents',
            '${runs.length} exécutions',
            const Color(0xff28c840),
          ),
          hubCard(
            context,
            2,
            LucideIcons.folderOpen,
            'Fichiers du Workspace',
            'Parcourez vos fichiers cloud',
            const Color(0xffff9500),
          ),
          hubCard(
            context,
            3,
            LucideIcons.clipboardList,
            'Plan de Travail',
            'Suivez la feuille de route',
            const Color(0xffbf5af2),
          ),
          hubCard(
            context,
            4,
            LucideIcons.globe,
            'Navigateur Web',
            'Recherches et sites',
            const Color(0xff32c5ff),
          ),
          hubCard(
            context,
            5,
            LucideIcons.bookOpen,
            'Sources & Références',
            'Références des réponses',
            const Color(0xff00b4c8),
          ),
        ],
      ),
    ],
  );
  Widget hubCard(
    BuildContext context,
    int index,
    IconData icon,
    String title,
    String subtitle,
    Color color,
  ) => Card(
    child: InkWell(
      borderRadius: BorderRadius.circular(14),
      onTap: () {
        DefaultTabController.of(context).animateTo(index);
        setState(() => overview = false);
      },
      child: Padding(
        padding: const EdgeInsets.all(10),
        // Cellule de grille a hauteur derivee de la largeur : le contenu
        // textuel DOIT s'ecraser (ellipsis), jamais deborder, y compris
        // quand le clavier reduit la hauteur (panneau monte hors ecran).
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              padding: const EdgeInsets.all(8),
              decoration: BoxDecoration(
                color: color.withValues(alpha: .08),
                borderRadius: BorderRadius.circular(10),
              ),
              child: Icon(icon, size: 18, color: color),
            ),
            const SizedBox(height: 8),
            Text(
              title,
              textAlign: TextAlign.center,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 12, fontWeight: FontWeight.w500),
            ),
            const SizedBox(height: 4),
            Text(
              subtitle,
              textAlign: TextAlign.center,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 10, color: Color(0xff86868b)),
            ),
            const SizedBox(height: 6),
            const Icon(
              LucideIcons.chevronRight,
              size: 12,
              color: Color(0xffb8bbc2),
            ),
          ],
        ),
      ),
    ),
  );
  Widget outputsView() {
    final attachments = [
      for (final message in w.messages)
        if (message['role'] == 'assistant') ...records(message['attachments']),
    ];
    final artifacts = extractCodeArtifacts(w.messages);
    for (final a in artifacts) {
      if (artifactStatus.containsKey(a.id)) a.status = artifactStatus[a.id]!;
    }
    final query = outputSearch.text.trim().toLowerCase();
    final visible = artifacts.where((a) {
      if (outputFilter == 'En attente' && a.status != ArtifactStatus.pending) {
        return false;
      }
      if (outputFilter == 'Enregistrés' && a.status != ArtifactStatus.applied) {
        return false;
      }
      if (outputFilter == 'Rejetés' && a.status != ArtifactStatus.rejected) {
        return false;
      }
      if (query.isNotEmpty &&
          !'${a.title} ${a.language} ${a.code}'.toLowerCase().contains(query)) {
        return false;
      }
      return true;
    }).toList();
    return ListView(
      padding: const EdgeInsets.all(20),
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                'Sorties (${artifacts.length})',
                style: const TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            if (attachments.isNotEmpty)
              Text(
                '${attachments.length} pièce${attachments.length > 1 ? 's' : ''} jointe${attachments.length > 1 ? 's' : ''}',
                style: const TextStyle(fontSize: 11, color: Color(0xff86868b)),
              ),
          ],
        ),
        const SizedBox(height: 12),
        TextField(
          controller: outputSearch,
          decoration: const InputDecoration(
            hintText: 'Rechercher un livrable…',
            prefixIcon: Icon(LucideIcons.search, size: 16),
            isDense: true,
          ),
          onChanged: (_) => setState(() {}),
        ),
        const SizedBox(height: 10),
        Wrap(
          spacing: 8,
          children: [
            for (final f in ['Tous', 'En attente', 'Enregistrés', 'Rejetés'])
              ChoiceChip(
                label: Text(f, style: const TextStyle(fontSize: 12)),
                selected: outputFilter == f,
                onSelected: (_) {
                  HapticFeedback.selectionClick();
                  setState(() => outputFilter = f);
                },
              ),
          ],
        ),
        const SizedBox(height: 12),
        if (attachments.isNotEmpty) ...[
          for (final output in attachments)
            Card(
              child: ListTile(
                leading: const Icon(LucideIcons.paperclip, size: 18),
                title: Text(
                  '${output['displayName'] ?? 'Fichier joint'}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                subtitle: const Text('Pièce jointe de la réponse'),
              ),
            ),
          const SizedBox(height: 8),
        ],
        if (artifacts.isEmpty)
          const EmptyState(
            icon: LucideIcons.fileText,
            title: 'Aucun fichier généré',
            subtitle:
                'Les blocs de code et diffs des réponses apparaîtront ici, avec copie et enregistrement.',
          )
        else if (visible.isEmpty)
          const EmptyState(
            icon: LucideIcons.search,
            title: 'Aucun résultat',
            subtitle: 'Modifiez la recherche ou le filtre.',
          ),
        for (final a in visible)
          Padding(
            padding: const EdgeInsets.only(bottom: 10),
            child: Card(
              child: ListTile(
                leading: Icon(
                  a.isDiff ? LucideIcons.gitCompare : LucideIcons.fileCode,
                  size: 18,
                  color: Theme.of(context).colorScheme.primary,
                ),
                title: Text(
                  a.title,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                ),
                subtitle: Text(
                  a.isDiff
                      ? 'diff · +${a.added} −${a.removed} · ${a.status.name}'
                      : '${a.language} · ${a.code.split('\n').length} lignes · ${a.status.name}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                trailing: const Icon(LucideIcons.chevronRight, size: 16),
                onTap: () {
                  HapticFeedback.selectionClick();
                  openArtifactDetail(
                    context: context,
                    workspace: w,
                    artifact: a,
                    onChanged: () {
                      artifactStatus[a.id] = a.status;
                      if (mounted) setState(() {});
                    },
                  ).then((_) {
                    artifactStatus[a.id] = a.status;
                    if (mounted) setState(() {});
                  });
                },
              ),
            ),
          ),
      ],
    );
  }

  List<Json> _visibleFiles() {
    final q = fileSearch.text.trim().toLowerCase();
    if (q.isEmpty) return files;
    return files
        .where(
          (f) =>
              '${f['originalName'] ?? f['name'] ?? ''}'.toLowerCase().contains(
                q,
              ),
        )
        .toList();
  }

  IconData _fileIcon(String name) {
    final lower = name.toLowerCase();
    if (lower.endsWith('.png') ||
        lower.endsWith('.jpg') ||
        lower.endsWith('.jpeg') ||
        lower.endsWith('.gif') ||
        lower.endsWith('.webp')) {
      return LucideIcons.image;
    }
    if (lower.endsWith('.pdf')) return LucideIcons.fileText;
    if (lower.endsWith('.md')) return LucideIcons.bookOpen;
    if (lower.endsWith('.json') || lower.endsWith('.csv')) {
      return LucideIcons.table;
    }
    if (lower.endsWith('.dart') ||
        lower.endsWith('.ts') ||
        lower.endsWith('.js') ||
        lower.endsWith('.py') ||
        lower.endsWith('.rs')) {
      return LucideIcons.fileCode;
    }
    return LucideIcons.fileText;
  }

  Future<void> _rememberBrowserUrl(String url) async {
    final key = 'aro.browser.history';
    final current = w.preferences.getStringList(key) ?? [];
    final next = [url, ...current.where((u) => u != url)].take(30).toList();
    await w.preferences.setStringList(key, next);
  }

  Widget browserView() => ListView(
    padding: const EdgeInsets.all(20),
    children: [
      const Text(
        'Navigateur Web',
        style: TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
      ),
      const SizedBox(height: 16),
      TextField(
        controller: address,
        keyboardType: TextInputType.url,
        decoration: const InputDecoration(
          hintText: 'https://…',
          prefixIcon: Icon(LucideIcons.globe, size: 16),
        ),
      ),
      const SizedBox(height: 12),
      FilledButton(
        onPressed: () => perform(context, () async {
          final raw = address.text.trim();
          final uri = Uri.tryParse(raw);
          if (uri == null ||
              uri.host.isEmpty ||
              !['https', 'http'].contains(uri.scheme)) {
            throw const ApiException(0, 'Saisissez une adresse HTTP ou HTTPS.');
          }
          if (!await launchUrl(uri, mode: LaunchMode.externalApplication)) {
            throw const ApiException(0, 'Impossible d’ouvrir cette adresse.');
          }
          await _rememberBrowserUrl(raw);
        }),
        child: const Text('Ouvrir dans le navigateur'),
      ),
      const SizedBox(height: 16),
      const Text(
        'Sur mobile, les sites s’ouvrent dans le navigateur de votre appareil, avec historique local dans Réglages › Navigateur.',
        style: TextStyle(fontSize: 12, color: Color(0xff86868b)),
      ),
    ],
  );
  Widget sourcesView() {
    final links = <String, String>{};
    for (final message in w.messages) {
      if (message['role'] != 'assistant') continue;
      for (final match in RegExp(
        r'\[([^\]]+)\]\((https?://[^)\s]+)\)',
      ).allMatches('${message['content'] ?? ''}')) {
        links[match[2]!] = match[1]!;
      }
    }
    return ListView(
      padding: const EdgeInsets.all(20),
      children: [
        if (links.isEmpty)
          const EmptyState(
            icon: LucideIcons.bookOpen,
            title: 'Aucune source active',
            subtitle: 'Les liens cités dans les réponses apparaîtront ici.',
          ),
        for (final entry in links.entries)
          ListTile(
            leading: const Icon(LucideIcons.bookOpen, size: 18),
            title: Text(entry.value),
            subtitle: Text(
              entry.key,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
            onTap: () => perform(context, () async {
              if (!await launchUrl(
                Uri.parse(entry.key),
                mode: LaunchMode.externalApplication,
              )) {
                throw const ApiException(
                  0,
                  'Impossible d’ouvrir cette source.',
                );
              }
            }),
          ),
      ],
    );
  }

  Future<void> createPlan() async {
    final values = await editFields(context, 'Nouveau plan', const [
      SettingField('title', 'Titre', required: true),
      SettingField('description', 'Description', multiline: true),
    ], {});
    if (values == null || !mounted) return;
    await perform(context, () async {
      final conversationId = await w.ensureConversation('${values['title']}');
      await w.api.request(
        'POST',
        '/collections/plans',
        body: {
          ...values,
          'conversationId': conversationId,
          'tasks': [],
          'status': 'active',
        },
      );
      await load();
    });
  }

  Future<void> addStep(Json plan) async {
    final value = await editFields(context, 'Nouvelle étape', const [
      SettingField('text', 'Étape', required: true),
    ], {});
    if (value == null || !mounted) return;
    await perform(context, () async {
      await w.api.request(
        'PATCH',
        '/collections/plans/${plan['id']}',
        body: {
          ...plan,
          'tasks': [
            ...records(plan['tasks']),
            {
              ...value,
              'id': DateTime.now().microsecondsSinceEpoch.toString(),
              'completed': false,
              'status': 'pending',
            },
          ],
        },
      );
      await load();
    });
  }

  double _planProgressValue(Json plan) {
    final p = planProgress(records(plan['tasks']));
    if (p.total == 0) return 0;
    return p.completed / p.total;
  }

  String _planProgressLabel(Json plan) {
    final p = planProgress(records(plan['tasks']));
    final status = '${plan['status'] ?? 'active'}';
    if (p.total == 0) return 'Aucune étape · $status';
    return '${p.completed}/${p.total} tâches · ${p.percentage}% · $status';
  }

  Future<void> cycleTask(Json plan, Json task) async {
    await perform(context, () async {
      final tasks = records(plan['tasks']);
      for (final t in tasks) {
        if (t['id'] == task['id']) {
          final current =
              '${t['status'] ?? (t['completed'] == true ? 'completed' : 'pending')}';
          final next = cycleTaskStatus(current);
          t['status'] = next;
          t['completed'] = next == 'completed';
          if (next != 'error') t['error'] = null;
          if (next == 'error' && '${t['error'] ?? ''}'.isEmpty) {
            t['error'] = 'À vérifier';
          }
        }
      }
      await w.api.request(
        'PATCH',
        '/collections/plans/${plan['id']}',
        body: {...plan, 'tasks': tasks},
      );
      await load();
    });
  }

  Future<void> deleteStep(Json plan, Json task) async {
    if (!await confirmDelete(context, '${task['text'] ?? 'Étape'}')) return;
    if (!mounted) return;
    await perform(context, () async {
      final tasks = records(
        plan['tasks'],
      ).where((t) => t['id'] != task['id']).toList();
      await w.api.request(
        'PATCH',
        '/collections/plans/${plan['id']}',
        body: {...plan, 'tasks': tasks},
      );
      await load();
    });
  }

  Future<void> updatePlanStatus(Json plan) async {
    final current = '${plan['status'] ?? 'active'}';
    final next = await showDialog<String>(
      context: context,
      builder: (_) => SimpleDialog(
        title: const Text('Statut du plan'),
        children: [
          for (final s in ['active', 'completed', 'archived'])
            SimpleDialogOption(
              onPressed: () => Navigator.pop(context, s),
              child: Row(
                children: [
                  if (s == current) const Icon(LucideIcons.check, size: 16),
                  if (s == current) const SizedBox(width: 8),
                  Text(s),
                ],
              ),
            ),
        ],
      ),
    );
    if (next == null || !mounted) return;
    await perform(context, () async {
      await w.api.request(
        'PATCH',
        '/collections/plans/${plan['id']}',
        body: {...plan, 'status': next},
      );
      await load();
    });
  }

  Future<void> deletePlan(Json plan) async {
    if (!await confirmDelete(context, '${plan['title'] ?? 'Plan'}')) return;
    if (!mounted) return;
    await perform(context, () async {
      await w.api.request('DELETE', '/collections/plans/${plan['id']}');
      await load();
    });
  }

  Future<void> generateAiPlan() async {
    final value = await editFields(context, 'Générer avec l’IA', const [
      SettingField('text', 'Objectif', required: true),
    ], {});
    if (value == null || !mounted) return;
    final objective = '${value['text'] ?? ''}'.trim();
    if (objective.isEmpty) return;
    // Ferme le panneau pour montrer la génération dans le chat.
    widget.onClose?.call();
    await w.send(
      'Établis un plan de travail détaillé pour : $objective. '
      'Crée-le avec tes outils de plan afin qu’il apparaisse dans l’onglet Plan.',
    );
    await load();
  }
}
