import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:aro_mobile/core/api.dart';
import 'package:aro_mobile/core/workspace.dart';
import 'package:aro_mobile/main.dart';
import 'package:aro_mobile/features/settings.dart';
import 'package:aro_mobile/features/workspace_panel.dart';
import 'package:aro_mobile/features/settings_catalog.dart';
import 'package:aro_mobile/ui/design.dart';
import 'api_test.dart' show MemoryCredentials, session;

Future<Workspace> workspace({
  bool signedIn = false,
  Future<http.Response> Function(http.Request)? handler,
}) async {
  SharedPreferences.setMockInitialValues({});
  final api = AroApi(
    baseUrl: 'https://example.test',
    credentials: MemoryCredentials(),
    client: MockClient(
      handler ?? (req) async => http.Response(jsonEncode([]), 200),
    ),
  );
  if (signedIn) await api.saveSession(session('test'));
  final w = Workspace(api, await SharedPreferences.getInstance())
    ..starting = false;
  w.settings = {
    'model': {'providers': [], 'temperature': .7},
    'search': {'provider': 'google-scrape'},
    'voice': {},
  };
  return w;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('flutter_tts'),
          (call) async => 1,
        );
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugin.csdcorp.com/speech_to_text'),
          (call) async => false,
        );
  });
  testWidgets('Login and register fit a narrow phone with accessible labels', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(360, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(AroApp(workspace: await workspace()));
    await tester.pumpAndSettle();
    expect(find.text('Connexion'), findsOneWidget);
    await tester.tap(find.text('Créer un compte'));
    await tester.pumpAndSettle();
    expect(find.text('Créer votre espace'), findsOneWidget);
    expect(find.text('Nom complet'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
  testWidgets('Authenticated entry is chat and sidebar opens settings', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(390, 844);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(AroApp(workspace: await workspace(signedIn: true)));
    await tester.pumpAndSettle();
    expect(find.text('Nouvelle conversation'), findsOneWidget);
    await tester.tap(find.byTooltip('Ouvrir la barre latérale'));
    await tester.pumpAndSettle();
    expect(find.text('PROJETS'), findsOneWidget);
    expect(find.byTooltip('Ajouter dossiers'), findsOneWidget);
    await tester.tap(find.text('Paramètres'));
    await tester.pumpAndSettle();
    expect(find.text('Rechercher dans les réglages'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
  testWidgets('Every desktop settings destination renders without overflow', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(360, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final w = await workspace(signedIn: true);
    for (final section in settingsSections) {
      await tester.pumpWidget(
        MaterialApp(
          key: ValueKey(section.id),
          theme: AroDesign.theme(Brightness.light),
          home: SettingsDetail(workspace: w, section: section),
        ),
      );
      await tester.pumpAndSettle();
      expect(
        find.text('Paramètres › ${section.title}'),
        findsOneWidget,
        reason: section.id,
      );
      expect(tester.takeException(), isNull, reason: section.id);
    }
  });
  testWidgets('Composer selects a real model and has no mode selector', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(360, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    Json? saved;
    final w = await workspace(
      signedIn: true,
      handler: (req) async {
        if (req.method == 'PUT' && req.url.path.endsWith('/settings')) {
          saved = object(jsonDecode(req.body));
          return http.Response(req.body, 200);
        }
        return http.Response('[]', 200);
      },
    );
    final ref = <String, dynamic>{
      'providerId': 'test',
      'providerKind': 'ollama',
      'modelId': 'gemma:test',
      'label': 'Gemma test',
    };
    w.settings = {
      'model': {
        'providers': [
          {
            'id': 'test',
            'kind': 'ollama',
            'enabled': true,
            'models': [ref],
          },
        ],
      },
    };
    await tester.pumpWidget(AroApp(workspace: w));
    await tester.pumpAndSettle();
    expect(find.byTooltip('Mode de conversation'), findsNothing);
    await tester.tap(find.text('Modèle IA'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Gemma test'));
    await tester.pumpAndSettle();
    expect(
      object(object(saved?['model'])['activeModelRef'])['modelId'],
      'gemma:test',
    );
    expect(find.text('Gemma test'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
  testWidgets('Sidebar hides project actions until a long press', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(390, 844);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final w = await workspace(signedIn: true);
    w.projects = [
      {'id': 'p', 'name': 'Projet discret'},
    ];
    await tester.pumpWidget(AroApp(workspace: w));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Ouvrir la barre latérale'));
    await tester.pumpAndSettle();
    expect(find.byTooltip('Organiser Projet discret'), findsNothing);
    await tester.longPress(find.text('Projet discret'));
    await tester.pumpAndSettle();
    expect(find.byTooltip('Organiser Projet discret'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
  testWidgets('Workspace hub and six panels fit a small phone', (tester) async {
    tester.view.physicalSize = const Size(360, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final w = await workspace(signedIn: true);
    await tester.pumpWidget(
      MaterialApp(
        theme: AroDesign.theme(Brightness.light),
        home: WorkspacePanel(workspace: w),
      ),
    );
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    for (final label in [
      'Sorties & Artefacts',
      'Sous-Agents',
      'Fichiers du Workspace',
      'Plan de Travail',
      'Navigateur Web',
      'Sources & Références',
    ]) {
      await tester.tap(find.byTooltip(label));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull, reason: label);
    }
  });
  test(
    'Settings coverage follows the desktop union, excluding its general alias',
    () {
      final source = File(
        '../desktop/src/features/settings/types.ts',
      ).readAsStringSync().split('export type ShortcutKey').first;
      final ids = RegExp(r'"([a-z-]+)"')
          .allMatches(source)
          .map((m) => m[1]!)
          .where((id) => id != 'general')
          .toSet();
      expect(settingsSections.map((s) => s.id).toSet(), ids);
    },
  );
}
