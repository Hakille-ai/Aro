import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../core/api.dart';
import '../core/workspace.dart';
import '../ui/design.dart';

/// Centre de notifications in-app — miroir mobile de
/// `NotificationCenter.svelte` : liste serveur, non-lues, tout-marquer-lu,
/// suppression, avec états chargement/vide/erreur et pull-to-refresh.
Future<void> openNotificationCenter({
  required BuildContext context,
  required Workspace workspace,
  required VoidCallback onChanged,
}) {
  return showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    showDragHandle: true,
    builder: (_) => _NotificationCenter(
      workspace: workspace,
      onChanged: onChanged,
    ),
  );
}

class _NotificationCenter extends StatefulWidget {
  final Workspace workspace;
  final VoidCallback onChanged;
  const _NotificationCenter({required this.workspace, required this.onChanged});

  @override
  State<_NotificationCenter> createState() => _NotificationCenterState();
}

class _NotificationCenterState extends State<_NotificationCenter> {
  List<Json> items = [];
  bool loading = true;
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
      items = records(await w.api.request('GET', '/notifications'));
    } catch (e) {
      error = '$e';
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  int get unread =>
      items.where((n) => n['readAt'] == null && n['read'] != true).length;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.78,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      unread > 0
                          ? 'Notifications ($unread non lue${unread > 1 ? 's' : ''})'
                          : 'Notifications',
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                  ),
                  if (items.isNotEmpty)
                    TextButton(
                      onPressed: () => perform(context, () async {
                        await w.api.request(
                          'POST',
                          '/notifications/read-all',
                          body: {},
                        );
                        widget.onChanged();
                        await load();
                      }, success: 'Tout marqué comme lu'),
                      child: const Text('Tout lire'),
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
                          child: Notice(error!, error: true, retry: load),
                        )
                      : items.isEmpty
                          ? const EmptyState(
                              icon: LucideIcons.bell,
                              title: 'Aucune notification',
                              subtitle:
                                  'Les alertes agents, plans et système apparaîtront ici.',
                            )
                          : RefreshIndicator(
                              onRefresh: load,
                              child: ListView.builder(
                                itemCount: items.length,
                                itemBuilder: (_, i) {
                                  final n = items[i];
                                  final read =
                                      n['readAt'] != null || n['read'] == true;
                                  return Dismissible(
                                    key: ValueKey(
                                      '${n['id'] ?? 'notif-$i'}',
                                    ),
                                    direction: DismissDirection.endToStart,
                                    background: Container(
                                      alignment: Alignment.centerRight,
                                      padding: const EdgeInsets.only(right: 20),
                                      color: const Color(0xffff3b30),
                                      child: const Icon(
                                        LucideIcons.trash2,
                                        color: Colors.white,
                                        size: 18,
                                      ),
                                    ),
                                    onDismissed: (_) =>
                                        _delete('${n['id'] ?? ''}'),
                                    child: ListTile(
                                      leading: Icon(
                                        _iconFor('${n['kind'] ?? 'info'}'),
                                        size: 18,
                                        color: read
                                            ? const Color(0xff86868b)
                                            : Theme.of(
                                                context,
                                              ).colorScheme.primary,
                                      ),
                                      title: Text(
                                        '${n['title'] ?? 'Notification'}',
                                        maxLines: 2,
                                        overflow: TextOverflow.ellipsis,
                                        style: TextStyle(
                                          fontWeight: read
                                              ? FontWeight.w400
                                              : FontWeight.w700,
                                        ),
                                      ),
                                      subtitle: Text(
                                        '${n['body'] ?? ''}',
                                        maxLines: 2,
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                      trailing: read
                                          ? null
                                          : Container(
                                              width: 9,
                                              height: 9,
                                              decoration: const BoxDecoration(
                                                color: Color(0xff0071e3),
                                                shape: BoxShape.circle,
                                              ),
                                            ),
                                      onTap: () => _markRead(n),
                                    ),
                                  );
                                },
                              ),
                            ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _markRead(Json n) async {
    final id = '${n['id'] ?? ''}';
    if (id.isEmpty) return;
    HapticFeedback.selectionClick();
    await perform(context, () async {
      await w.api.request('POST', '/notifications/$id/read', body: {});
      widget.onChanged();
      await load();
    });
  }

  Future<void> _delete(String id) async {
    if (id.isEmpty) return;
    await perform(context, () async {
      await w.api.request('DELETE', '/notifications/$id');
      widget.onChanged();
      await load();
    }, success: 'Notification supprimée');
  }

  IconData _iconFor(String kind) => switch (kind.toLowerCase()) {
        'success' => LucideIcons.circleCheck,
        'warning' => LucideIcons.triangleAlert,
        'error' => LucideIcons.circleAlert,
        'agent' => LucideIcons.bot,
        _ => LucideIcons.bell,
      };
}

/// Cloche header avec compteur non-lus — miroir du `NotificationCenter`
/// desktop. Rafraîchit au tap et après fermeture du centre.
class NotificationBell extends StatefulWidget {
  final Workspace workspace;
  const NotificationBell({super.key, required this.workspace});

  @override
  State<NotificationBell> createState() => _NotificationBellState();
}

class _NotificationBellState extends State<NotificationBell> {
  int unread = 0;

  @override
  void initState() {
    super.initState();
    _refresh();
  }

  Future<void> _refresh() async {
    try {
      final value = object(
        await widget.workspace.api.request('GET', '/notifications/unread-count'),
      );
      if (!mounted) return;
      setState(() => unread = (value['unreadCount'] as num? ?? 0).toInt());
    } catch (_) {
      // Compteur indisponible : la cloche reste sans badge, le centre
      // affichera l'erreur détaillée avec réessai.
    }
  }

  @override
  Widget build(BuildContext context) => IconButton(
        tooltip: unread > 0
            ? '$unread notification${unread > 1 ? 's' : ''} non lue${unread > 1 ? 's' : ''}'
            : 'Notifications',
        onPressed: () async {
          HapticFeedback.selectionClick();
          await openNotificationCenter(
            context: context,
            workspace: widget.workspace,
            onChanged: _refresh,
          );
          await _refresh();
        },
        icon: Badge(
          isLabelVisible: unread > 0,
          label: Text('$unread'),
          child: const Icon(LucideIcons.bell, size: 20),
        ),
      );
}
