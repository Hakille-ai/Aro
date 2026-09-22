import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'api.dart';
import 'drafts.dart';
import 'mentions.dart';

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
  late final DraftStore drafts;
  Workspace(this.api, this.preferences, {DraftBackend? draftBackend}) {
    theme = preferences.getString('aro.theme') ?? 'system';
    drafts = DraftStore(
      backend: draftBackend ?? SecureDraftBackend(),
      prefs: preferences,
    );
  }
  String get userName =>
      object(api.session?['user'])['name']?.toString() ?? 'Mon espace';
  String get orgName =>
      object(api.session?['activeOrganization'])['name']?.toString() ?? 'ARO';
  Json? get current =>
      conversations.where((c) => c['id'] == activeId).firstOrNull;
  String get draftKey => 'aro.draft.${api.accountKey}.${activeId ?? 'new'}';

  /// Dernier statut IA serveur (`GET /assistant/status`), vide si inconnu.
  /// Pilotage UX : banniere pre-envoi, badges page Modeles, garde-fous.
  Json aiStatus = {};

  /// Surcharge modele du composer (par message, comme le desktop) : quand
  /// elle est renseignee, le serveur DOIT servir exactement ce modele
  /// (local ou cloud opt-in) ou echouer honnetement, sans substitution.
  /// Null = defaut d'organisation (cascade complete cote serveur).
  Json? modelOverride;

  /// Modele effectif du prochain envoi : surcharge puis defaut d'org.
  Json get effectiveModelRef {
    final o = modelOverride;
    if (o != null && '${o['modelId'] ?? ''}'.isNotEmpty) return o;
    return object(object(settings['model'])['activeModelRef']);
  }

  void setModelOverride(Json? ref) {
    modelOverride = ref;
    notifyListeners();
  }

  bool get aiStatusKnown => aiStatus.isNotEmpty;
  bool get aiCanGenerate => aiStatus['canGenerate'] == true;

  /// Message FR actionnable quand la generation est impossible (notice
  /// d'erreur detaillee, affichee clavier ferme).
  String aiGuidanceMessage() {
    switch ('${aiStatus['guidance'] ?? ''}') {
      case 'start-local-engine':
        return "Aucune IA n'est joignable sur ce serveur. Demarrez Ollama avec un modele de chat sur le serveur (`ollama pull gemma3:1b`), ou demandez a l'administrateur d'activer la generation cloud (Reglages › Modeles).";
      case 'contact-admin':
        return "La generation cloud est autorisee mais incompletement configuree. Demandez a l'administrateur de deposer la cle du provider (Reglages › Modeles).";
      case 'demo-mode':
        return 'Seul le modele de demonstration est actif : ses reponses sont factices. Configurez un vrai moteur (Ollama) ou le cloud (Reglages › Modeles).';
      default:
        return "L'IA est indisponible sur ce serveur. Reessayez plus tard ou verifiez les Reglages › Modeles.";
    }
  }

  /// Version courte pour la banniere persistante : 2 lignes max, meme
  /// clavier ouvert et petit ecran. Le detail vit dans Reglages › Modeles.
  String aiGuidanceShort() {
    switch ('${aiStatus['guidance'] ?? ''}') {
      case 'start-local-engine':
        return 'IA indisponible : aucun moteur local.';
      case 'contact-admin':
        return 'IA indisponible : config admin incomplète.';
      case 'demo-mode':
        return 'Mode démo : réponses factices.';
      default:
        return 'IA indisponible sur ce serveur.';
    }
  }

  Future<void> refreshAiStatus() async {
    if (!api.authenticated) {
      aiStatus = {};
    } else {
      try {
        aiStatus = await api.assistantStatus();
      } catch (_) {
        aiStatus = {};
      }
    }
    notifyListeners();
  }

  Future<void> start() async {
    try {
      await api.restore();
      if (api.authenticated) await refresh();
    } catch (e) {
      error = '$e';
    } finally {
      starting = false;
      await refreshAiStatus();
    }
  }

  Future<void> authenticate(String authMode, Json payload) async {
    await api.login(authMode, payload);
    _accountEpoch++;
    _selection++;
    resetView();
    notifyListeners();
    await refresh();
    await refreshAiStatus();
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
    modelOverride = null;
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
    await refreshAiStatus();
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
      draft = await drafts.read(draftKey) ?? '';
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
    draft = await drafts.read(draftKey) ?? '';
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
    await drafts.write(draftKey, value);
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
      // Pre-check UX : si le serveur a dit ne pas pouvoir generer, on
      // n'envoie rien (ni message utilisateur, ni generation). Erreur
      // ephemere, historique intact. Statut inconnu (hors-ligne) => on
      // laisse le serveur trancher (fail-open).
      if (aiStatusKnown && !aiCanGenerate) {
        error = aiGuidanceMessage();
        return;
      }
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
        await drafts.remove('aro.draft.${api.accountKey}.new');
      }
      final enriched = await buildMentionContext(text.trim());
      // Surcharge explicite (comme desktop/web) : uniquement quand
      // l'utilisateur a choisi un modele dans le composer. Sinon, le serveur
      // applique sa cascade complete (defaut d'organisation).
      final override = modelOverride;
      final hasOverride =
          override != null && '${override['modelId'] ?? ''}'.isNotEmpty;
      final search = object(settings['search']);
      final payload = <String, dynamic>{
        'conversationId': activeId,
        'content': enriched,
        'mode': mode,
        'webAccess': webAccess ? 'on' : 'off',
        'attachments': attachments,
        if (personality?['prompt'] != null)
          'systemPrompt': personality!['prompt'],
        if (hasOverride) 'modelId': '${override['modelId']}',
        // Le contrat serveur nomme ce champ `provider` (id du provider).
        if (hasOverride && '${override['providerId'] ?? ''}'.isNotEmpty)
          'provider': '${override['providerId']}',
        if ('${search['provider'] ?? ''}'.isNotEmpty)
          'searchSettings': {
            'provider': '${search['provider']}',
            if (search['endpoint'] != null)
              'endpoint': '${search['endpoint']}',
          },
        // Client fin : le serveur complete (consignes du mode + souvenirs),
        // comme le harnais desktop. Le desktop envoie "full" (inchange).
        'promptScope': 'personal',
      };
      messages = [
        ...messages,
        {'role': 'user', 'content': text, 'attachments': List.of(attachments)},
        {'role': 'assistant', 'content': ''},
      ];
      draft = '';
      await drafts.remove(draftKey);
      attachments = [];
      notifyListeners();
      // Batch les chunks SSE : un rebuild à ~8 Hz suffit à l'œil et évite
      // de reconstruire tout MaterialApp à chaque token.
      final throttle = Stopwatch()..start();
      var pendingNotify = false;
      Future<void> flush() async {
        if (!pendingNotify) return;
        pendingNotify = false;
        throttle.reset();
        notifyListeners();
        // Laisse un frame respirer avant la prochaine vague de chunks.
        await Future<void>.delayed(const Duration(milliseconds: 1));
      }

      await for (final event in api.stream(payload, _abort!.future)) {
        if (epoch != _accountEpoch) return;
        if (event.type == 'chunk') {
          messages.last['content'] =
              '${messages.last['content']}${event.json['content'] ?? ''}';
          pendingNotify = true;
          if (throttle.elapsedMilliseconds >= 120) await flush();
        } else if (event.type == 'done') {
          // Indisponibilite serveur : rien n'a ete persiste cote serveur
          // (assistantMessage: null). On retire la bulle optimiste (qui a
          // accumule le texte d'erreur streame) et on affiche une notice
          // ephemere : l'historique n'est jamais pollue par des erreurs.
          if (event.json['unavailable'] == true) {
            final streamed = messages.isNotEmpty
                ? '${messages.last['content'] ?? ''}'.trim()
                : '';
            if (messages.isNotEmpty) messages.removeLast();
            error = streamed.isNotEmpty ? streamed : aiGuidanceMessage();
            await refreshAiStatus();
            pendingNotify = true;
            await flush();
          } else {
            final finalMessage = object(event.json['assistantMessage']);
            if (finalMessage.isNotEmpty) {
              messages[messages.length - 1] = finalMessage;
            }
            final userMessage = object(event.json['userMessage']);
            if (userMessage.isNotEmpty) {
              messages[messages.length - 2] = userMessage;
            }
            pendingNotify = true;
            await flush();
          }
        } else if (event.type == 'error') {
          await flush();
          throw ApiException(
            500,
            event.json['error']?.toString() ?? 'La génération a échoué.',
          );
        } else {
          await flush();
        }
      }
      await flush();
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

  /// Construit `Contexte du projet (références @)` comme le desktop :
  /// max 5 fichiers, 12 000 caractères chacun, + skills/agents/MCP/plugins.
  /// L'affichage garde le texte d'origine ; seul le payload est enrichi.
  Future<String> buildMentionContext(String content) async {
    final wanted = dedupeMentionedPaths(extractMentionedPaths(content));
    if (wanted.isEmpty) return content;
    final parts = <String>[];
    try {
      final files = records(await api.request('GET', '/files'));
      final byName = {
        for (final f in files)
          '${f['originalName'] ?? f['name'] ?? ''}'.toLowerCase(): f,
      };
      var count = 0;
      for (final token in wanted) {
        if (token.contains(':')) continue;
        final file = byName[token.toLowerCase()];
        if (file == null || file['id'] == null) continue;
        if (count >= 5) break;
        try {
          final bytes = await api.downloadBytes('${file['id']}');
          final text = utf8.decode(bytes, allowMalformed: true);
          if (text.trim().isEmpty) continue;
          final body = text.length > 12000
              ? '${text.substring(0, 12000)}\n…[tronqué]'
              : text;
          parts.add(
            "--- Fichier : ${file['originalName'] ?? file['name']} ---\n$body",
          );
          count++;
        } catch (_) {
          // Binaire ou indisponible : on garde la référence visible.
        }
      }
    } catch (_) {
      // Hors-ligne : on envoie le message tel quel.
    }
    for (final token in wanted) {
      if (!token.contains(':')) continue;
      final sep = token.indexOf(':');
      final kind = token.substring(0, sep).toLowerCase();
      final key = token.substring(sep + 1).toLowerCase();
      try {
        if (kind == 'skill') {
          final items = records(
            await api.request('GET', '/collections/skills'),
          );
          final found = items.where(
            (s) => '${s['name'] ?? ''}'.toLowerCase() == key,
          ).firstOrNull;
          if (found != null) {
            parts.add(
              "--- Compétence / Skill : ${found['name']} ---\nDescription: ${found['description'] ?? ''}",
            );
          }
        } else if (kind == 'agent') {
          final items = records(
            await api.request('GET', '/collections/agent-definitions'),
          );
          final found = items.where(
            (a) => '${a['name'] ?? ''}'.toLowerCase() == key,
          ).firstOrNull;
          if (found != null) {
            parts.add(
              "--- Agent spécialisé : ${found['name']} ---\nDescription: ${found['description'] ?? ''}\nInstructions: ${found['systemPrompt'] ?? ''}",
            );
          }
        } else if (kind == 'mcp') {
          final items = records(
            await api.request('GET', '/collections/mcp-servers'),
          );
          final found = items.where(
            (m) => '${m['name'] ?? ''}'.toLowerCase() == key,
          ).firstOrNull;
          if (found != null) {
            parts.add(
              "--- Serveur MCP : ${found['name']} ---\n${found['url'] ?? found['command'] ?? ''}",
            );
          }
        } else if (kind == 'plugin') {
          final items = records(await api.request('GET', '/plugins'));
          final found = items.where(
            (p) => '${p['name'] ?? ''}'.toLowerCase() == key,
          ).firstOrNull;
          if (found != null) {
            parts.add(
              "--- Plugin : ${found['name']} ---\nDescription: ${found['description'] ?? ''}",
            );
          }
        }
      } catch (_) {
        // Une collection inaccessible ne bloque jamais l'envoi.
      }
    }
    if (parts.isEmpty) return content;
    return 'Contexte du projet (références @) :\n${parts.join('\n\n')}\n\n--- Message ---\n$content';
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
      await drafts.removeAll('aro.draft.$key.');
      modelOverride = null;
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
