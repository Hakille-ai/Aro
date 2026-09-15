import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'dart:ui';

import 'package:flutter/material.dart';

import '../core/api.dart';
import '../core/workspace.dart';
import '../ui/design.dart';

class AuthPage extends StatefulWidget {
  final Workspace workspace;
  const AuthPage({super.key, required this.workspace});
  @override
  State<AuthPage> createState() => _AuthPageState();
}

class _AuthPageState extends State<AuthPage> {
  final _form = GlobalKey<FormState>();
  final email = TextEditingController(),
      password = TextEditingController(),
      name = TextEditingController(),
      org = TextEditingController(),
      token = TextEditingController();
  String mode = 'login';
  bool busy = false, visible = false;
  String? error, success;
  @override
  void initState() {
    super.initState();
    final params = Uri.base.queryParameters;
    if (params['reset_token'] != null) {
      mode = 'reset-password';
      token.text = params['reset_token']!;
    }
    if (params['invitation_token'] != null) {
      mode = 'invitation';
      token.text = params['invitation_token']!;
    }
  }

  @override
  void dispose() {
    for (final c in [email, password, name, org, token]) {
      c.dispose();
    }
    super.dispose();
  }

  void switchMode(String value) => setState(() {
    mode = value;
    error = null;
    success = null;
  });

  Future<void> submit() async {
    if (!_form.currentState!.validate()) return;
    FocusManager.instance.primaryFocus?.unfocus();
    setState(() {
      busy = true;
      error = null;
      success = null;
    });
    try {
      final w = widget.workspace;
      if (mode == 'forgot-password') {
        await w.api.request(
          'POST',
          '/auth/password-reset/request',
          auth: false,
          body: {'email': email.text.trim()},
        );
        success =
            'Si un compte correspond à cette adresse, un lien vous sera envoyé.';
      } else if (mode == 'reset-password') {
        await w.api.request(
          'POST',
          '/auth/password-reset/confirm',
          auth: false,
          body: {'token': token.text.trim(), 'newPassword': password.text},
        );
        mode = 'login';
        success = 'Mot de passe mis à jour. Vous pouvez vous connecter.';
      } else {
        await w.authenticate(
          mode == 'invitation' ? 'invitations/accept-account' : mode,
          {
            'email': email.text.trim(),
            'password': password.text,
            if (mode == 'register') 'name': name.text.trim(),
            if (mode == 'register')
              'organizationName': org.text.trim().isEmpty
                  ? 'Espace de ${name.text.trim()}'
                  : org.text.trim(),
            if (mode == 'invitation') 'token': token.text.trim(),
          },
        );
      }
    } catch (e) {
      error = '$e';
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> configureServer() async {
    final controller = TextEditingController(
      text: widget.workspace.api.baseUrl,
    );
    final result = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Serveur ARO'),
        content: TextField(
          controller: controller,
          keyboardType: TextInputType.url,
          decoration: const InputDecoration(
            labelText: 'Adresse HTTPS du serveur',
            hintText: 'https://aro.example.com',
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Annuler'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, controller.text),
            child: const Text('Enregistrer'),
          ),
        ],
      ),
    );
    controller.dispose();
    if (result == null || !mounted) return;
    await perform(context, () async {
      final url = AroApi.validateServer(result);
      await widget.workspace.api.clearSession();
      widget.workspace.api.baseUrl = url;
      await widget.workspace.preferences.setString('aro.server', url);
      if (mounted) setState(() {});
    }, success: 'Serveur enregistré');
  }

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    final title = switch (mode) {
      'register' => 'Créer votre espace',
      'forgot-password' => 'Mot de passe oublié',
      'reset-password' => 'Nouveau mot de passe',
      'invitation' => 'Accepter l’invitation',
      _ => 'Connexion',
    };
    return Scaffold(
      body: Stack(
        children: [
          Positioned.fill(
            child: ColoredBox(
              color: dark ? Colors.black : const Color(0xfff5f5f7),
            ),
          ),
          Positioned(
            left: -160,
            top: -120,
            child: _glow(const Color(0xff0071e3), 540),
          ),
          Positioned(
            right: -210,
            bottom: -110,
            child: _glow(const Color(0xff30d158), 600),
          ),
          Positioned(
            right: -180,
            top: 260,
            child: _glow(const Color(0xffff453a), 400),
          ),
          SafeArea(
            child: Center(
              child: SingleChildScrollView(
                padding: const EdgeInsets.symmetric(
                  horizontal: 24,
                  vertical: 40,
                ),
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 440),
                  child: Column(
                    children: [
                      ClipRRect(
                        borderRadius: BorderRadius.circular(24),
                        child: BackdropFilter(
                          filter: ImageFilter.blur(sigmaX: 35, sigmaY: 35),
                          child: Container(
                            padding: const EdgeInsets.all(28),
                            decoration: BoxDecoration(
                              color:
                                  (dark
                                          ? const Color(0xff1c1c1e)
                                          : Colors.white)
                                      .withValues(alpha: .55),
                              borderRadius: BorderRadius.circular(24),
                              border: Border.all(
                                color: Colors.white.withValues(
                                  alpha: dark ? .08 : .5,
                                ),
                              ),
                            ),
                            child: AutofillGroup(
                              child: Form(
                                key: _form,
                                child: Column(
                                  crossAxisAlignment:
                                      CrossAxisAlignment.stretch,
                                  children: [
                                    const Align(
                                      alignment: Alignment.centerLeft,
                                      child: Brand(size: 50),
                                    ),
                                    const SizedBox(height: 30),
                                    Text(
                                      title,
                                      textAlign: TextAlign.left,
                                      style: Theme.of(context)
                                          .textTheme
                                          .headlineLarge
                                          ?.copyWith(
                                            fontWeight: FontWeight.w800,
                                          ),
                                    ),
                                    const SizedBox(height: 8),
                                    Text(
                                      mode == 'forgot-password'
                                          ? 'Recevez un lien de réinitialisation sécurisé.'
                                          : 'Accédez à votre espace de travail',
                                      textAlign: TextAlign.left,
                                      style: Theme.of(
                                        context,
                                      ).textTheme.bodySmall,
                                    ),
                                    const SizedBox(height: 28),
                                    if (mode == 'register') ...[
                                      _field(
                                        name,
                                        'Nom complet',
                                        LucideIcons.user,
                                        hints: const [AutofillHints.name],
                                      ),
                                      const SizedBox(height: 14),
                                      _field(
                                        org,
                                        'Nom de l’organisation',
                                        LucideIcons.building2,
                                        required: false,
                                      ),
                                      const SizedBox(height: 14),
                                    ],
                                    if (mode != 'reset-password') ...[
                                      _field(
                                        email,
                                        'Adresse e-mail',
                                        LucideIcons.mail,
                                        emailField: true,
                                        hints: const [AutofillHints.email],
                                      ),
                                      const SizedBox(height: 14),
                                    ],
                                    if (mode == 'reset-password' ||
                                        mode == 'invitation') ...[
                                      _field(
                                        token,
                                        'Code reçu par e-mail',
                                        LucideIcons.keyRound,
                                      ),
                                      const SizedBox(height: 14),
                                    ],
                                    if (mode != 'forgot-password')
                                      TextFormField(
                                        controller: password,
                                        obscureText: !visible,
                                        enabled: !busy,
                                        autofillHints: [
                                          mode == 'register'
                                              ? AutofillHints.newPassword
                                              : AutofillHints.password,
                                        ],
                                        onFieldSubmitted: (_) =>
                                            busy ? null : submit(),
                                        validator: (v) => (v?.length ?? 0) < 10
                                            ? '10 caractères minimum.'
                                            : null,
                                        decoration: InputDecoration(
                                          labelText: 'Mot de passe',
                                          prefixIcon: const Icon(
                                            LucideIcons.lockKeyhole,
                                            size: 18,
                                          ),
                                          suffixIcon: IconButton(
                                            tooltip: visible
                                                ? 'Masquer le mot de passe'
                                                : 'Afficher le mot de passe',
                                            onPressed: () => setState(
                                              () => visible = !visible,
                                            ),
                                            icon: Icon(
                                              visible
                                                  ? Icons
                                                        .visibility_off_outlined
                                                  : LucideIcons.eye,
                                              size: 18,
                                            ),
                                          ),
                                        ),
                                      ),
                                    if (mode == 'login')
                                      Align(
                                        alignment: Alignment.centerRight,
                                        child: TextButton(
                                          onPressed: busy
                                              ? null
                                              : () => switchMode(
                                                  'forgot-password',
                                                ),
                                          child: const Text(
                                            'Mot de passe oublié ?',
                                            style: TextStyle(fontSize: 12),
                                          ),
                                        ),
                                      ),
                                    if (error != null) ...[
                                      const SizedBox(height: 16),
                                      Notice(error!, error: true),
                                    ],
                                    if (success != null) ...[
                                      const SizedBox(height: 16),
                                      Notice(success!),
                                    ],
                                    const SizedBox(height: 22),
                                    FilledButton(
                                      onPressed: busy ? null : submit,
                                      child: busy
                                          ? const SizedBox(
                                              width: 20,
                                              height: 20,
                                              child: CircularProgressIndicator(
                                                strokeWidth: 2,
                                              ),
                                            )
                                          : Text(switch (mode) {
                                              'register' => 'Créer l’espace',
                                              'forgot-password' =>
                                                'Envoyer le lien',
                                              'reset-password' => 'Enregistrer',
                                              'invitation' =>
                                                'Rejoindre l’espace',
                                              _ => 'Se connecter',
                                            }),
                                    ),
                                    const SizedBox(height: 16),
                                    Text(
                                      mode == 'login'
                                          ? 'Nouveau sur ARO ?'
                                          : 'Vous avez déjà un compte ?',
                                      textAlign: TextAlign.center,
                                      style: Theme.of(
                                        context,
                                      ).textTheme.bodySmall,
                                    ),
                                    TextButton(
                                      onPressed: busy
                                          ? null
                                          : () => switchMode(
                                              mode == 'login'
                                                  ? 'register'
                                                  : 'login',
                                            ),
                                      child: Text(
                                        mode == 'login'
                                            ? 'Créer un compte'
                                            : 'Retour à la connexion',
                                      ),
                                    ),
                                    if (mode == 'login')
                                      TextButton(
                                        onPressed: busy
                                            ? null
                                            : () => switchMode('invitation'),
                                        child: const Text(
                                          'J’ai une invitation',
                                          style: TextStyle(fontSize: 12),
                                        ),
                                      ),
                                  ],
                                ),
                              ),
                            ),
                          ),
                        ),
                      ),
                      const SizedBox(height: 24),
                      TextButton.icon(
                        onPressed: busy ? null : configureServer,
                        icon: const Icon(LucideIcons.server, size: 15),
                        label: const Text(
                          'Connexion au serveur',
                          style: TextStyle(fontSize: 12),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _glow(Color color, double size) => IgnorePointer(
    child: Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        gradient: RadialGradient(
          colors: [color.withValues(alpha: .13), color.withValues(alpha: 0)],
        ),
      ),
    ),
  );
  Widget _field(
    TextEditingController controller,
    String label,
    IconData icon, {
    bool required = true,
    bool emailField = false,
    List<String>? hints,
  }) => TextFormField(
    controller: controller,
    enabled: !busy,
    autofillHints: hints,
    textInputAction: TextInputAction.next,
    keyboardType: emailField ? TextInputType.emailAddress : TextInputType.text,
    validator: (v) {
      if (required && (v?.trim().isEmpty ?? true)) {
        return 'Ce champ est requis.';
      }
      if (emailField &&
          !RegExp(r'^[^\s@]+@[^\s@]+\.[^\s@]+$').hasMatch(v!.trim())) {
        return 'Adresse e-mail invalide.';
      }
      return null;
    },
    decoration: InputDecoration(
      labelText: label,
      prefixIcon: Icon(icon, size: 18),
    ),
  );
}
