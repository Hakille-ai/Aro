import 'dart:async';
import 'dart:convert';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:aro_mobile/core/api.dart';

class MemoryCredentials implements CredentialStore {
  String? value;
  @override
  Future<String?> read() async => value;
  @override
  Future<void> write(String? next) async {
    value = next;
  }
}

Json session(String token) => {
  'accessToken': token,
  'refreshToken': 'refresh-$token',
  'user': {'id': 'user-a'},
  'activeOrganization': {'id': 'org-a'},
};
void main() {
  test(
    'SSE survives byte boundaries, CRLF, Unicode and multiline data',
    () async {
      final bytes = utf8.encode(
        ': keepalive\r\nid: 12\r\nevent: chunk\r\ndata: {"content":\r\ndata: "été 🌱"}\r\n\r\nevent: done\ndata: {}\n\n',
      );
      final events = await decodeEvents(
        Stream.fromIterable(bytes.map((b) => [b])),
      ).toList();
      expect(events.length, 2);
      expect(events.first.id, '12');
      expect(events.first.json['content'], 'été 🌱');
      expect(events.last.type, 'done');
    },
  );
  test('Incomplete SSE event is not dispatched', () async {
    expect(
      await decodeEvents(
        Stream.value(utf8.encode('data: {"content":"partial"}\n')),
      ).toList(),
      isEmpty,
    );
  });
  test(
    'Concurrent 401s share refresh with the flat AuthSession contract',
    () async {
      var refreshes = 0;
      final api = AroApi(
        baseUrl: 'https://example.test',
        credentials: MemoryCredentials(),
        client: MockClient((req) async {
          if (req.url.path.endsWith('/auth/refresh')) {
            refreshes++;
            await Future<void>.delayed(const Duration(milliseconds: 10));
            return http.Response(jsonEncode(session('new')), 200);
          }
          if (req.headers['Authorization'] == 'Bearer old') {
            return http.Response('{}', 401);
          }
          expect(req.headers['Authorization'], 'Bearer new');
          return http.Response('[]', 200);
        }),
      );
      await api.saveSession(session('old'));
      await Future.wait([
        api.request('GET', '/projects'),
        api.request('GET', '/folders'),
      ]);
      expect(refreshes, 1);
      expect(api.session!['refreshToken'], 'refresh-new');
    },
  );
  test('Credentials are bound to the configured server', () async {
    final storage = MemoryCredentials();
    final first = AroApi(baseUrl: 'https://one.test', credentials: storage);
    await first.saveSession(session('secret'));
    final second = AroApi(baseUrl: 'https://two.test', credentials: storage);
    await second.restore();
    expect(second.authenticated, isFalse);
  });
  test('Logout cannot be undone by an in-flight refresh', () async {
    final refresh = Completer<http.Response>();
    final started = Completer<void>();
    final api = AroApi(
      baseUrl: 'https://example.test',
      credentials: MemoryCredentials(),
      client: MockClient((req) async {
        if (req.url.path.endsWith('/auth/refresh')) {
          started.complete();
          return refresh.future;
        }
        return http.Response('{}', 401);
      }),
    );
    await api.saveSession(session('old'));
    final request = api.request('GET', '/projects');
    final assertion = expectLater(request, throwsA(isA<ApiException>()));
    await started.future;
    await api.clearSession();
    refresh.complete(http.Response(jsonEncode(session('new')), 200));
    await assertion;
    expect(api.authenticated, isFalse);
  });
  test('Failed writes are not automatically replayed', () async {
    var calls = 0;
    final api = AroApi(
      baseUrl: 'https://example.test',
      credentials: MemoryCredentials(),
      client: MockClient((req) async {
        calls++;
        return http.Response('{"error":"busy"}', 503);
      }),
    );
    await expectLater(
      api.request('POST', '/conversations', body: {'title': 'test'}),
      throwsA(isA<ApiException>()),
    );
    expect(calls, 1);
  });
  test(
    'A pending write cannot be replayed into another organization',
    () async {
      final pending = Completer<http.Response>();
      final started = Completer<void>();
      var calls = 0;
      final api = AroApi(
        baseUrl: 'https://example.test',
        credentials: MemoryCredentials(),
        client: MockClient((request) {
          calls++;
          started.complete();
          return pending.future;
        }),
      );
      await api.saveSession(session('old'));
      final request = api.request(
        'POST',
        '/projects',
        body: {'name': 'Private'},
      );
      final assertion = expectLater(request, throwsA(isA<ApiException>()));
      await started.future;
      await api.saveSession({
        ...session('new'),
        'activeOrganization': {'id': 'org-b'},
      });
      pending.complete(http.Response('{}', 401));
      await assertion;
      expect(calls, 1);
    },
  );
  test('Collection record aliases preserve nested user configuration', () {
    final values = records([
      {
        'system_prompt': 'Act carefully',
        'created_at': 'today',
        'config': {'custom_key': 1},
      },
    ]);
    expect(values.single['systemPrompt'], 'Act carefully');
    expect(values.single['config'], {'custom_key': 1});
  });
  test(
    'Server rejects embedded credentials, query parameters and non-HTTP URLs',
    () {
      for (final url in [
        'https://user:password@example.test',
        'file:///tmp',
        'https://example.test?token=secret',
      ]) {
        expect(() => AroApi.validateServer(url), throwsA(isA<ApiException>()));
      }
      expect(
        AroApi.validateServer('https://example.test/v1/'),
        'https://example.test',
      );
    },
  );
}
