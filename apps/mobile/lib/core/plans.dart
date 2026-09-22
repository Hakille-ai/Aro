import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

/// Logique Plan de Travail — miroir mobile de `desktop/src/lib/plan-utils.ts`.
String cycleTaskStatus(String? current) => switch (current) {
      'pending' => 'in_progress',
      'in_progress' => 'completed',
      'completed' => 'error',
      'error' => 'pending',
      _ => 'pending',
    };

({int total, int completed, int percentage}) planProgress(List<dynamic> tasks) {
  if (tasks.isEmpty) return (total: 0, completed: 0, percentage: 0);
  final done = tasks.where((t) {
    final m = t is Map ? Map<String, dynamic>.from(t) : <String, dynamic>{};
    return m['completed'] == true || m['status'] == 'completed';
  }).length;
  return (
    total: tasks.length,
    completed: done,
    percentage: ((done / tasks.length) * 100).round(),
  );
}

String taskStatusLabel(String? status) => switch (status) {
      'in_progress' => 'En cours',
      'completed' => 'Terminée',
      'error' => 'Bloquée',
      _ => 'À faire',
    };

Color taskStatusColor(String? status) => switch (status) {
      'in_progress' => const Color(0xff0071e3),
      'completed' => const Color(0xff28c840),
      'error' => const Color(0xffff3b30),
      _ => const Color(0xff86868b),
    };

IconData taskStatusIcon(String? status) => switch (status) {
      'in_progress' => LucideIcons.loader,
      'completed' => LucideIcons.circleCheck,
      'error' => LucideIcons.circleAlert,
      _ => LucideIcons.circle,
    };
