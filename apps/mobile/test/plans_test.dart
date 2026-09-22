import 'package:flutter_test/flutter_test.dart';
import 'package:aro_mobile/core/plans.dart';

void main() {
  test('cycleTaskStatus follows desktop pending→in_progress→completed→error', () {
    expect(cycleTaskStatus('pending'), 'in_progress');
    expect(cycleTaskStatus('in_progress'), 'completed');
    expect(cycleTaskStatus('completed'), 'error');
    expect(cycleTaskStatus('error'), 'pending');
    expect(cycleTaskStatus(null), 'pending');
  });

  test('planProgress counts completed + status completed', () {
    final p = planProgress([
      {'status': 'completed'},
      {'completed': true},
      {'status': 'pending'},
      {'status': 'in_progress'},
    ]);
    expect(p.total, 4);
    expect(p.completed, 2);
    expect(p.percentage, 50);
  });

  test('planProgress empty is zero', () {
    final p = planProgress([]);
    expect((p.total, p.completed, p.percentage), (0, 0, 0));
  });
}
