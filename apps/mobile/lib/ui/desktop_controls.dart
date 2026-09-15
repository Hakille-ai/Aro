import 'package:flutter/material.dart';

/// Actions stay out of the reading path. Touch users reveal them with a long
/// press; keyboard and pointer users reveal them through focus and hover.
class QuietRow extends StatefulWidget {
  final Widget Function(bool showActions) builder;
  final bool selected;
  const QuietRow({super.key, required this.builder, this.selected = false});
  @override
  State<QuietRow> createState() => _QuietRowState();
}

class _QuietRowState extends State<QuietRow> {
  bool hover = false, focus = false, revealed = false;
  @override
  Widget build(BuildContext context) => MouseRegion(
    onEnter: (_) => setState(() => hover = true),
    onExit: (_) => setState(() {
      hover = false;
      revealed = false;
    }),
    child: Focus(
      onFocusChange: (value) => setState(() => focus = value),
      child: GestureDetector(
        onLongPress: () => setState(() => revealed = !revealed),
        child: widget.builder(
          hover ||
              focus ||
              revealed ||
              widget.selected,
        ),
      ),
    ),
  );
}

class DesktopTool extends StatelessWidget {
  final IconData icon;
  final String label;
  final VoidCallback? onPressed;
  final Color color;
  final bool filled;
  const DesktopTool({
    super.key,
    required this.icon,
    required this.label,
    this.onPressed,
    this.color = const Color(0xff2997ff),
    this.filled = false,
  });
  @override
  Widget build(BuildContext context) => SizedBox(
    width: 40,
    height: 40,
    child: IconButton(
      tooltip: label,
      onPressed: onPressed,
      style: IconButton.styleFrom(
        padding: EdgeInsets.zero,
        foregroundColor: filled ? Colors.white : color,
        backgroundColor: color.withValues(alpha: filled ? 1 : .055),
        disabledBackgroundColor: color.withValues(alpha: .25),
        side: BorderSide(color: color.withValues(alpha: .18)),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(11)),
      ),
      icon: Icon(icon, size: 19),
    ),
  );
}
