import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:flutter/material.dart';

/// Mirrors packages/ui-tokens/src/colors.ts and the desktop auth stylesheet.
abstract final class AroDesign {
  static const blue = Color(0xff0071e3);
  static const blueDark = Color(0xff0a84ff);
  static ThemeData theme(Brightness brightness, {bool oled = false}) {
    final dark = brightness == Brightness.dark;
    final bg = dark
        ? (oled ? Colors.black : const Color(0xff111115))
        : const Color(0xfff5f5f7);
    final surface = dark ? const Color(0xff16161c) : Colors.white;
    final text = dark ? const Color(0xfff5f5f7) : const Color(0xff1d1d1f);
    final secondary = dark ? const Color(0xff98989f) : const Color(0xff6e6e73);
    final border = dark
        ? Colors.white.withValues(alpha: .08)
        : Colors.black.withValues(alpha: .06);
    final scheme =
        ColorScheme.fromSeed(
          seedColor: dark ? blueDark : blue,
          brightness: brightness,
        ).copyWith(
          primary: dark ? blueDark : blue,
          surface: surface,
          onSurface: text,
          onSurfaceVariant: secondary,
          outlineVariant: border,
        );
    return ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      scaffoldBackgroundColor: bg,
      fontFamily: 'Inter',
      fontFamilyFallback: const ['SF Pro Text', 'Roboto', 'Helvetica', 'Arial'],
      visualDensity: VisualDensity.standard,
      iconTheme: IconThemeData(size: 18, color: secondary),
      textTheme: TextTheme(
        headlineLarge: TextStyle(
          fontSize: 32,
          height: 1.2,
          fontWeight: FontWeight.w600,
          letterSpacing: -1.1,
          color: text,
        ),
        headlineSmall: TextStyle(
          fontSize: 24,
          fontWeight: FontWeight.w600,
          letterSpacing: -.65,
          color: text,
        ),
        titleLarge: TextStyle(
          fontSize: 20,
          fontWeight: FontWeight.w600,
          letterSpacing: -.45,
          color: text,
        ),
        titleMedium: TextStyle(
          fontSize: 15,
          fontWeight: FontWeight.w600,
          color: text,
        ),
        bodyLarge: TextStyle(fontSize: 15, height: 1.55, color: text),
        bodyMedium: TextStyle(fontSize: 14, height: 1.45, color: text),
        bodySmall: TextStyle(fontSize: 12, height: 1.45, color: secondary),
      ),
      appBarTheme: AppBarTheme(
        backgroundColor: bg,
        foregroundColor: text,
        elevation: 0,
        scrolledUnderElevation: 0,
        centerTitle: false,
      ),
      dividerTheme: DividerThemeData(color: border, thickness: 1, space: 1),
      drawerTheme: DrawerThemeData(
        backgroundColor: surface,
        shape: const RoundedRectangleBorder(),
        width: 300,
      ),
      cardTheme: CardThemeData(
        color: surface,
        elevation: 0,
        margin: EdgeInsets.zero,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(16),
          side: BorderSide(color: border),
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: dark
            ? Colors.white.withValues(alpha: .04)
            : Colors.black.withValues(alpha: .025),
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 16,
          vertical: 15,
        ),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: BorderSide(color: border),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: BorderSide(color: border),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: BorderSide(color: scheme.primary, width: 1.5),
        ),
        hintStyle: TextStyle(fontSize: 14, color: secondary),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size(48, 48),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
          textStyle: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          minimumSize: const Size(48, 44),
          side: BorderSide(color: border),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
        ),
      ),
      listTileTheme: ListTileThemeData(
        iconColor: secondary,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
        contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 3),
      ),
      snackBarTheme: SnackBarThemeData(
        behavior: SnackBarBehavior.floating,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
    );
  }
}

class Brand extends StatelessWidget {
  final double size;
  final bool wordmark;
  const Brand({super.key, this.size = 34, this.wordmark = true});
  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      ClipRRect(
        borderRadius: BorderRadius.circular(size * .24),
        child: Image.asset(
          'assets/logo.png',
          width: size,
          height: size,
          semanticLabel: 'ARO',
        ),
      ),
      if (wordmark) ...[
        const SizedBox(width: 10),
        const Text(
          'ARO',
          style: TextStyle(
            fontSize: 19,
            letterSpacing: -.4,
            fontWeight: FontWeight.w600,
          ),
        ),
      ],
    ],
  );
}

class Notice extends StatelessWidget {
  final String text;
  final bool error;
  final VoidCallback? retry;
  const Notice(this.text, {super.key, this.error = false, this.retry});
  @override
  Widget build(BuildContext context) {
    final color = error
        ? Theme.of(context).colorScheme.error
        : Theme.of(context).colorScheme.primary;
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: color.withValues(alpha: .08),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            error ? LucideIcons.circleAlert : LucideIcons.info,
            size: 18,
            color: color,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              text,
              style: Theme.of(
                context,
              ).textTheme.bodySmall?.copyWith(color: color),
            ),
          ),
          if (retry != null)
            IconButton(
              onPressed: retry,
              icon: const Icon(LucideIcons.refreshCw),
              tooltip: 'Réessayer',
            ),
        ],
      ),
    );
  }
}

class EmptyState extends StatelessWidget {
  final IconData icon;
  final String title, subtitle;
  final Widget? action;
  const EmptyState({
    super.key,
    required this.icon,
    required this.title,
    required this.subtitle,
    this.action,
  });
  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.all(28),
    child: Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          padding: const EdgeInsets.all(18),
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.primary.withValues(alpha: .07),
            borderRadius: BorderRadius.circular(20),
          ),
          child: Icon(
            icon,
            size: 28,
            color: Theme.of(context).colorScheme.primary,
          ),
        ),
        const SizedBox(height: 20),
        Text(
          title,
          textAlign: TextAlign.center,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 8),
        Text(
          subtitle,
          textAlign: TextAlign.center,
          style: Theme.of(context).textTheme.bodySmall,
        ),
        if (action != null) ...[const SizedBox(height: 20), action!],
      ],
    ),
  );
}

Future<void> perform(
  BuildContext context,
  Future<void> Function() action, {
  String? success,
}) async {
  try {
    await action();
    if (context.mounted && success != null) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(success)));
    }
  } catch (e) {
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
    }
  }
}

Future<bool> confirmDelete(BuildContext context, String name) async =>
    await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Supprimer cet élément ?'),
        content: Text('« $name » sera supprimé. Cette action est définitive.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Annuler'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Supprimer'),
          ),
        ],
      ),
    ) ??
    false;
