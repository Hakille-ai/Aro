import 'package:aro_mobile/core/drafts.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

class MemoryDraftBackend implements DraftBackend {
  final map = <String, String>{};
  @override
  Future<String?> read(String key) async => map[key];
  @override
  Future<void> write(String key, String? value) async {
    if (value == null) {
      map.remove(key);
    } else {
      map[key] = value;
    }
  }

  @override
  Future<Map<String, String>> readAll() async => Map.of(map);
}

class ThrowingDraftBackend implements DraftBackend {
  @override
  Future<String?> read(String key) async => throw StateError('no keystore');
  @override
  Future<void> write(String key, String? value) async =>
      throw StateError('no keystore');
  @override
  Future<Map<String, String>> readAll() async =>
      throw StateError('no keystore');
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('drafts round-trip through the secure backend, never prefs', () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final backend = MemoryDraftBackend();
    final drafts = DraftStore(backend: backend, prefs: prefs);

    await drafts.write('aro.draft.u.c1', 'hello');
    expect(await drafts.read('aro.draft.u.c1'), 'hello');
    expect(prefs.getString('aro.draft.u.c1'), isNull);
    expect(backend.map.keys.single, 'aro.draft.v1.aro.draft.u.c1');

    await drafts.remove('aro.draft.u.c1');
    expect(await drafts.read('aro.draft.u.c1'), isNull);
  });

  test('legacy prefs drafts migrate to secure storage once', () async {
    SharedPreferences.setMockInitialValues({'aro.draft.u.c2': 'ancien'});
    final prefs = await SharedPreferences.getInstance();
    final backend = MemoryDraftBackend();
    final drafts = DraftStore(backend: backend, prefs: prefs);

    expect(await drafts.read('aro.draft.u.c2'), 'ancien');
    expect(backend.map['aro.draft.v1.aro.draft.u.c2'], 'ancien');
    expect(prefs.getString('aro.draft.u.c2'), isNull);
  });

  test('broken keystore degrades to prefs instead of crashing', () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final drafts = DraftStore(backend: ThrowingDraftBackend(), prefs: prefs);

    await drafts.write('aro.draft.u.c3', 'repli');
    expect(await drafts.read('aro.draft.u.c3'), 'repli');
    await drafts.remove('aro.draft.u.c3');
    expect(await drafts.read('aro.draft.u.c3'), isNull);
  });

  test('removeAll clears one account in both stores', () async {
    SharedPreferences.setMockInitialValues({
      'aro.draft.u1.c1': 'vieux',
      'aro.draft.u2.c1': 'autre',
    });
    final prefs = await SharedPreferences.getInstance();
    final backend = MemoryDraftBackend();
    backend.map['aro.draft.v1.aro.draft.u1.c9'] = 'neuf';
    backend.map['aro.draft.v1.aro.draft.u2.c9'] = 'autre-neuf';
    final drafts = DraftStore(backend: backend, prefs: prefs);

    await drafts.removeAll('aro.draft.u1.');
    expect(prefs.getString('aro.draft.u1.c1'), isNull);
    expect(prefs.getString('aro.draft.u2.c1'), 'autre');
    expect(backend.map.containsKey('aro.draft.v1.aro.draft.u1.c9'), isFalse);
    expect(backend.map['aro.draft.v1.aro.draft.u2.c9'], 'autre-neuf');
  });
}
