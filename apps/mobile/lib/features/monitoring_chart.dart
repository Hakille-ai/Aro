import 'package:flutter/material.dart';

import '../core/api.dart';

/// Résumé de consommation — miroir mobile de `MonitoringSettings.svelte` +
/// `TokenConsumptionGauge.svelte` : totaux, top modèles, barres et jauge
/// vs budget contexte 8 192 tokens (desktop `ContextBudget` total).
({int events, int totalTokens, Map<String, int> byModel}) usageSummary(
  List<Json> items,
) {
  var total = 0;
  final byModel = <String, int>{};
  for (final item in items) {
    final tokens =
        (item['totalTokens'] as num? ?? item['tokenCount'] as num? ?? 0)
            .toInt();
    total += tokens;
    final model = '${item['modelId'] ?? item['kind'] ?? 'Inconnu'}';
    byModel[model] = (byModel[model] ?? 0) + tokens;
  }
  return (events: items.length, totalTokens: total, byModel: byModel);
}

List<int> usageBarsTokens(List<Json> items, [int count = 20]) {
  final recent = items.length <= count
      ? items
      : items.sublist(items.length - count);
  return [
    for (final item in recent)
      (item['totalTokens'] as num? ?? item['tokenCount'] as num? ?? 0).toInt(),
  ];
}

class UsageBars extends StatelessWidget {
  final List<int> values;
  const UsageBars({super.key, required this.values});

  @override
  Widget build(BuildContext context) {
    if (values.isEmpty) return const SizedBox.shrink();
    final max = values.fold<int>(1, (m, v) => v > m ? v : m);
    return SizedBox(
      height: 96,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          for (final v in values)
            Expanded(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 2),
                child: Tooltip(
                  message: '$v tokens',
                  child: FractionallySizedBox(
                    heightFactor: (v / max).clamp(0.06, 1.0),
                    alignment: Alignment.bottomCenter,
                    child: Container(
                      decoration: BoxDecoration(
                        color: Theme.of(
                          context,
                        ).colorScheme.primary.withValues(alpha: .75),
                        borderRadius: BorderRadius.circular(4),
                      ),
                    ),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class TokenGauge extends StatelessWidget {
  final int tokens;
  final int budget;
  const TokenGauge({super.key, required this.tokens, this.budget = 8192});

  @override
  Widget build(BuildContext context) {
    final ratio = (tokens / budget).clamp(0.0, 1.0);
    final over = tokens > budget;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                'Dernier événement : $tokens / $budget tokens',
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ),
            if (over)
              const Text(
                'dépassé',
                style: TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w700,
                  color: Color(0xffff3b30),
                ),
              ),
          ],
        ),
        const SizedBox(height: 6),
        ClipRRect(
          borderRadius: BorderRadius.circular(999),
          child: LinearProgressIndicator(
            value: ratio,
            minHeight: 8,
            backgroundColor: Theme.of(
              context,
            ).colorScheme.onSurface.withValues(alpha: .08),
            color: over
                ? const Color(0xffff3b30)
                : Theme.of(context).colorScheme.primary,
          ),
        ),
      ],
    );
  }
}
