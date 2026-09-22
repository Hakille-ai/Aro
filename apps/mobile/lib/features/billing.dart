import 'package:flutter/material.dart';
import '../core/api.dart';
import '../core/workspace.dart';

/// Read-only billing overview. Purchases and key creation belong to desktop/web.
class BillingOverview extends StatefulWidget {
  final Workspace workspace;
  const BillingOverview({super.key, required this.workspace});
  @override
  State<BillingOverview> createState() => _BillingOverviewState();
}

class _BillingOverviewState extends State<BillingOverview> {
  Json? snapshot;
  String? error;
  String scope = '';
  int revision = 0;
  bool loading = true;
  @override
  void initState() {
    super.initState();
    widget.workspace.addListener(changed);
    load();
  }
  void changed() {
    if (scope != widget.workspace.api.accountKey) load();
  }
  Future<void> load() async {
    final request = ++revision;
    scope = widget.workspace.api.accountKey;
    final expectedScope = scope;
    setState(() { loading = true; snapshot = null; error = null; });
    try {
      final result = object(await widget.workspace.api.request('GET', '/billing/account'));
      if (!mounted || request != revision || expectedScope != widget.workspace.api.accountKey) return;
      setState(() { snapshot = result; loading = false; });
    } catch (e) {
      if (!mounted || request != revision || expectedScope != widget.workspace.api.accountKey) return;
      setState(() { error = '$e'; loading = false; });
    }
  }
  @override
  void dispose() {
    revision++;
    widget.workspace.removeListener(changed);
    super.dispose();
  }
  String euros(dynamic value) => '${((value as num? ?? 0) / 1000000).toStringAsFixed(2).replaceAll('.', ',')} €';
  @override
  Widget build(BuildContext context) {
    final account = object(snapshot?['account']);
    final rights = object(snapshot?['entitlements']);
    final balance = (account['balanceMicros'] as num? ?? 0);
    final reserved = (account['reservedMicros'] as num? ?? 0);
    return Scaffold(
      appBar: AppBar(title: const Text('Offre & consommation'), actions: [IconButton(tooltip: 'Actualiser', onPressed: loading ? null : load, icon: const Icon(Icons.refresh))]),
      body: loading ? const Center(child: CircularProgressIndicator()) : ListView(padding: const EdgeInsets.all(24), children: [
        if (error != null) Text(error!, style: TextStyle(color: Theme.of(context).colorScheme.error)),
        if (snapshot != null) ...[
          Text('ARO ${account['plan'] ?? 'Community'}', style: Theme.of(context).textTheme.headlineSmall),
          const SizedBox(height: 8),
          Text(rights['managedSync'] == true ? 'Abonnement actif' : 'Aucun abonnement cloud actif'),
          const SizedBox(height: 24),
          ListTile(contentPadding: EdgeInsets.zero, title: const Text('Crédit disponible'), trailing: Text(euros((balance - reserved).clamp(0, double.infinity)))),
          ListTile(contentPadding: EdgeInsets.zero, title: const Text('Calcul en cours / à vérifier'), trailing: Text(euros(reserved))),
          ListTile(contentPadding: EdgeInsets.zero, title: const Text('Consommation du mois'), trailing: Text(euros(account['monthSpentMicros']))),
          ListTile(contentPadding: EdgeInsets.zero, title: const Text('Plafond mensuel'), trailing: Text(euros(account['monthlyLimitMicros']))),
          ListTile(contentPadding: EdgeInsets.zero, title: const Text('Plafond par requête'), trailing: Text(euros(account['perRequestLimitMicros']))),
          const SizedBox(height: 24),
          const Text('Le calcul IA est distinct de votre abonnement. Un plafond à zéro désactive le calcul payant. Les modèles locaux et les clés personnelles ne consomment pas ces crédits.'),
          const SizedBox(height: 16),
          const Text('La gestion de l’abonnement, des budgets et des clés de calcul est disponible dans les paramètres ARO sur ordinateur.'),
        ],
      ]),
    );
  }
}
