import 'dart:convert';
import 'package:aro_mobile/core/api.dart';
import 'package:aro_mobile/core/workspace.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'api_test.dart' show MemoryCredentials, session;

Future<Workspace> sendWorkspace(
  Map<String, http.Response> Function(http.BaseRequest) respond,
  Map<String, dynamic> settings,
) async {
  SharedPreferences.setMockInitialValues({});
  final api = AroApi(
    baseUrl: 'https://example.test',
    credentials: MemoryCredentials(),
    client: MockClient((req) async => respond(req)['*']!),
  );
  await api.saveSession(session('test'));
  final w = Workspace(api, await SharedPreferences.getInstance());
  w.settings = settings;
  w.activeId = 'conv-1';
  return w;
}

Json baseSettings() => {
  'model': {
    'providers': [
      {
        'id': 'ollama-local',
        'kind': 'ollama',
        'enabled': true,
      },
    ],
    'activeModelRef': {
      'providerId': 'ollama-local',
      'providerKind': 'ollama',
      'modelId': 'gemma3:1b',
      'label': 'gemma3:1b',
    },
  },
  'search': {'provider': 'duckduckgo'},
};

String sseDone() => 'event: chunk\ndata: {"content":"ok"}\n\n'
    'event: done\ndata: {"assistantMessage":{"role":"assistant","content":"ok"},"userMessage":{"role":"user","content":"hi"}}\n\n';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('effective model defaults to org active, override wins', () async {
    final w = await sendWorkspace((_) => {}, baseSettings());
    expect(w.modelOverride, isNull);
    expect(w.effectiveModelRef['modelId'], 'gemma3:1b');
    w.setModelOverride({
      'providerId': 'openai',
      'providerKind': 'openai',
      'modelId': 'gpt-x',
      'label': 'GPT X',
    });
    expect(w.effectiveModelRef['modelId'], 'gpt-x');
    w.setModelOverride(null);
    expect(w.effectiveModelRef['modelId'], 'gemma3:1b');
  });

  test('send without override omits model keys (legacy cascade)', () async {
    Map<String, dynamic>? streamed;
    final w = await sendWorkspace((req) {
      if (req.url.path.endsWith('/assistant/stream')) {
        streamed =
            jsonDecode((req as http.Request).body) as Map<String, dynamic>;
        return {
          '*': http.Response(
            sseDone(),
            200,
            headers: {'content-type': 'text/event-stream'},
          ),
        };
      }
      return {'*': http.Response('[]', 200)};
    }, baseSettings());
    await w.send('hi');
    expect(streamed, isNotNull);
    expect(streamed!.containsKey('modelId'), isFalse);
    expect(streamed!.containsKey('provider'), isFalse);
    expect(streamed!['promptScope'], 'personal');
    expect(streamed!['searchSettings'], {'provider': 'duckduckgo'});
  });

  test('send with override transmits modelId/provider (strict server)', () async {
    Map<String, dynamic>? streamed;
    final w = await sendWorkspace((req) {
      if (req.url.path.endsWith('/assistant/stream')) {
        streamed =
            jsonDecode((req as http.Request).body) as Map<String, dynamic>;
        return {
          '*': http.Response(
            sseDone(),
            200,
            headers: {'content-type': 'text/event-stream'},
          ),
        };
      }
      return {'*': http.Response('[]', 200)};
    }, baseSettings());
    w.setModelOverride({
      'providerId': 'openai',
      'providerKind': 'openai',
      'modelId': 'gpt-x',
      'label': 'GPT X',
    });
    await w.send('hi');
    expect(streamed!['modelId'], 'gpt-x');
    expect(streamed!['provider'], 'openai');
    expect(streamed!['promptScope'], 'personal');
  });
}
