import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:http/http.dart' as http;

typedef Json = Map<String, dynamic>;
Json object(dynamic value) =>
    value is Map ? Map<String, dynamic>.from(value) : <String, dynamic>{};
// Collection endpoints return PostgreSQL snake_case records; typed Axum routes
// return camelCase. Normalize record keys once, without rewriting user JSON.
Json record(dynamic value) => object(value).map(
  (key, value) => MapEntry(
    key.replaceAllMapped(RegExp(r'_([a-z])'), (m) => m[1]!.toUpperCase()),
    value,
  ),
);
List<Json> records(dynamic value) =>
    (value is List ? value : object(value)['items'] as List? ?? [])
        .map(record)
        .toList();

class ApiException implements Exception {
  final int status;
  final String message;
  const ApiException(this.status, this.message);
  @override
  String toString() => message;
}

abstract interface class CredentialStore {
  Future<String?> read();
  Future<void> write(String? value);
}

class SecureCredentials implements CredentialStore {
  final FlutterSecureStorage storage;
  SecureCredentials([this.storage = const FlutterSecureStorage()]);
  @override
  Future<String?> read() => storage.read(key: 'aro.mobile.session.v1');
  @override
  Future<void> write(String? value) => value == null
      ? storage.delete(key: 'aro.mobile.session.v1')
      : storage.write(key: 'aro.mobile.session.v1', value: value);
}

class ServerEvent {
  final String type;
  final String data;
  final String? id;
  const ServerEvent(this.type, this.data, this.id);
  Json get json => object(jsonDecode(data));
}

/// Decodes arbitrary UTF-8/network boundaries and dispatches only complete SSE events.
Stream<ServerEvent> decodeEvents(Stream<List<int>> bytes) async* {
  var type = 'message';
  String? id;
  final data = <String>[];
  await for (final line
      in bytes.transform(utf8.decoder).transform(const LineSplitter())) {
    if (line.isEmpty) {
      if (data.isNotEmpty) yield ServerEvent(type, data.join('\n'), id);
      data.clear();
      type = 'message';
      continue;
    }
    if (line.startsWith(':')) continue;
    final colon = line.indexOf(':');
    final key = colon < 0 ? line : line.substring(0, colon);
    var value = colon < 0 ? '' : line.substring(colon + 1);
    if (value.startsWith(' ')) value = value.substring(1);
    if (key == 'event') type = value;
    if (key == 'data') data.add(value);
    if (key == 'id' && !value.contains('\u0000')) id = value;
  }
}

class AroApi {
  final http.Client client;
  final CredentialStore credentials;
  String baseUrl;
  Json? session;
  Future<void>? _refreshing;
  int _sessionGeneration = 0;
  AroApi({
    required this.baseUrl,
    required this.credentials,
    http.Client? client,
  }) : client = client ?? http.Client();
  bool get authenticated => session?['accessToken'] != null;
  String get accountKey =>
      '${object(session?['user'])['id']}:${object(session?['activeOrganization'])['id']}';

  static String validateServer(String raw) {
    final uri = Uri.tryParse(raw.trim());
    if (uri == null ||
        !uri.hasAuthority ||
        uri.userInfo.isNotEmpty ||
        uri.hasQuery ||
        uri.hasFragment ||
        !['https', 'http'].contains(uri.scheme)) {
      throw const ApiException(0, 'Saisissez une adresse de serveur valide.');
    }
    if (kReleaseMode && uri.scheme != 'https') {
      throw const ApiException(0, 'Une connexion HTTPS est requise.');
    }
    return raw
        .trim()
        .replaceFirst(RegExp(r'/+$'), '')
        .replaceFirst(RegExp(r'/v1$'), '');
  }

  Uri uri(String path) => Uri.parse(
    '${validateServer(baseUrl)}${path == '/health' ? '' : '/v1'}$path',
  );
  Map<String, String> get headers => {
    'Content-Type': 'application/json',
    if (authenticated) 'Authorization': 'Bearer ${session!['accessToken']}',
  };

  Future<void> restore() async {
    final saved = await credentials.read();
    if (saved == null) return;
    try {
      final data = object(jsonDecode(saved));
      if (data['server'] == baseUrl) session = object(data['session']);
    } on FormatException {
      await credentials.write(null);
    }
  }

  Future<void> saveSession(Json value) async {
    if (session == null ||
        object(value['user'])['id'] != object(session?['user'])['id'] ||
        object(value['activeOrganization'])['id'] !=
            object(session?['activeOrganization'])['id']) {
      _sessionGeneration++;
    }
    session = value;
    await credentials.write(jsonEncode({'server': baseUrl, 'session': value}));
  }

  Future<void> clearSession() async {
    _sessionGeneration++;
    session = null;
    await credentials.write(null);
  }

  Future<void> login(String mode, Json payload) async {
    final value = object(
      await request('POST', '/auth/$mode', body: payload, auth: false),
    );
    if (value['accessToken'] == null || value['refreshToken'] == null) {
      throw const ApiException(500, 'Réponse de connexion invalide.');
    }
    await saveSession(value);
  }

  Future<void> _refresh() {
    if (_refreshing != null) return _refreshing!;
    final generation = _sessionGeneration;
    return _refreshing = (() async {
      final token = session?['refreshToken'];
      if (token == null) {
        throw const ApiException(401, 'Reconnectez-vous à votre espace.');
      }
      try {
        final value = object(
          await request(
            'POST',
            '/auth/refresh',
            body: {'refreshToken': token},
            auth: false,
          ),
        );
        if (generation != _sessionGeneration) {
          throw const ApiException(401, 'Session fermée.');
        }
        await saveSession(value);
      } on ApiException catch (e) {
        if ((e.status == 401 || e.status == 403) &&
            generation == _sessionGeneration) {
          await clearSession();
        }
        rethrow;
      } finally {
        _refreshing = null;
      }
    })();
  }

  Future<dynamic> request(
    String method,
    String path, {
    dynamic body,
    bool auth = true,
    bool retry = true,
  }) async {
    try {
      final generation = _sessionGeneration;
      final token = session?['accessToken'];
      final req = http.Request(method, uri(path));
      req.headers.addAll(auth ? headers : {'Content-Type': 'application/json'});
      if (body != null) req.body = jsonEncode(body);
      final response = await http.Response.fromStream(
        await client.send(req).timeout(const Duration(seconds: 25)),
      ).timeout(const Duration(seconds: 30));
      if (response.statusCode == 401 && auth && retry && authenticated) {
        if (generation != _sessionGeneration) {
          throw const ApiException(
            401,
            'Espace changé. Réessayez depuis votre espace actuel.',
          );
        }
        if (token == session?['accessToken']) await _refresh();
        return await request(
          method,
          path,
          body: body,
          auth: auth,
          retry: false,
        );
      }
      if (response.statusCode >= 400) {
        throw _error(response.statusCode, response.body);
      }
      return response.body.isEmpty ? null : jsonDecode(response.body);
    } on TimeoutException {
      throw const ApiException(
        0,
        'Le serveur met trop de temps à répondre. Réessayez.',
      );
    } on http.ClientException {
      throw const ApiException(
        0,
        'Connexion au serveur impossible. Vérifiez le réseau et son adresse.',
      );
    } on FormatException {
      throw const ApiException(
        502,
        'Le serveur a renvoyé une réponse incompatible.',
      );
    }
  }

  ApiException _error(int code, String body) {
    var message = 'La demande a échoué ($code).';
    try {
      message = object(jsonDecode(body))['error']?.toString() ?? message;
    } catch (_) {
      /* Never render an HTML error response. */
    }
    if (code == 401) message = 'Votre session a expiré. Reconnectez-vous.';
    if (code == 403) message = 'Votre rôle ne permet pas cette action.';
    if (code == 404) {
      message = 'Cette ressource ou fonction est indisponible sur ce serveur.';
    }
    return ApiException(code, message);
  }

  Stream<ServerEvent> stream(Json payload, Future<void> abort) async* {
    final token = session?['accessToken'];
    http.AbortableRequest build() =>
        http.AbortableRequest(
            'POST',
            uri('/assistant/stream'),
            abortTrigger: abort,
          )
          ..headers.addAll({...headers, 'Accept': 'text/event-stream'})
          ..body = jsonEncode(payload);
    var response = await client
        .send(build())
        .timeout(const Duration(minutes: 2));
    if (response.statusCode == 401 && authenticated) {
      await response.stream.drain<void>();
      if (token == session?['accessToken']) await _refresh();
      response = await client.send(build()).timeout(const Duration(minutes: 2));
    }
    if (response.statusCode >= 400) {
      throw _error(response.statusCode, await response.stream.bytesToString());
    }
    var completed = false;
    await for (final event in decodeEvents(
      response.stream.timeout(const Duration(minutes: 2)),
    )) {
      if (event.type == 'done') completed = true;
      yield event;
    }
    if (!completed) {
      throw const ApiException(
        0,
        'Réponse interrompue. Actualisez la conversation pour récupérer le résultat enregistré.',
      );
    }
  }

  /// Pre-check UX : le client sait AVANT d'envoyer si le serveur peut
  /// generer, par quel chemin, et quoi faire sinon. Aucun secret expose.
  Future<Json> assistantStatus() async =>
      object(await request('GET', '/assistant/status'));

  Future<Uint8List> downloadBytes(String fileId) async {
    final req = http.Request('GET', uri('/files/$fileId/content'));
    req.headers.addAll(headers);
    final response = await http.Response.fromStream(
      await client.send(req).timeout(const Duration(minutes: 2)),
    ).timeout(const Duration(minutes: 2));
    if (response.statusCode == 401 && authenticated) {
      await _refresh();
      return downloadBytes(fileId);
    }
    if (response.statusCode >= 400) {
      throw _error(response.statusCode, response.body);
    }
    return response.bodyBytes;
  }

  Future<Json> upload(String name, Uint8List bytes, String digest) async {
    final result = object(
      await request(
        'POST',
        '/files/uploads',
        body: {
          'originalName': name,
          'mimeType': 'application/octet-stream',
          'sizeBytes': bytes.length,
          'sha256': digest,
        },
      ),
    );
    final uploadId = object(result['upload'])['id'];
    if (uploadId == null) {
      throw const ApiException(502, 'Impossible de préparer le fichier.');
    }
    final response = await client
        .put(
          uri('/files/uploads/$uploadId/content'),
          headers: {...headers, 'Content-Type': 'application/octet-stream'},
          body: bytes,
        )
        .timeout(const Duration(minutes: 2));
    if (response.statusCode >= 400) {
      throw _error(response.statusCode, response.body);
    }
    return object(result['file']);
  }
}
