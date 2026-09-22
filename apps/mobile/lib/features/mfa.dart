import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../core/api.dart';
import '../core/workspace.dart';
import '../ui/design.dart';

/// Second facteur TOTP — enrollment vérifiée côté serveur
/// (`POST /auth/mfa/totp/setup|enable|disable`, RFC 6238).
///
/// Honnêteté : la route `/auth/login` actuelle du serveur ne demande pas
/// encore le code à la connexion. Cette carte prépare l'enrôlement vérifié
/// (preuve de possession exigée à l'activation comme à la désactivation)
/// sans prétendre protéger la connexion.
class MfaCard extends StatefulWidget {
  final Workspace workspace;
  const MfaCard({super.key, required this.workspace});

  @override
  State<MfaCard> createState() => _MfaCardState();
}

class _MfaCardState extends State<MfaCard> {
  String? secret;
  String? otpauthUrl;
  final code = TextEditingController();
  bool busy = false;

  Workspace get w => widget.workspace;

  @override
  void dispose() {
    code.dispose();
    super.dispose();
  }

  bool get validCode => RegExp(r'^\d{6}$').hasMatch(code.text.trim());

  Future<void> _setup() async {
    setState(() => busy = true);
    await perform(context, () async {
      final result = Map<String, dynamic>.from(
        await w.api.request('POST', '/auth/mfa/totp/setup', body: {}) as Map,
      );
      secret = '${result['secret'] ?? ''}';
      otpauthUrl = '${result['otpauthUrl'] ?? ''}';
      if (secret!.isEmpty) throw const ApiException(502, 'Secret MFA vide.');
    });
    if (mounted) setState(() => busy = false);
  }

  Future<void> _enable() async {
    if (!validCode) return;
    setState(() => busy = true);
    await perform(context, () async {
      await w.api.request(
        'POST',
        '/auth/mfa/totp/enable',
        body: {'code': code.text.trim()},
      );
      secret = null;
      code.clear();
    }, success: 'Second facteur activé ✓');
    if (mounted) setState(() => busy = false);
  }

  Future<void> _disable() async {
    if (!validCode) return;
    setState(() => busy = true);
    await perform(context, () async {
      await w.api.request(
        'POST',
        '/auth/mfa/totp/disable',
        body: {'code': code.text.trim()},
      );
      secret = null;
      code.clear();
    }, success: 'Second facteur désactivé');
    if (mounted) setState(() => busy = false);
  }

  @override
  Widget build(BuildContext context) => Card(
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const Row(
                children: [
                  Icon(LucideIcons.shieldCheck, size: 18),
                  SizedBox(width: 10),
                  Flexible(
                    child: Text(
                      'Second facteur (TOTP)',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              const Text(
                'Compatible Google Authenticator, 1Password, etc.',
                style: TextStyle(fontSize: 12, color: Color(0xff86868b)),
              ),
              const SizedBox(height: 12),
              const Notice(
                'Le serveur vérifie le code à l’activation et à la désactivation (RFC 6238). La connexion par mot de passe ne demande pas encore ce code côté serveur : l’enrôlement reste préparatoire.',
              ),
              const SizedBox(height: 16),
              if (secret == null)
                OutlinedButton.icon(
                  onPressed: busy ? null : _setup,
                  icon: const Icon(LucideIcons.plus, size: 15),
                  label: const Text('Configurer un nouveau secret'),
                )
              else ...[
                const Text(
                  'SECRET (à saisir dans votre application d’authentification)',
                  style: TextStyle(fontSize: 10, fontWeight: FontWeight.w500),
                ),
                const SizedBox(height: 7),
                Row(
                  children: [
                    Expanded(
                      child: SelectableText(
                        secret!,
                        style: const TextStyle(
                          fontFamily: 'monospace',
                          fontSize: 13,
                        ),
                      ),
                    ),
                    IconButton(
                      tooltip: 'Copier le secret',
                      onPressed: () async {
                        await Clipboard.setData(ClipboardData(text: secret!));
                        if (!context.mounted) return;
                        HapticFeedback.selectionClick();
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(content: Text('Secret copié')),
                        );
                      },
                      icon: const Icon(LucideIcons.copy, size: 16),
                    ),
                  ],
                ),
                if ((otpauthUrl ?? '').isNotEmpty) ...[
                  const SizedBox(height: 8),
                  SelectableText(
                    otpauthUrl!,
                    style: const TextStyle(fontSize: 10, color: Color(0xff86868b)),
                  ),
                ],
                const SizedBox(height: 12),
              ],
              TextField(
                controller: code,
                keyboardType: TextInputType.number,
                maxLength: 6,
                decoration: const InputDecoration(
                  hintText: 'Code à 6 chiffres',
                  counterText: '',
                  isDense: true,
                  prefixIcon: Icon(LucideIcons.keyRound, size: 16),
                ),
                onChanged: (_) => setState(() {}),
              ),
              const SizedBox(height: 10),
              Row(
                children: [
                  Expanded(
                    child: FilledButton(
                      onPressed: busy || !validCode ? null : _enable,
                      child: const Text('Activer'),
                    ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: OutlinedButton(
                      onPressed: busy || !validCode ? null : _disable,
                      child: const Text('Désactiver'),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      );
}
