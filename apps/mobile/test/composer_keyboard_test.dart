import 'package:aro_mobile/core/api.dart';
import 'package:aro_mobile/core/workspace.dart';
import 'package:aro_mobile/main.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'api_test.dart' show MemoryCredentials, session;

Future<Workspace> keyboardWorkspace() async {
  SharedPreferences.setMockInitialValues({});
  final api = AroApi(
    baseUrl: 'https://example.test',
    credentials: MemoryCredentials(),
    client: MockClient((req) async => http.Response('[]', 200)),
  );
  await api.saveSession(session('test'));
  final w = Workspace(api, await SharedPreferences.getInstance())
    ..starting = false;
  return w;
}

Future<void> pumpChatWithKeyboard(
  WidgetTester tester, {
  Json aiStatus = const {},
}) async {
  // Pixel 9 logique (~412x917) + clavier Gboard ouvert.
  tester.view.physicalSize = const Size(412, 917);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.view.resetViewInsets);

  final w = await keyboardWorkspace();
  w.aiStatus = aiStatus;
  await tester.pumpWidget(AroApp(workspace: w));
  await tester.pumpAndSettle();

  await tester.tap(
    find.text("Demandez n'importe quoi à ARO (@ pour citer)"),
  );
  await tester.pump();
  tester.view.viewInsets = const FakeViewPadding(bottom: 350);
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('Composer never overflows when the keyboard opens', (
    tester,
  ) async {
    await pumpChatWithKeyboard(tester);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Chat survives keyboard with AI and error banners shown', (
    tester,
  ) async {
    await pumpChatWithKeyboard(tester, aiStatus: {
      'canGenerate': false,
      'source': 'none',
      'guidance': 'start-local-engine',
      'providers': [],
      'runnableModelIds': [],
    });
    // La banniere IA compacte est bien affichee...
    expect(
      find.text('IA indisponible : aucun moteur local.'),
      findsOneWidget,
    );
    // ...et rien ne deborde malgre la hauteur reduite.
    expect(tester.takeException(), isNull);
  });
}
