import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import '../core/api.dart';
import '../core/workspace.dart';
import '../ui/design.dart';
import 'monitoring_chart.dart';
import 'settings_catalog.dart';
import 'profile_editor.dart';
import 'billing.dart';

class SettingsPage extends StatefulWidget {
  final Workspace workspace;
  final String? initialSection;
  const SettingsPage({super.key, required this.workspace, this.initialSection});
  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  String query = '', selected = 'profile';
  @override
  void initState() {
    super.initState();
    selected = widget.initialSection ?? 'profile';
  }

  @override
  Widget build(BuildContext context) {
    final wide = MediaQuery.sizeOf(context).width >= 800;
    final items = settingsSections
        .where(
          (s) => '${s.title} ${s.description} ${s.group}'
              .toLowerCase()
              .contains(query.toLowerCase()),
        )
        .toList();
    final menu = ListView(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 16),
      children: [
        TextField(
          style: const TextStyle(fontSize: 12),
          decoration: const InputDecoration(
            hintText: 'Rechercher dans les réglages',
            isDense: true,
            prefixIcon: Icon(LucideIcons.search, size: 14),
            contentPadding: EdgeInsets.symmetric(horizontal: 10, vertical: 10),
          ),
          onChanged: (v) => setState(() => query = v),
        ),
        for (final group in items.map((s) => s.group).toSet()) ...[
          Padding(
            padding: const EdgeInsets.fromLTRB(10, 26, 0, 8),
            child: Text(
              group.toUpperCase(),
              style: const TextStyle(
                fontSize: 10,
                fontWeight: FontWeight.w700,
                letterSpacing: .2,
                color: Color(0xff86868b),
              ),
            ),
          ),
          for (final item in items.where((s) => s.group == group))
            ListTile(
              selected: wide && selected == item.id,
              selectedColor: Colors.white,
              selectedTileColor: Theme.of(context).colorScheme.primary,
              dense: true,
              minTileHeight: 42,
              horizontalTitleGap: 10,
              contentPadding: const EdgeInsets.symmetric(horizontal: 12),
              leading: Icon(item.icon, size: 16),
              title: Text(item.title, style: const TextStyle(fontSize: 13)),
              trailing: wide
                  ? null
                  : const Icon(LucideIcons.chevronRight, size: 14),
              onTap: () {
                if (wide) {
                  setState(() => selected = item.id);
                } else {
                  Navigator.push(
                    context,
                    MaterialPageRoute(
                      builder: (_) => SettingsDetail(
                        workspace: widget.workspace,
                        section: item,
                      ),
                    ),
                  );
                }
              },
            ),
        ],
        if (items.isEmpty)
          const Padding(
            padding: EdgeInsets.all(24),
            child: Text('Aucun réglage trouvé'),
          ),
      ],
    );
    return Scaffold(
      appBar: AppBar(
        title: const Text('Paramètres'),
        leading: IconButton(
          tooltip: 'Retour',
          icon: const Icon(LucideIcons.arrowLeft, size: 18),
          onPressed: () => Navigator.pop(context),
        ),
      ),
      body: wide
          ? Row(
              children: [
                SizedBox(width: 230, child: menu),
                const VerticalDivider(width: 1),
                Expanded(
                  child: SettingsDetail(
                    key: ValueKey(selected),
                    workspace: widget.workspace,
                    section: settingsSections.firstWhere(
                      (s) => s.id == selected,
                    ),
                  ),
                ),
              ],
            )
          : menu,
    );
  }
}

class SettingsDetail extends StatefulWidget {
  final Workspace workspace;
  final SettingsSection section;
  const SettingsDetail({
    super.key,
    required this.workspace,
    required this.section,
  });
  @override
  State<SettingsDetail> createState() => _SettingsDetailState();
}

class _SettingsDetailState extends State<SettingsDetail> {
  List<Json> items = [];
  List<Json> invitations = [];
  Json draft = {};
  String? error;
  String? invitationsError;
  bool loading = true, saving = false;
  Workspace get w => widget.workspace;
  SettingsSection get s => widget.section;
  @override
  void initState() {
    super.initState();
    load();
  }

  String? get endpoint => s.collection != null
      ? '/collections/${s.collection}'
      : switch (s.id) {
          'organization' => '/memberships',
          'permissions' => '/agent/permission-profiles',
          'plugins' => '/plugins',
          'monitoring' => '/collections/usage-events',
          'paths' => '/devices',
          _ => null,
        };
  Future<void> load() async {
    setState(() {
      loading = true;
      error = null;
      invitationsError = null;
    });
    try {
      draft = w.copySettings();
      if (endpoint != null) {
        items = records(await w.api.request('GET', endpoint!));
      }
      if (s.id == 'organization') {
        try {
          invitations = records(await w.api.request('GET', '/invitations'));
        } catch (e) {
          invitations = [];
          invitationsError = '$e';
        }
      }
    } catch (e) {
      error = '$e';
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  @override
  Widget build(BuildContext context) => s.id == 'billing' ? BillingOverview(workspace: w) : Scaffold(
    backgroundColor: Theme.of(context).colorScheme.surface,
    appBar: AppBar(
      backgroundColor: Theme.of(context).colorScheme.surface,
      title: Text(
        'Paramètres › ${s.title}',
        style: const TextStyle(fontSize: 13),
      ),
      actions: [
        if (s.collection != null)
          IconButton(
            tooltip: 'Ajouter',
            onPressed: saving ? null : () => edit(),
            icon: const Icon(LucideIcons.plus),
          ),
        IconButton(
          tooltip: 'Actualiser',
          onPressed: loading ? null : load,
          icon: const Icon(LucideIcons.refreshCw, size: 20),
        ),
      ],
    ),
    body: loading
        ? const Center(child: CircularProgressIndicator())
        : Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 760),
              child: ListView(
                padding: const EdgeInsets.all(24),
                children: [
                  Text(
                    s.id == 'profile' ? 'Informations du Profil' : s.title,
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                  const SizedBox(height: 8),
                  Text(
                    s.description,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  const SizedBox(height: 28),
                  if (error != null) Notice(error!, error: true, retry: load),
                  if (s.id == 'profile')
                    ProfileEditor(workspace: w)
                  else if (s.collection != null)
                    ...collectionContent()
                  else
                    ...specificContent(),
                  const SizedBox(height: 40),
                ],
              ),
            ),
          ),
  );

  List<Widget> collectionContent() => [
    if (['mcp', 'scheduler', 'hooks', 'agents'].contains(s.id))
      const Padding(
        padding: EdgeInsets.only(bottom: 20),
        child: Notice(
          'Ces configurations appartiennent au serveur. Leur exécution dépend des outils et permissions de cet environnement.',
        ),
      ),
    if (items.isEmpty && error == null)
      EmptyState(
        icon: s.icon,
        title: 'Votre espace est prêt',
        subtitle:
            'Ajoutez votre premier élément dans ${s.title.toLowerCase()}.',
        action: FilledButton.icon(
          onPressed: () => edit(),
          icon: const Icon(LucideIcons.plus, size: 18),
          label: const Text('Ajouter'),
        ),
      ),
    for (final item in items)
      Padding(
        padding: const EdgeInsets.only(bottom: 10),
        child: Card(
          child: ListTile(
            leading: Icon(s.icon, color: Theme.of(context).colorScheme.primary),
            title: Text(
              label(item),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
            subtitle: Text(
              '${item['description'] ?? item['content'] ?? item['prompt'] ?? item['status'] ?? ''}',
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
            onTap: () => edit(item),
            trailing: PopupMenuButton<String>(
              tooltip: 'Actions',
              onSelected: (action) async {
                if (action == 'edit') {
                  edit(item);
                  return;
                }
                if (await confirmDelete(context, label(item)) && mounted) {
                  await perform(context, () async {
                    await w.api.request('DELETE', '$endpoint/${item['id']}');
                    await load();
                  });
                }
              },
              itemBuilder: (_) => const [
                PopupMenuItem(value: 'edit', child: Text('Modifier')),
                PopupMenuItem(value: 'delete', child: Text('Supprimer')),
              ],
            ),
          ),
        ),
      ),
  ];

  String label(Json item) =>
      '${item['name'] ?? item['title'] ?? item['mode'] ?? item['content'] ?? 'Élément'}';
  List<Widget> specificContent() {
    switch (s.id) {
      case 'preferences':
        return [
          Card(
            child: Column(
              children: [
                for (final option in [
                  ('system', 'Système', LucideIcons.monitor),
                  ('light', 'Clair', LucideIcons.sun),
                  ('dark', 'Sombre', LucideIcons.moon),
                  ('oled', 'Noir OLED', LucideIcons.circle),
                ])
                  ListTile(
                    leading: Icon(option.$3),
                    title: Text(option.$2),
                    trailing: w.theme == option.$1
                        ? Icon(
                            LucideIcons.circleCheck,
                            color: Theme.of(context).colorScheme.primary,
                          )
                        : const Icon(LucideIcons.circle),
                    onTap: () async {
                      await w.setTheme(option.$1);
                      if (mounted) setState(() {});
                    },
                  ),
              ],
            ),
          ),
          const SizedBox(height: 20),
          const Notice(
            'La navigation mobile est en français. L’apparence est enregistrée sur cet appareil.',
          ),
        ];
      case 'profile':
        final user = object(w.api.session?['user']);
        return [
          Card(
            child: Column(
              children: [
                ListTile(
                  leading: const Icon(LucideIcons.user),
                  title: Text(w.userName),
                  subtitle: const Text('Nom complet'),
                  trailing: const Icon(LucideIcons.pencil),
                  onTap: editProfile,
                ),
                ListTile(
                  leading: const Icon(LucideIcons.mail),
                  title: Text('${user['email'] ?? ''}'),
                  subtitle: const Text('Adresse e-mail'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 20),
          OutlinedButton.icon(
            onPressed: () => perform(context, () async {
              await w.logout();
              if (mounted) Navigator.of(context).popUntil((r) => r.isFirst);
            }),
            icon: const Icon(LucideIcons.logOut),
            label: const Text('Se déconnecter'),
          ),
        ];
      case 'organization':
        return [
          Card(
            child: ListTile(
              leading: const Icon(LucideIcons.building2),
              title: Text(w.orgName),
              subtitle: const Text('Organisation active'),
              trailing: IconButton(
                tooltip: 'Renommer l’organisation',
                onPressed: renameOrganization,
                icon: const Icon(LucideIcons.pencil, size: 16),
              ),
            ),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: OutlinedButton.icon(
                  onPressed: switchOrganization,
                  icon: const Icon(LucideIcons.arrowLeftRight, size: 16),
                  label: const Text('Changer'),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: OutlinedButton.icon(
                  onPressed: createOrganization,
                  icon: const Icon(LucideIcons.plus, size: 16),
                  label: const Text('Créer'),
                ),
              ),
            ],
          ),
          const SizedBox(height: 20),
          Row(
            children: [
              Expanded(
                child: Text(
                  'Membres (${items.length})',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ),
              TextButton.icon(
                onPressed: inviteMember,
                icon: const Icon(LucideIcons.userPlus, size: 16),
                label: const Text('Inviter'),
              ),
            ],
          ),
          const SizedBox(height: 8),
          if (items.isEmpty && error == null)
            const EmptyState(
              icon: LucideIcons.users,
              title: 'Aucun membre visible',
              subtitle: 'Les membres de votre espace apparaîtront ici.',
            ),
          for (final item in items)
            Card(
              child: ListTile(
                leading: const Icon(LucideIcons.user, size: 18),
                title: Text(
                  '${item['name'] ?? item['email'] ?? item['userId'] ?? 'Membre'}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                subtitle: Text(
                  '${item['email'] ?? ''}${item['email'] != null ? ' · ' : ''}${item['role'] ?? ''}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                trailing: PopupMenuButton<String>(
                  tooltip: 'Gérer ce membre',
                  onSelected: (action) {
                    if (action == 'role') {
                      changeMemberRole(item);
                    } else if (action == 'remove') {
                      removeMember(item);
                    }
                  },
                  itemBuilder: (_) => const [
                    PopupMenuItem(
                      value: 'role',
                      child: Text('Changer le rôle'),
                    ),
                    PopupMenuItem(
                      value: 'remove',
                      child: Text('Retirer'),
                    ),
                  ],
                ),
              ),
            ),
          const SizedBox(height: 20),
          Text(
            'Invitations en attente (${invitations.length})',
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 8),
          if (invitationsError != null)
            Notice(invitationsError!, error: true, retry: load),
          if (invitations.isEmpty && invitationsError == null)
            const Text(
              'Aucune invitation en attente.',
              style: TextStyle(fontSize: 12, color: Color(0xff86868b)),
            ),
          for (final invitation in invitations)
            Card(
              child: ListTile(
                leading: const Icon(LucideIcons.mailPlus, size: 18),
                title: Text(
                  '${invitation['email'] ?? invitation['name'] ?? 'Invitation'}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                subtitle: Text(
                  '${invitation['role'] ?? ''}${invitation['expiresAt'] != null ? ' · expire ${invitation['expiresAt']}' : ''}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                trailing: IconButton(
                  tooltip: 'Révoquer l’invitation',
                  onPressed: () => revokeInvitation(invitation),
                  icon: const Icon(LucideIcons.trash2, size: 16),
                ),
              ),
            ),
        ];
      case 'models':
        final model = object(draft['model']);
        final ai = w.aiStatus;
        final execByProvider = <String, Json>{
          for (final p in records(ai['providers']))
            '${p['providerId']}': p,
        };
        final runnable = <String>{
          for (final m in (ai['runnableModelIds'] as List? ?? [])) '$m',
        };
        String providerSubtitle(Json provider) {
          final exec = execByProvider['${provider['id']}'];
          if (exec == null) {
            return provider['enabled'] == true
                ? 'Activé sur le serveur'
                : 'Désactivé';
          }
          switch ('${exec['reason'] ?? ''}') {
            case 'local-ok':
              return 'Exécuté sur ce téléphone (serveur local)';
            case 'local-unreachable':
              return 'Moteur local injoignable — démarrez-le sur le serveur';
            case 'cloud-ready':
              return 'Cloud — exécuté via le serveur (opt-in actif)';
            case 'cloud-no-consent':
              return 'Cloud — opt-in administrateur requis';
            case 'cloud-no-key':
              return 'Cloud — clé administrateur manquante';
            case 'demo':
              return 'Démo — réponses factices';
            case 'disabled':
              return 'Désactivé';
            default:
              return 'Non exécutable depuis ce téléphone';
          }
        }

        bool modelRunnable(Json ref) {
          if (ai.isEmpty) return true;
          return runnable.contains('${ref['modelId']}');
        }

        return [
          const Notice(
            'Les modèles ci-dessous sont ceux de votre serveur ARO. Seuls les modèles marqués exécutables répondent sur ce téléphone.',
          ),
          if (ai.isNotEmpty && ai['canGenerate'] != true)
            Notice(w.aiGuidanceMessage(), error: true),
          if (ai['mockActive'] == true)
            const Notice(
              'Le modèle actif est une démo : ses réponses sont factices. Choisissez un vrai moteur ci-dessous.',
              error: true,
            ),
          const SizedBox(height: 20),
          for (final provider in records(model['providers']))
            Card(
              child: Column(
                children: [
                  ListTile(
                    leading: const Icon(LucideIcons.cpu),
                    title: Text(
                      '${provider['displayName'] ?? provider['kind']}',
                    ),
                    subtitle: Text(providerSubtitle(provider)),
                  ),
                  for (final ref in records(provider['models']))
                    ListTile(
                      title: Text('${ref['label'] ?? ref['modelId']}'),
                      subtitle: modelRunnable(ref)
                          ? null
                          : const Text(
                              'Non exécuté sur ce téléphone',
                              style: TextStyle(fontSize: 12),
                            ),
                      trailing:
                          object(model['activeModelRef'])['modelId'] ==
                              ref['modelId']
                          ? Icon(
                              LucideIcons.circleCheck,
                              color: Theme.of(context).colorScheme.primary,
                            )
                          : const Icon(LucideIcons.circle),
                      onTap:
                          saving ||
                              provider['enabled'] != true ||
                              !modelRunnable(ref)
                          ? null
                          : () => saveModel(ref),
                    ),
                ],
              ),
            ),
          const SizedBox(height: 20),
          Text('Créativité', style: Theme.of(context).textTheme.titleMedium),
          Slider(
            value: (model['temperature'] as num? ?? .7).toDouble().clamp(0, 2),
            min: 0,
            max: 2,
            divisions: 20,
            label: '${model['temperature'] ?? .7}',
            onChanged: (v) => setState(() => model['temperature'] = v),
            onChangeEnd: (v) {
              draft['model'] = model;
              saveDraft();
            },
          ),
        ];
      case 'search':
        final search = object(draft['search']);
        return [
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                children: [
                  DropdownButtonFormField<String>(
                    initialValue:
                        [
                          'google-scrape',
                          'duckduckgo',
                          'brave',
                        ].contains(search['provider'])
                        ? search['provider']
                        : 'google-scrape',
                    decoration: const InputDecoration(
                      labelText: 'Moteur de recherche',
                    ),
                    items: const [
                      DropdownMenuItem(
                        value: 'google-scrape',
                        child: Text('Google'),
                      ),
                      DropdownMenuItem(
                        value: 'duckduckgo',
                        child: Text('DuckDuckGo'),
                      ),
                      DropdownMenuItem(value: 'brave', child: Text('Brave')),
                    ],
                    onChanged: (v) {
                      search['provider'] = v;
                      draft['search'] = search;
                    },
                  ),
                  const SizedBox(height: 18),
                  const Notice(
                    'Les fournisseurs nécessitant une clé doivent être configurés sur le serveur.',
                  ),
                  const SizedBox(height: 16),
                  FilledButton(
                    onPressed: saving ? null : saveDraft,
                    child: const Text('Enregistrer'),
                  ),
                ],
              ),
            ),
          ),
        ];
      case 'voice':
        return [
          const Notice(
            'La dictée et la lecture utilisent les services vocaux du téléphone. Selon le système, la reconnaissance peut nécessiter Internet.',
          ),
          const SizedBox(height: 20),
          Card(
            child: SwitchListTile(
              title: const Text('Lire les réponses'),
              subtitle: const Text('Lecture vocale après une réponse complète'),
              value: w.preferences.getBool('aro.speak') ?? false,
              onChanged: (v) async {
                await w.preferences.setBool('aro.speak', v);
                if (mounted) setState(() {});
              },
            ),
          ),
          const SizedBox(height: 20),
          const Notice(
            'L’écoute permanente et les exécutables Whisper/Piper du desktop ne sont pas activés sur mobile.',
          ),
        ];
      case 'permissions':
        return [
          const Notice(
            'L’app demande le microphone et les fichiers au moment de leur utilisation. Les droits des autres applications ne sont pas accessibles automatiquement.',
          ),
          const SizedBox(height: 20),
          for (final item in items)
            Card(
              child: ListTile(
                leading: const Icon(LucideIcons.shield),
                title: Text(label(item)),
                subtitle: Text(
                  'Réseau : ${item['allowNetwork'] == true ? 'autorisé' : 'restreint'} · Confirmation : ${item['requireConfirmation'] ?? 'selon politique'}',
                ),
              ),
            ),
          if (items.isEmpty && error == null)
            const EmptyState(
              icon: LucideIcons.shield,
              title: 'Aucun profil disponible',
              subtitle: 'Les profils autorisés apparaîtront ici.',
            ),
        ];
      case 'plugins':
        return [
          for (final item in items)
            Card(
              child: ListTile(
                leading: const Icon(LucideIcons.puzzle),
                title: Text(label(item)),
                subtitle: Text(
                  '${item['description'] ?? item['status'] ?? ''}',
                ),
                trailing: const Icon(LucideIcons.info),
                onTap: () => showDialog(
                  context: context,
                  builder: (context) => AlertDialog(
                    title: Text(label(item)),
                    content: Text(
                      '${item['description'] ?? 'Extension installée sur le serveur.'}',
                    ),
                    actions: [
                      TextButton(
                        onPressed: () => Navigator.pop(context),
                        child: const Text('Fermer'),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          if (items.isEmpty && error == null)
            const EmptyState(
              icon: LucideIcons.puzzle,
              title: 'Aucun plugin installé',
              subtitle:
                  'Les plugins installés dans votre espace apparaîtront ici.',
            ),
        ];
      case 'paths':
        return [
          const Notice(
            'Un chemin de dossier desktop ne désigne pas un dossier du téléphone. Importez les fichiers depuis le chat ; le jumelage d’un exécuteur PC n’est pas encore disponible.',
          ),
          const SizedBox(height: 20),
          for (final item in items)
            Card(
              child: ListTile(
                leading: const Icon(LucideIcons.monitorSmartphone),
                title: Text(label(item)),
                subtitle: Text('${item['platform'] ?? ''}'),
              ),
            ),
          for (final project in w.projects.where((p) => p['rootPath'] != null))
            Card(
              child: ListTile(
                title: Text('${project['name']}'),
                subtitle: Text('${project['rootPath']}'),
              ),
            ),
        ];
      case 'monitoring':
        final summary = usageSummary(items);
        final topModels = summary.byModel.entries.toList()
          ..sort((a, b) => b.value.compareTo(a.value));
        final lastTokens = usageBarsTokens(items).isEmpty
            ? 0
            : usageBarsTokens(items).last;
        return [
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    '${summary.events} événements · ${summary.totalTokens} tokens',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 12),
                  UsageBars(values: usageBarsTokens(items)),
                  const SizedBox(height: 4),
                  const Text(
                    '20 derniers événements',
                    style: TextStyle(fontSize: 10, color: Color(0xff86868b)),
                  ),
                  const SizedBox(height: 12),
                  TokenGauge(tokens: lastTokens),
                ],
              ),
            ),
          ),
          const SizedBox(height: 12),
          if (topModels.isNotEmpty)
            Card(
              child: Column(
                children: [
                  for (final entry in topModels.take(5))
                    ListTile(
                      dense: true,
                      leading: const Icon(LucideIcons.cpu, size: 16),
                      title: Text(
                        entry.key,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      trailing: Text('${entry.value} tokens'),
                    ),
                ],
              ),
            ),
          if (topModels.isNotEmpty) const SizedBox(height: 8),
          for (final item in items.take(100))
            Card(
              child: ListTile(
                title: Text(
                  '${item['modelId'] ?? item['kind'] ?? 'Exécution'}',
                ),
                subtitle: Text('${item['createdAt'] ?? ''}'),
                trailing: Text(
                  '${item['totalTokens'] ?? item['tokenCount'] ?? '—'} tokens',
                ),
              ),
            ),
        ];
      case 'shortcuts':
        return [
          for (final shortcut in [
            ('Ctrl / ⌘ + N', 'Nouvelle conversation'),
            ('Ctrl / ⌘ + K', 'Recherche globale'),
            ('Ctrl / ⌘ + ,', 'Réglages'),
            ('Échap', 'Fermer un panneau'),
          ])
            Card(
              child: ListTile(
                title: Text(shortcut.$2),
                trailing: Text(
                  shortcut.$1,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ),
            ),
          const SizedBox(height: 20),
          const Notice(
            'Sur téléphone, les mêmes actions sont accessibles dans la barre supérieure et le menu latéral.',
          ),
        ];
      case 'notifications':
        final notification = object(draft['notification']);
        bool flag(String key, bool fallback) {
          final value = notification[key];
          return value is bool ? value : fallback;
        }

        void setFlag(String key, bool value) {
          notification[key] = value;
          draft['notification'] = notification;
          setState(() {});
        }

        return [
          const Notice(
            'Les notifications push et les e-mails sont envoyés par votre serveur ARO. Cette page règle les alertes in-app sur ce téléphone ; la configuration SMTP reste sur le desktop.',
          ),
          const SizedBox(height: 20),
          Card(
            child: Column(
              children: [
                SwitchListTile(
                  title: const Text('Notifications in-app'),
                  subtitle: const Text('Toasts pour agents, plans et sync'),
                  value: flag('desktopNotificationsEnabled', true),
                  onChanged: saving
                      ? null
                      : (v) => setFlag('desktopNotificationsEnabled', v),
                ),
                SwitchListTile(
                  title: const Text('Son'),
                  subtitle: const Text('Signal sonore avec les toasts'),
                  value: flag('soundEnabled', true),
                  onChanged: saving
                      ? null
                      : (v) => setFlag('soundEnabled', v),
                ),
                SwitchListTile(
                  title: const Text('Fin des agents'),
                  subtitle: const Text('Me prévenir quand un agent termine'),
                  value: flag('agentCompletionNotifications', true),
                  onChanged: saving
                      ? null
                      : (v) => setFlag('agentCompletionNotifications', v),
                ),
                SwitchListTile(
                  title: const Text('Résumés planifiés'),
                  subtitle: const Text('Alertes des tâches du planificateur'),
                  value: flag('routineNotifications', false),
                  onChanged: saving
                      ? null
                      : (v) => setFlag('routineNotifications', v),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: saving ? null : saveDraft,
            child: const Text('Enregistrer'),
          ),
        ];
      case 'browser':
        final history = w.preferences
            .getStringList('aro.browser.history')
            ?.reversed
            .take(20)
            .toList();
        return [
          const Notice(
            'Sur mobile, les liens s’ouvrent dans le navigateur de votre appareil, avec votre consentement à chaque ouverture. Aucune navigation intégrée silencieuse.',
          ),
          const SizedBox(height: 20),
          Card(
            child: ListTile(
              leading: const Icon(LucideIcons.globe),
              title: const Text('Ouverture externe'),
              subtitle: Text(
                history == null || history.isEmpty
                    ? 'Aucun lien ouvert depuis ARO pour le moment.'
                    : '${history.length} derniers liens ouverts depuis ARO.',
              ),
              trailing: (history == null || history.isEmpty)
                  ? null
                  : IconButton(
                      tooltip: 'Effacer l’historique local',
                      onPressed: () => perform(context, () async {
                        await w.preferences.remove('aro.browser.history');
                        if (mounted) setState(() {});
                      }, success: 'Historique effacé'),
                      icon: const Icon(LucideIcons.trash2, size: 18),
                    ),
            ),
          ),
          if (history != null && history.isNotEmpty) ...[
            const SizedBox(height: 12),
            for (final url in history)
              Card(
                child: ListTile(
                  leading: const Icon(LucideIcons.history, size: 18),
                  title: Text(
                    url,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: const Icon(
                    LucideIcons.externalLink,
                    size: 16,
                  ),
                  onTap: () => perform(context, () async {
                    final uri = Uri.tryParse(url);
                    if (uri == null || uri.host.isEmpty) {
                      throw const ApiException(
                        0,
                        'Ce lien enregistré est invalide.',
                      );
                    }
                    if (!await launchUrl(
                      uri,
                      mode: LaunchMode.externalApplication,
                    )) {
                      throw const ApiException(
                        0,
                        'Impossible d’ouvrir ce lien.',
                      );
                    }
                  }),
                ),
              ),
          ],
        ];
      case 'computer':
        return [
          const Notice(
            'Le contrôle du PC (volume, batterie, Wi-Fi, captures, lancement d’apps, terminal) reste sur l’application desktop, avec autorisation explicite. Cette page décrit uniquement ce téléphone.',
          ),
          const SizedBox(height: 20),
          Card(
            child: Column(
              children: [
                ListTile(
                  leading: const Icon(LucideIcons.monitorSmartphone),
                  title: const Text('Cet appareil'),
                  subtitle: Text(
                    Theme.of(context).platform.name,
                  ),
                ),
                ListTile(
                  leading: const Icon(LucideIcons.server),
                  title: const Text('Serveur ARO'),
                  subtitle: Text(w.api.baseUrl),
                ),
                ListTile(
                  leading: const Icon(LucideIcons.shieldCheck),
                  title: const Text('Permissions natives'),
                  subtitle: const Text(
                    'Microphone, fichiers et notifications demandés au moment de l’usage.',
                  ),
                ),
              ],
            ),
          ),
        ];
      case 'system':
        return [
          Card(
            child: ListTile(
              leading: const Icon(LucideIcons.server),
              title: const Text('Serveur connecté'),
              subtitle: Text(w.api.baseUrl),
            ),
          ),
          const SizedBox(height: 20),
          OutlinedButton.icon(
            onPressed: () => perform(context, () async {
              await w.api.request('GET', '/health', auth: false);
            }, success: 'Le serveur répond'),
            icon: const Icon(LucideIcons.wifi),
            label: const Text('Tester la connexion'),
          ),
          const SizedBox(height: 12),
          OutlinedButton.icon(
            onPressed: () => perform(
              context,
              w.refresh,
              success: 'Synchronisation terminée',
            ),
            icon: const Icon(LucideIcons.refreshCw),
            label: const Text('Synchroniser'),
          ),
          const SizedBox(height: 20),
          const Notice(
            'Pour changer de serveur, déconnectez-vous puis utilisez « Connexion au serveur ». Les opérations de maintenance du PC restent sur le desktop.',
          ),
        ];
      default:
        return [];
    }
  }

  Future<void> saveModel(Json ref) async {
    final model = object(draft['model']);
    model['activeModelRef'] = ref;
    model['provider'] = ref['providerKind'];
    model['modelId'] = ref['modelId'];
    draft['model'] = model;
    await saveDraft();
    // Le défaut global change : toute surcharge par message devient caduque.
    w.setModelOverride(null);
    await w.refreshAiStatus();
    if (mounted) setState(() {});
  }

  Future<void> saveDraft() async {
    setState(() => saving = true);
    await perform(
      context,
      () => w.saveSettings(draft),
      success: 'Réglages enregistrés',
    );
    if (mounted) setState(() => saving = false);
  }

  Future<void> editProfile() async {
    final result = await editFields(
      context,
      'Votre profil',
      [const SettingField('name', 'Nom complet', required: true)],
      {'name': w.userName},
    );
    if (result == null || !mounted) return;
    await perform(context, () async {
      final user = object(
        await w.api.request('PATCH', '/users/me', body: result),
      );
      await w.api.saveSession({...w.api.session!, 'user': user});
      if (mounted) setState(() {});
    }, success: 'Profil enregistré');
  }

  Future<void> switchOrganization() async {
    await perform(context, () async {
      final orgs = records(await w.api.request('GET', '/organizations'));
      if (!mounted) return;
      final id = await showDialog<String>(
        context: context,
        builder: (context) => SimpleDialog(
          title: const Text('Organisation'),
          children: [
            for (final org in orgs)
              SimpleDialogOption(
                onPressed: () => Navigator.pop(context, org['id']),
                child: Text('${org['name']}'),
              ),
          ],
        ),
      );
      if (id == null) return;
      await w.switchOrganization(id);
      if (mounted) setState(() {});
    });
  }

  Future<void> inviteMember() async {
    final result = await editFields(context, 'Inviter un membre', const [
      SettingField('name', 'Nom complet', required: true),
      SettingField('email', 'Adresse e-mail', required: true),
      SettingField('role', 'Rôle', options: ['member', 'admin']),
    ], const {'role': 'member'});
    if (result == null || !mounted) return;
    await perform(context, () async {
      try {
        await w.api.request('POST', '/memberships', body: result);
      } on ApiException catch (e) {
        if (e.status == 503) {
          throw const ApiException(
            503,
            'L’envoi d’invitations n’est pas configuré sur ce serveur.',
          );
        }
        rethrow;
      }
      await load();
    }, success: 'Invitation envoyée');
  }

  Future<void> changeMemberRole(Json member) async {
    final current = '${member['role'] ?? 'member'}';
    final next = await showDialog<String>(
      context: context,
      builder: (_) => SimpleDialog(
        title: const Text('Rôle du membre'),
        children: [
          for (final role in ['member', 'admin'])
            SimpleDialogOption(
              onPressed: () => Navigator.pop(context, role),
              child: Row(
                children: [
                  if (role == current) const Icon(LucideIcons.check, size: 16),
                  if (role == current) const SizedBox(width: 8),
                  Text(role == 'admin' ? 'Admin' : 'Membre'),
                ],
              ),
            ),
        ],
      ),
    );
    if (next == null || next == current || !mounted) return;
    await perform(context, () async {
      await w.api.request(
        'PATCH',
        '/memberships/${member['id']}',
        body: {'role': next},
      );
      await load();
    }, success: 'Rôle mis à jour');
  }

  Future<void> removeMember(Json member) async {
    final label =
        '${member['name'] ?? member['email'] ?? member['userId'] ?? 'Membre'}';
    if (!await confirmDelete(context, label)) return;
    if (!mounted) return;
    await perform(context, () async {
      await w.api.request('DELETE', '/memberships/${member['id']}');
      await load();
    }, success: 'Membre retiré');
  }

  Future<void> revokeInvitation(Json invitation) async {
    final label =
        '${invitation['email'] ?? invitation['name'] ?? 'Invitation'}';
    if (!await confirmDelete(context, label)) return;
    if (!mounted) return;
    await perform(context, () async {
      await w.api.request('DELETE', '/invitations/${invitation['id']}');
      await load();
    }, success: 'Invitation révoquée');
  }

  Future<void> renameOrganization() async {
    final result = await editFields(
      context,
      'Renommer l’organisation',
      const [SettingField('name', 'Nom', required: true)],
      {'name': w.orgName},
    );
    if (result == null || !mounted) return;
    await perform(context, () async {
      final orgId = object(w.api.session?['activeOrganization'])['id'];
      await w.api.request(
        'PATCH',
        '/organizations/$orgId',
        body: {'name': result['name']},
      );
      await w.refresh();
      if (mounted) setState(() {});
    }, success: 'Organisation renommée');
  }

  Future<void> createOrganization() async {
    final result = await editFields(
      context,
      'Créer une organisation',
      const [SettingField('name', 'Nom', required: true)],
      const {},
    );
    if (result == null || !mounted) return;
    await perform(context, () async {
      final created = object(
        await w.api.request('POST', '/organizations', body: result),
      );
      final org = object(created['organization']);
      if (org['id'] != null) {
        await w.switchOrganization('${org['id']}');
      }
      await load();
    }, success: 'Organisation créée');
  }

  Future<void> edit([Json? item]) async {
    final result = await editFields(
      context,
      item == null ? 'Ajouter · ${s.title}' : 'Modifier',
      s.fields,
      {...s.defaults, ...?item},
    );
    if (result == null || !mounted) return;
    if (s.id == 'hooks') {
      result['events'] = '${result['events'] ?? ''}'
          .split(',')
          .map((v) => v.trim())
          .where((v) => v.isNotEmpty)
          .toList();
    }
    await perform(context, () async {
      await w.api.request(
        item == null ? 'POST' : 'PATCH',
        item == null ? endpoint! : '$endpoint/${item['id']}',
        body: result,
      );
      await load();
    }, success: 'Enregistré');
  }
}

Future<Json?> editFields(
  BuildContext context,
  String title,
  List<SettingField> fields,
  Json initial,
) => showDialog<Json>(
  context: context,
  builder: (_) => _Editor(title: title, fields: fields, initial: initial),
);

class _Editor extends StatefulWidget {
  final String title;
  final List<SettingField> fields;
  final Json initial;
  const _Editor({
    required this.title,
    required this.fields,
    required this.initial,
  });
  @override
  State<_Editor> createState() => _EditorState();
}

class _EditorState extends State<_Editor> {
  final form = GlobalKey<FormState>();
  late final Map<String, TextEditingController> controls;
  late bool enabled, pinned;
  @override
  void initState() {
    super.initState();
    controls = {
      for (final f in widget.fields)
        f.key: TextEditingController(
          text: widget.initial[f.key] is List
              ? (widget.initial[f.key] as List).join(', ')
              : '${widget.initial[f.key] ?? f.options?.first ?? ''}',
        ),
    };
    enabled = widget.initial['enabled'] == true;
    pinned = widget.initial['pinned'] == true;
  }

  @override
  void dispose() {
    for (final c in controls.values) {
      c.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: Text(widget.title),
    content: SizedBox(
      width: 480,
      child: SingleChildScrollView(
        child: Form(
          key: form,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final f in widget.fields)
                Padding(
                  padding: const EdgeInsets.only(bottom: 16),
                  child: f.options != null
                      ? DropdownButtonFormField<String>(
                          initialValue:
                              f.options!.contains(controls[f.key]!.text)
                              ? controls[f.key]!.text
                              : f.options!.first,
                          decoration: InputDecoration(labelText: f.label),
                          items: [
                            for (final o in f.options!)
                              DropdownMenuItem(value: o, child: Text(o)),
                          ],
                          onChanged: (v) => controls[f.key]!.text = v!,
                        )
                      : TextFormField(
                          controller: controls[f.key],
                          minLines: f.multiline ? 3 : 1,
                          maxLines: f.multiline ? 7 : 1,
                          decoration: InputDecoration(labelText: f.label),
                          validator: (v) {
                            if (f.required && (v?.trim().isEmpty ?? true)) {
                              return 'Ce champ est requis.';
                            }
                            if (f.key == 'url' &&
                                v!.isNotEmpty &&
                                !(Uri.tryParse(v)?.hasAuthority ?? false)) {
                              return 'Adresse invalide.';
                            }
                            return null;
                          },
                        ),
                ),
              if (widget.initial.containsKey('enabled'))
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Activer'),
                  value: enabled,
                  onChanged: (value) => setState(() => enabled = value),
                ),
              if (widget.initial.containsKey('pinned'))
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Épingler'),
                  value: pinned,
                  onChanged: (value) => setState(() => pinned = value),
                ),
            ],
          ),
        ),
      ),
    ),
    actions: [
      TextButton(
        onPressed: () => Navigator.pop(context),
        child: const Text('Annuler'),
      ),
      FilledButton(
        onPressed: () {
          if (form.currentState!.validate()) {
            Navigator.pop(context, {
              ...widget.initial,
              if (widget.initial.containsKey('enabled')) 'enabled': enabled,
              if (widget.initial.containsKey('pinned')) 'pinned': pinned,
              for (final f in widget.fields)
                f.key: controls[f.key]!.text.trim(),
            });
          }
        },
        child: const Text('Enregistrer'),
      ),
    ],
  );
}
