import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'core/api.dart';
import 'core/workspace.dart';
import 'features/auth.dart';
import 'features/home.dart';
import 'ui/design.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final prefs = await SharedPreferences.getInstance();
  final server =
      prefs.getString('aro.server') ??
      const String.fromEnvironment('ARO_API_URL');
  final workspace = Workspace(
    AroApi(baseUrl: server, credentials: SecureCredentials()),
    prefs,
  );
  runApp(AroApp(workspace: workspace));
  await workspace.start();
}

class AroApp extends StatelessWidget {
  final Workspace workspace;
  const AroApp({super.key, required this.workspace});
  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: workspace,
    builder: (context, _) => MaterialApp(
      key: ValueKey(
        '${workspace.api.authenticated}:${workspace.api.accountKey}',
      ),
      title: 'ARO',
      debugShowCheckedModeBanner: false,
      theme: AroDesign.theme(Brightness.light),
      darkTheme: AroDesign.theme(
        Brightness.dark,
        oled: workspace.theme == 'oled',
      ),
      themeMode: switch (workspace.theme) {
        'light' => ThemeMode.light,
        'dark' || 'oled' => ThemeMode.dark,
        _ => ThemeMode.system,
      },
      locale: const Locale('fr'),
      supportedLocales: const [Locale('fr'), Locale('en')],
      localizationsDelegates: GlobalMaterialLocalizations.delegates,
      home: workspace.starting
          ? const SplashPage()
          : workspace.api.authenticated
          ? HomePage(workspace: workspace)
          : AuthPage(workspace: workspace),
    ),
  );
}
