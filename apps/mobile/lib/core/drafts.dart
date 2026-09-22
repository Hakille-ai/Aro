import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Brouillons chiffres (entreprise) : le contenu des messages en cours de
/// frappe ne dort plus en clair dans SharedPreferences. Stockage primaire =
/// trousseau OS (flutter_secure_storage), avec migration automatique de
/// l'ancien emplacement et repli honnete sur prefs si le trousseau est
/// indisponible (jamais de crash pour un brouillon).
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

  DraftStore({required this.backend, required this.prefs});

  String _namespaced(String key) => '$prefix$key';

  Future<String?> read(String key) async {
    try {
      final secured = await backend.read(_namespaced(key));
      if (secured != null) return secured;
    } catch (_) {
      // Trousseau indisponible : repli prefs ci-dessous.
    }
    // Migration une fois : prefs -> trousseau, puis suppression de l'ancien.
    final legacy = prefs.getString(key);
    if (legacy == null) return null;
    try {
      await backend.write(_namespaced(key), legacy);
    } catch (_) {
      // On garde la valeur prefs comme repli durable.
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
      try {
        await prefs.remove(key);
      } catch (_) {}
      return;
    } catch (_) {
      // Repli durable : mieux un brouillon en clair qu'un brouillon perdu.
      try {
        await prefs.setString(key, value);
      } catch (_) {}
    }
  }

  Future<void> remove(String key) async {
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
    for (final key in prefs
        .getKeys()
        .where((key) => key.startsWith(accountPrefix))
        .toList()) {
      try {
        await prefs.remove(key);
      } catch (_) {}
    }
  }
}
