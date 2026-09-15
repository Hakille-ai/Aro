import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'api.dart';

class Workspace extends ChangeNotifier {
  final AroApi api;
  final SharedPreferences preferences;
  bool starting = true, loading = false, sending = false;
  String? error, activeId;
  String? destinationProject, destinationFolder;
  Json? personality;
  String mode = 'chat', theme = 'system';
  bool webAccess = false;
  Json bootstrap = {}, settings = {};
  List<Json> conversations = [],
      projects = [],
      folders = [],
      messages = [],
      attachments = [];
  Completer<void>? _abort;
  int _selection = 0;
  int _accountEpoch = 0;
  String draft = '';
  Workspace(this.api, this.preferences) {
    theme = preferences.getString('aro.theme') ?? 'system';
  }
  String get userName =>
      object(api.session?['user'])['name']?.toString() ?? 'Mon espace';
  String get orgName =>
      object(api.session?['activeOrganization'])['name']?.toString() ?? 'ARO';
  Json? get current =>
      conversations.where((c) => c['id'] == activeId).firstOrNull;
  String get draftKey => 'aro.draft.${api.accountKey}.${activeId ?? 'new'}';

  Future<void> start() async {
    try {
      await api.restore();
      if (api.authenticated) await refresh();
    } catch (e) {
      error = '$e';
    } finally {
      starting = false;
      notifyListeners();
    }
  }

  Future<void> authenticate(String authMode, Json payload) async {
    await api.login(authMode, payload);
    _accountEpoch++;
    _selection++;
    resetView();
    notifyListeners();
    await refresh();
  }

  void resetView() {
    conversations = [];
    projects = [];
    folders = [];
    messages = [];
    attachments = [];
    bootstrap = {};
    settings = {};
    activeId = null;
    draft = '';
    error = null;
    destinationProject = null;
    destinationFolder = null;
    personality = null;
  }

  Future<void> switchOrganization(String id) async {
    if (sending) {
      throw const ApiException(
        0,
        'Arrêtez la réponse avant de changer d’organisation.',
      );
    }
    final result = object(
      await api.request(
        'POST',
        '/auth/switch-organization',
        body: {'organizationId': id},
      ),
    );
    _accountEpoch++;
    _selection++;
    await api.saveSession(result);
    resetView();
    notifyListeners();
    await refresh();
  }

  Future<void> refresh() async {
    final epoch = _accountEpoch;
    loading = true;
    error = null;
    notifyListeners();
    try {
      final data = await Future.wait([
        api.request('GET', '/bootstrap'),
        api.request('GET', '/projects'),
        api.request('GET', '/folders'),
      ]);
      if (epoch != _accountEpoch) return;
      bootstrap = object(data[0]);
      settings = object(bootstrap['settings']);
      conversations = records(bootstrap['conversations']);
      projects = records(data[1]);
      folders = records(data[2]);
      draft = preferences.getString(draftKey) ?? '';
    } catch (e) {
      if (epoch == _accountEpoch) error = '$e';
    } finally {
      if (epoch == _accountEpoch) {
        loading = false;
        notifyListeners();
      }
    }
  }

  Future<void> select(String? id) async {
    if (sending) return;
    final selected = ++_selection;
    activeId = id;
    loading = false;
    mode = current?['mode']?.toString() ?? 'chat';
    destinationProject = current?['projectId'];
    destinationFolder = current?['folderId'];
    messages = [];
    error = null;
    attachments = [];
    draft = preferences.getString(draftKey) ?? '';
    notifyListeners();
    if (id == null) return;
    loading = true;
    notifyListeners();
    try {
      final result = records(
        await api.request('GET', '/conversations/$id/messages'),
      );
      if (selected == _selection) messages = result;
    } catch (e) {
      if (selected == _selection) error = '$e';
    } finally {
      if (selected == _selection) {
        loading = false;
        notifyListeners();
      }
    }
  }

  Future<void> saveDraft(String value) async {
    draft = value;
    await preferences.setString(draftKey, value);
  }

  Future<String> ensureConversation(String title) async {
    if (activeId != null) return activeId!;
    final epoch = _accountEpoch;
    final created = object(
      await api.request(
        'POST',
        '/conversations',
        body: {
          'title': title,
          'mode': mode,
          if (destinationProject != null) 'projectId': destinationProject,
          if (destinationFolder != null) 'folderId': destinationFolder,
        },
      ),
    );
    if (epoch != _accountEpoch) throw const ApiException(401, 'Espace changé.');
    activeId = created['id'] as String;
    conversations.insert(0, created);
    notifyListeners();
    return activeId!;
  }

  Future<void> setTheme(String value) async {
    theme = value;
    notifyListeners();
    await preferences.setString('aro.theme', value);
  }

  Future<void> send(String text) async {
    if (sending || (text.trim().isEmpty && attachments.isEmpty)) return;
    final epoch = _accountEpoch;
    sending = true;
    error = null;
    _abort = Completer<void>();
    notifyListeners();
    try {
      if (activeId == null) {
        final created = object(
          await api.request(
            'POST',
            '/conversations',
            body: {
              'title': text.trim().isEmpty
                  ? 'Document partagé'
                  : text.trim().substring(0, text.trim().length.clamp(0, 70)),
              'mode': mode,
              if (destinationProject != null) 'projectId': destinationProject,
              if (destinationFolder != null) 'folderId': destinationFolder,
            },
          ),
        );
        if (epoch != _accountEpoch) return;
        activeId = created['id'] as String;
        conversations.insert(0, created);
        await preferences.remove('aro.draft.${api.accountKey}.new');
      }
      final payload = <String, dynamic>{
        'conversationId': activeId,
        'content': text.trim(),
        'mode': mode,
        'webAccess': webAccess ? 'on' : 'off',
        'attachments': attachments,
        if (personality?['prompt'] != null)
          'systemPrompt': personality!['prompt'],
      };
      messages = [
        ...messages,
        {'role': 'user', 'content': text, 'attachments': List.of(attachments)},
        {'role': 'assistant', 'content': ''},
      ];
      draft = '';
      await preferences.remove(draftKey);
      attachments = [];
      notifyListeners();
      await for (final event in api.stream(payload, _abort!.future)) {
        if (epoch != _accountEpoch) return;
        if (event.type == 'chunk') {
          messages.last['content'] =
              '${messages.last['content']}${event.json['content'] ?? ''}';
        } else if (event.type == 'done') {
          final finalMessage = object(event.json['assistantMessage']);
          if (finalMessage.isNotEmpty) {
            messages[messages.length - 1] = finalMessage;
          }
          final userMessage = object(event.json['userMessage']);
          if (userMessage.isNotEmpty) {
            messages[messages.length - 2] = userMessage;
          }
        } else if (event.type == 'error') {
          throw ApiException(
            500,
            event.json['error']?.toString() ?? 'La génération a échoué.',
          );
        }
        notifyListeners();
      }
    } catch (e) {
      if (epoch == _accountEpoch) {
        error = _abort?.isCompleted == true
            ? 'Réception arrêtée. Le serveur peut terminer la tâche ; actualisez pour retrouver son résultat.'
            : '$e';
        if (draft.isEmpty) await saveDraft(text);
      }
    } finally {
      if (epoch == _accountEpoch) {
        sending = false;
        _abort = null;
        notifyListeners();
      }
    }
  }

  void stop() {
    if (_abort?.isCompleted == false) _abort!.complete();
  }

  Future<void> mutate(String method, String path, {dynamic body}) async {
    await api.request(method, path, body: body);
    await refresh();
  }

  Future<void> saveSettings(Json next) async {
    settings = object(await api.request('PUT', '/settings', body: next));
    notifyListeners();
  }

  Future<void> logout() async {
    stop();
    _selection++;
    _accountEpoch++;
    try {
      await api.request(
        'POST',
        '/auth/logout',
        body: {'refreshToken': api.session?['refreshToken']},
      );
    } finally {
      final key = api.accountKey;
      await api.clearSession();
      for (final item
          in preferences
              .getKeys()
              .where((k) => k.startsWith('aro.draft.$key.'))
              .toList()) {
        await preferences.remove(item);
      }
      conversations = [];
      folders = [];
      projects = [];
      messages = [];
      settings = {};
      bootstrap = {};
      activeId = null;
      draft = '';
      attachments = [];
      sending = false;
      error = null;
      notifyListeners();
    }
  }

  Json copySettings() => object(jsonDecode(jsonEncode(settings)));
}
