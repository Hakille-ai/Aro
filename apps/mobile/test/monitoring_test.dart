import 'package:flutter_test/flutter_test.dart';
import 'package:aro_mobile/features/monitoring_chart.dart';

void main() {
  test('usageSummary totals tokens and groups by model', () {
    final summary = usageSummary([
      {'modelId': 'a', 'totalTokens': 100},
      {'modelId': 'a', 'tokenCount': 50},
      {'modelId': 'b', 'totalTokens': 25},
      {'kind': 'run'},
    ]);
    expect(summary.events, 4);
    expect(summary.totalTokens, 175);
    expect(summary.byModel['a'], 150);
    expect(summary.byModel['b'], 25);
  });

  test('usageBarsTokens keeps the last 20 events', () {
    final items = [for (var i = 0; i < 30; i++) {'totalTokens': i}];
    final bars = usageBarsTokens(items);
    expect(bars.length, 20);
    expect(bars.first, 10);
    expect(bars.last, 29);
  });
}
