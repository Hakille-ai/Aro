import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import '../core/api.dart';
import '../core/workspace.dart';
import '../ui/design.dart';
import 'mfa.dart';

class ProfileEditor extends StatefulWidget {
  final Workspace workspace;
  const ProfileEditor({super.key, required this.workspace});
  @override
  State<ProfileEditor> createState() => _ProfileEditorState();
}

class _ProfileEditorState extends State<ProfileEditor> {
  final form = GlobalKey<FormState>();
  late final TextEditingController name, role;
  final keyName = TextEditingController();
  List<Json> keys = [];
  String? error;
  bool saving = false;
  late String avatar;
  Workspace get w => widget.workspace;
  static const colors = [
    0xff00a8ed,
    0xffdf3caa,
    0xff08c7b2,
    0xffffb000,
    0xff292929,
    0xff2185eb,
  ];
  @override
  void initState() {
    super.initState();
    final user = object(w.api.session?['user']);
    name = TextEditingController(text: w.userName);
    role = TextEditingController(text: '${user['roleTitle'] ?? ''}');
    avatar = '${user['avatarColor'] ?? '#2185eb'}';
    loadKeys();
  }

  Future<void> loadKeys() async {
    try {
      final result = records(await w.api.request('GET', '/api-keys'));
      if (mounted) {
        setState(() {
          keys = result;
          error = null;
        });
      }
    } catch (e) {
      if (mounted) setState(() => error = '$e');
    }
  }

  @override
  void dispose() {
    name.dispose();
    role.dispose();
    keyName.dispose();
    super.dispose();
  }

  Color get avatarColor {
    final hex = RegExp(r'#[a-fA-F0-9]{6}').firstMatch(avatar)?[0]?.substring(1);
    return Color(int.tryParse('ff${hex ?? '2185eb'}', radix: 16) ?? 0xff2185eb);
  }

  Widget field(String label, Widget input) => Padding(
    padding: const EdgeInsets.only(bottom: 18),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: const TextStyle(fontSize: 10, fontWeight: FontWeight.w500),
        ),
        const SizedBox(height: 7),
        input,
      ],
    ),
  );
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Card(
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Form(
            key: form,
            child: Column(
              children: [
                Row(
                  children: [
                    CircleAvatar(
                      radius: 28,
                      backgroundColor: avatarColor,
                      child: Text(
                        name.text.trim().isEmpty
                            ? 'A'
                            : name.text
                                  .trim()
                                  .split(RegExp(r'\s+'))
                                  .take(2)
                                  .map((p) => p.characters.first)
                                  .join()
                                  .toUpperCase(),
                        style: const TextStyle(
                          color: Colors.white,
                          fontSize: 22,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ),
                    const SizedBox(width: 16),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            w.userName,
                            style: const TextStyle(
                              fontSize: 17,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                          const SizedBox(height: 4),
                          Text(
                            '${object(w.api.session?['user'])['email'] ?? ''}',
                            style: Theme.of(context).textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
                const Padding(
                  padding: EdgeInsets.symmetric(vertical: 20),
                  child: Divider(),
                ),
                Container(
                  padding: const EdgeInsets.all(16),
                  decoration: BoxDecoration(
                    color: Theme.of(context).scaffoldBackgroundColor,
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      field(
                        'NOM COMPLET',
                        TextFormField(
                          controller: name,
                          decoration: const InputDecoration(isDense: true),
                          validator: (v) => v == null || v.trim().isEmpty
                              ? 'Saisissez votre nom.'
                              : null,
                        ),
                      ),
                      field(
                        'ADRESSE E-MAIL',
                        TextFormField(
                          initialValue:
                              '${object(w.api.session?['user'])['email'] ?? ''}',
                          readOnly: true,
                          decoration: const InputDecoration(isDense: true),
                        ),
                      ),
                      field(
                        'RÔLE / TITRE',
                        TextFormField(
                          controller: role,
                          decoration: const InputDecoration(isDense: true),
                        ),
                      ),
                      const Text(
                        "COULEUR DE L’AVATAR",
                        style: TextStyle(fontSize: 10),
                      ),
                      const SizedBox(height: 10),
                      Wrap(
                        spacing: 8,
                        runSpacing: 8,
                        children: [
                          for (final value in colors)
                            Semantics(
                              label: 'Couleur ${colors.indexOf(value) + 1}',
                              selected: avatarColor == Color(value),
                              button: true,
                              child: InkWell(
                                customBorder: const CircleBorder(),
                                onTap: () => setState(
                                  () => avatar =
                                      '#${value.toRadixString(16).substring(2)}',
                                ),
                                child: Container(
                                  width: 32,
                                  height: 32,
                                  decoration: BoxDecoration(
                                    color: Color(value),
                                    shape: BoxShape.circle,
                                    border: Border.all(
                                      color: avatarColor == Color(value)
                                          ? Theme.of(
                                              context,
                                            ).colorScheme.primary
                                          : Colors.transparent,
                                      width: 2,
                                    ),
                                  ),
                                ),
                              ),
                            ),
                        ],
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 16),
                Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton(
                    onPressed: saving ? null : save,
                    child: const Text('Enregistrer'),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
      const SizedBox(height: 28),
      Card(
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const Text(
                "Clés d’API Personnelles",
                style: TextStyle(fontSize: 14, fontWeight: FontWeight.w600),
              ),
              const SizedBox(height: 8),
              const Text(
                "Clés d’API pour intégrer ARO dans vos outils de développement.",
                style: TextStyle(fontSize: 12, color: Color(0xff86868b)),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: keyName,
                decoration: const InputDecoration(
                  hintText: 'ex: Clé Production, SDK Local…',
                  isDense: true,
                ),
              ),
              const SizedBox(height: 10),
              OutlinedButton.icon(
                onPressed: saving ? null : createKey,
                icon: const Icon(LucideIcons.plus, size: 15),
                label: const Text('Générer une clé'),
              ),
              if (error != null) Notice(error!, error: true, retry: loadKeys),
              for (final key in keys)
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  dense: true,
                  title: Text('${key['name']}'),
                  subtitle: Text('${key['prefix'] ?? ''}…'),
                  trailing: IconButton(
                    tooltip: 'Révoquer la clé',
                    icon: const Icon(LucideIcons.trash2, size: 16),
                    onPressed: () => revoke(key),
                  ),
                ),
            ],
          ),
        ),
      ),
      const SizedBox(height: 28),
      MfaCard(workspace: w),
      const SizedBox(height: 24),
      OutlinedButton.icon(
        onPressed: () => perform(context, () async {
          await w.logout();
        }),
        icon: const Icon(LucideIcons.logOut, size: 16),
        label: const Text('Se déconnecter'),
      ),
    ],
  );
  Future<void> save() async {
    if (!form.currentState!.validate()) return;
    setState(() => saving = true);
    await perform(context, () async {
      final user = object(
        await w.api.request(
          'PATCH',
          '/users/me',
          body: {
            'name': name.text.trim(),
            'roleTitle': role.text.trim(),
            'avatarColor': avatar,
          },
        ),
      );
      await w.api.saveSession({...w.api.session!, 'user': user});
    }, success: 'Profil enregistré');
    if (mounted) setState(() => saving = false);
  }

  Future<void> createKey() async {
    if (keyName.text.trim().isEmpty) {
      setState(() => error = 'Donnez un nom à la clé.');
      return;
    }
    setState(() => saving = true);
    await perform(context, () async {
      final result = object(
        await w.api.request(
          'POST',
          '/api-keys',
          body: {'name': keyName.text.trim()},
        ),
      );
      if (!mounted) return;
      await showDialog<void>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('Votre clé personnelle'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Copiez cette clé maintenant : elle ne sera plus affichée.',
              ),
              const SizedBox(height: 16),
              SelectableText('${result['secret']}'),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () =>
                  Clipboard.setData(ClipboardData(text: '${result['secret']}')),
              child: const Text('Copier'),
            ),
            TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Fermer'),
            ),
          ],
        ),
      );
      keyName.clear();
      await loadKeys();
    });
    if (mounted) setState(() => saving = false);
  }

  Future<void> revoke(Json key) async {
    if (await confirmDelete(context, '${key['name']}') && mounted) {
      await perform(context, () async {
        await w.api.request('DELETE', "/api-keys/${key['id']}");
        await loadKeys();
      });
    }
  }
}
