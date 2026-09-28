import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Brouillons chiffres (entreprise) : le contenu des messages en cours de
/// frappe ne dort plus en clair dans SharedPreferences. Stockage primaire =
/// trousseau OS (flutter_secure_storage), avec migration automatique de
/// l'ancien emplacement. Si le trousseau est indisponible, le repli reste
/// en mémoire pour cette session et n'écrit pas le contenu en clair.
abstract interface class DraftBackend {
  Future<String?> read(String key);
  Future<void> write(String key, String? value);
  Future<Map<String, String>> readAll();
}

class SecureDraftBackend implements DraftBackend {
  final FlutterSecureStorage storage;
  SecureDraftBackend([this.storage = const FlutterSecureStorage()]);

  @override
  Future<String?> read(String key) => storage.read(key: key);

  @override
  Future<void> write(String key, String? value) => value == null
      ? storage.delete(key: key)
      : storage.write(key: key, value: value);

  @override
  Future<Map<String, String>> readAll() => storage.readAll();
}

class DraftStore {
  static const prefix = 'aro.draft.v1.';
  final DraftBackend backend;
  final SharedPreferences prefs;
  final Map<String, String> _volatile = {};

  DraftStore({required this.backend, required this.prefs});

  String _namespaced(String key) => '$prefix$key';

  Future<String?> read(String key) async {
    if (_volatile.containsKey(key)) return _volatile[key];
    try {
      final secured = await backend.read(_namespaced(key));
      if (secured != null) return secured;
    } catch (_) {
      // Legacy data can still be migrated into volatile memory below.
    }
    // Migration une fois : prefs -> trousseau, puis suppression de l'ancien.
    final legacy = prefs.getString(key);
    if (legacy == null) return null;
    try {
      await backend.write(_namespaced(key), legacy);
    } catch (_) {
      _volatile[key] = legacy;
      try {
        await prefs.remove(key);
      } catch (_) {}
      return legacy;
    }
    try {
      await prefs.remove(key);
    } catch (_) {}
    return legacy;
  }

  Future<void> write(String key, String value) async {
    try {
      await backend.write(_namespaced(key), value);
      _volatile.remove(key);
      try {
        await prefs.remove(key);
      } catch (_) {}
      return;
    } catch (_) {
      _volatile[key] = value;
      try {
        await prefs.remove(key);
      } catch (_) {}
    }
  }

  Future<void> remove(String key) async {
    _volatile.remove(key);
    try {
      await backend.write(_namespaced(key), null);
    } catch (_) {}
    try {
      await prefs.remove(key);
    } catch (_) {}
  }

  /// Supprime tous les brouillons d'un compte (`accountPrefix` =
  /// `aro.draft.<accountKey>.`), dans les deux emplacements.
  Future<void> removeAll(String accountPrefix) async {
    _volatile.removeWhere((key, _) => key.startsWith(accountPrefix));
    try {
      final all = await backend.readAll();
      for (final stored in all.keys) {
        if (stored.startsWith('$prefix$accountPrefix')) {
          try {
            await backend.write(stored, null);
          } catch (_) {}
        }
      }
    } catch (_) {}
    for (final key
        in prefs
            .getKeys()
            .where((key) => key.startsWith(accountPrefix))
            .toList()) {
      try {
        await prefs.remove(key);
      } catch (_) {}
    }
  }
}
