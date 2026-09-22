import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'dart:async';
import 'dart:convert';

import 'package:crypto/crypto.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter_tts/flutter_tts.dart';
import 'package:speech_to_text/speech_to_text.dart';
import 'package:url_launcher/url_launcher.dart';

import '../core/api.dart';
import '../core/mentions.dart';
import '../core/workspace.dart';
import '../ui/design.dart';
import '../ui/desktop_controls.dart';
import '../core/personalities.dart';
import 'command_palette.dart';
import 'mention_sheet.dart';
import 'notifications.dart';
import 'settings.dart';
import 'settings_catalog.dart';
import 'workspace_panel.dart';

class HomePage extends StatefulWidget {
  final Workspace workspace;
  const HomePage({super.key, required this.workspace});
  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage>
    with WidgetsBindingObserver, TickerProviderStateMixin {
  final shell = GlobalKey<ScaffoldState>();
  final composer = TextEditingController();
  final scroll = ScrollController();
  final speech = SpeechToText();
  final tts = FlutterTts();
  String search = '', filter = '';
  bool uploading = false, listening = false;
  String? boundId;
  Workspace get w => widget.workspace;

  // --- Apple-style sidebars (gauche + droite, swipe partout) ---
  late final AnimationController _sidebar;
  late final AnimationController _rightSidebar;
  bool _edgeDrag = false;
  int _dragSide = 0; // -1 gauche, +1 droite

  // --- Messages : feedback local (comme web), copies, accordéons ---
  final Map<String, bool?> _feedback = {};
  final Set<String> _expandedSteps = {};
  final Set<String> _collapsedThinking = {};
  final Set<String> _expandedStepDetails = {};
  String? _copiedId;
  Timer? _copiedTimer;
  // Message dont les actions sont révélées (tap sur la bulle, façon iMessage).
  String? _actionsFor;
  // Trigger @mention actif dans le composer (miroir desktop mention-model).
  MentionTrigger? _mentionTrigger;

  void _toggleActions(String id) {
    HapticFeedback.selectionClick();
    setState(() {
      _actionsFor = _actionsFor == id ? null : id;
    });
  }

  void openSidebar() {
    closeRightSidebar(silent: true);
    HapticFeedback.lightImpact();
    _sidebar.animateTo(
      1,
      duration: const Duration(milliseconds: 320),
      curve: Curves.easeOutCubic,
    );
  }

  void closeSidebar() {
    if (_sidebar.value == 0) return;
    HapticFeedback.lightImpact();
    _sidebar.animateTo(
      0,
      duration: const Duration(milliseconds: 260),
      curve: Curves.easeOutCubic,
    );
  }

  void openRightSidebar() {
    closeSidebar();
    HapticFeedback.lightImpact();
    _rightSidebar.animateTo(
      1,
      duration: const Duration(milliseconds: 320),
      curve: Curves.easeOutCubic,
    );
  }

  void closeRightSidebar({bool silent = false}) {
    if (_rightSidebar.value == 0) return;
    if (!silent) HapticFeedback.lightImpact();
    _rightSidebar.animateTo(
      0,
      duration: const Duration(milliseconds: 260),
      curve: Curves.easeOutCubic,
    );
  }

  void closeSidebars() {
    closeSidebar();
    closeRightSidebar(silent: true);
  }

  void _onDragStart(DragStartDetails d) {
    // Swipe partout : droite -> gauche, gauche -> droite.
    _edgeDrag = true;
    _dragSide = 0;
    _sidebar.stop();
    _rightSidebar.stop();
    if (_sidebar.value == 0 && _rightSidebar.value == 0) {
      HapticFeedback.selectionClick();
    }
  }

  void _onDragUpdate(DragUpdateDetails d, double leftW, double rightW) {
    if (!_edgeDrag) return;
    final delta = d.primaryDelta ?? 0;
    if (delta == 0) return;
    if (_dragSide == 0) {
      if (_sidebar.value > 0.02) {
        _dragSide = -1;
      } else if (_rightSidebar.value > 0.02) {
        _dragSide = 1;
      } else {
        _dragSide = delta > 0 ? -1 : 1;
      }
    }
    if (_dragSide == -1) {
      // Sidebar gauche : + ouvre, - ferme. Bascule fluide vers droite si on repart à gauche.
      if (_sidebar.value <= 0 && delta < 0) {
        _dragSide = 1;
        _rightSidebar.value = (_rightSidebar.value - delta / rightW).clamp(
          0.0,
          1.0,
        );
      } else {
        _sidebar.value = (_sidebar.value + delta / leftW).clamp(0.0, 1.0);
      }
    } else {
      // Panneau droit plein écran : - ouvre (swipe gauche), + ferme. Miroir exact.
      if (_rightSidebar.value <= 0 && delta > 0) {
        _dragSide = -1;
        _sidebar.value = (_sidebar.value + delta / leftW).clamp(0.0, 1.0);
      } else {
        _rightSidebar.value = (_rightSidebar.value - delta / rightW).clamp(
          0.0,
          1.0,
        );
      }
    }
  }

  void _onDragEnd(DragEndDetails d) {
    if (!_edgeDrag) return;
    _edgeDrag = false;
    final v = d.primaryVelocity ?? 0;
    if (_dragSide == -1) {
      if (v > 550) {
        HapticFeedback.mediumImpact();
        openSidebar();
      } else if (v < -550) {
        closeSidebar();
      } else if (_sidebar.value > 0.42) {
        HapticFeedback.lightImpact();
        openSidebar();
      } else {
        closeSidebar();
      }
    } else if (_dragSide == 1) {
      // Miroir : vitesse négative (vers la gauche) ouvre.
      if (v < -550) {
        HapticFeedback.mediumImpact();
        openRightSidebar();
      } else if (v > 550) {
        closeRightSidebar();
      } else if (_rightSidebar.value > 0.42) {
        HapticFeedback.lightImpact();
        openRightSidebar();
      } else {
        closeRightSidebar();
      }
    }
    _dragSide = 0;
  }

  @override
  void initState() {
    super.initState();
    _sidebar = AnimationController(
      vsync: this,
      lowerBound: 0,
      upperBound: 1,
      value: 0,
      duration: const Duration(milliseconds: 320),
      reverseDuration: const Duration(milliseconds: 260),
    );
    _rightSidebar = AnimationController(
      vsync: this,
      lowerBound: 0,
      upperBound: 1,
      value: 0,
      duration: const Duration(milliseconds: 320),
      reverseDuration: const Duration(milliseconds: 260),
    );
    WidgetsBinding.instance.addObserver(this);
    composer.text = w.draft;
    boundId = w.activeId;
    w.addListener(changed);
  }

  void changed() {
    if (boundId != w.activeId ||
        (composer.text.isEmpty && w.draft.isNotEmpty && !w.sending)) {
      boundId = w.activeId;
      composer.text = w.draft;
    }
    if (mounted) setState(() {});
    if (w.sending) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (scroll.hasClients &&
            scroll.position.maxScrollExtent - scroll.offset < 250) {
          scroll.jumpTo(scroll.position.maxScrollExtent);
        }
      });
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.inactive) {
      speech.stop();
      w.saveDraft(composer.text);
    }
    if (state == AppLifecycleState.resumed &&
        !w.sending &&
        w.activeId != null) {
      w.select(w.activeId);
    }
  }

  @override
  void dispose() {
    _copiedTimer?.cancel();
    _sidebar.dispose();
    _rightSidebar.dispose();
    w.removeListener(changed);
    WidgetsBinding.instance.removeObserver(this);
    speech.cancel();
    tts.stop();
    composer.dispose();
    scroll.dispose();
    super.dispose();
  }

  void settings() => Navigator.push(
    context,
    MaterialPageRoute(builder: (_) => SettingsPage(workspace: w)),
  );
  Future<void> select(String? id) async {
    if (w.sending) return;
    _actionsFor = null;
    closeSidebars();
    await w.saveDraft(composer.text);
    await w.select(id);
    if (mounted) {
      composer.text = w.draft;
    }
  }

  Future<void> send() async {
    final text = composer.text;
    composer.clear();
    _actionsFor = null;
    _mentionTrigger = null;
    await w.send(text);
    if (!mounted) return;
    if (w.error != null) {
      composer.text = w.draft;
    } else if (w.preferences.getBool('aro.speak') == true &&
        w.messages.isNotEmpty) {
      await tts.setLanguage('fr-FR');
      await tts.speak('${w.messages.last['content']}');
    }
  }

  /// Régénère la dernière réponse en renvoyant le dernier message
  /// utilisateur comme nouveau tour (le cloud n'expose pas de route
  /// `/regenerate` : aucun faux endpoint, l'historique reste linéaire).
  Future<void> regenerate() async {
    if (w.sending) return;
    String? lastUser;
    for (var i = w.messages.length - 1; i >= 0; i--) {
      if (w.messages[i]['role'] == 'user' &&
          '${w.messages[i]['content'] ?? ''}'.trim().isNotEmpty) {
        lastUser = '${w.messages[i]['content']}';
        break;
      }
    }
    if (lastUser == null || lastUser.trim().isEmpty) return;
    HapticFeedback.mediumImpact();
    _actionsFor = null;
    await w.send(lastUser);
    if (!mounted) return;
    if (w.error != null) {
      composer.text = w.draft;
    }
  }

  void _onComposerChanged(String value) {
    w.saveDraft(value);
    final cursor = composer.selection.baseOffset < 0
        ? value.length
        : composer.selection.baseOffset;
    final trigger = detectMentionQuery(value, cursor);
    if ('${trigger?.query}' != '${_mentionTrigger?.query}' ||
        trigger?.startIndex != _mentionTrigger?.startIndex) {
      setState(() => _mentionTrigger = trigger);
    }
  }

  Future<void> _openMentions({String? initialQuery}) async {
    final trigger = _mentionTrigger;
    final picked = await openMentionSheet(
      context: context,
      workspace: w,
      initialQuery: initialQuery ?? trigger?.query ?? '',
    );
    if (picked == null || !mounted) return;
    if (trigger != null &&
        trigger.startIndex >= 0 &&
        trigger.endIndex <= composer.text.length) {
      final applied = applyMentionSelection(composer.text, trigger, picked);
      composer.text = applied.newText;
      composer.selection = TextSelection.collapsed(offset: applied.newCursor);
    } else {
      final cursor = composer.selection.baseOffset < 0
          ? composer.text.length
          : composer.selection.baseOffset;
      final before = composer.text.substring(0, cursor);
      final after = composer.text.substring(cursor);
      final sep = before.isEmpty || before.endsWith(' ') ? '' : ' ';
      composer.text = '$before$sep@$picked $after';
    }
    w.saveDraft(composer.text);
    setState(() => _mentionTrigger = null);
  }

  // ============ Header contextuel (miroir ConversationTopbar, en mieux) ============
  // Titre de la conversation + fil d'Ariane projet/dossier (le web oublie le
  // dossier) + profil actif. Tap sur le titre = actions (renommer, classer,
  // supprimer). Tap sur le profil = changer de profil.
  Json? get _headerProject {
    final c = w.current;
    if (c == null || c['projectId'] == null) return null;
    return w.projects
        .where((p) => '${p['id']}' == '${c['projectId']}')
        .firstOrNull;
  }

  Json? get _headerFolder {
    final c = w.current;
    if (c == null || c['folderId'] == null) return null;
    return w.folders
        .where((f) => '${f['id']}' == '${c['folderId']}')
        .firstOrNull;
  }

  Future<void> _openConversationActions() async {
    final c = w.current;
    if (c == null || w.sending) return;
    HapticFeedback.selectionClick();
    final action = await showModalBottomSheet<String>(
      context: context,
      showDragHandle: true,
      backgroundColor: Theme.of(context).colorScheme.surface,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(24)),
      ),
      builder: (sheetCtx) => SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 8, 20, 24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                '${c['title'] ?? 'Conversation'}',
                textAlign: TextAlign.center,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: Theme.of(context).textTheme.titleMedium?.copyWith(
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  letterSpacing: -0.3,
                ),
              ),
              const SizedBox(height: 14),
              _sheetAction(
                icon: LucideIcons.pencil,
                label: 'Renommer',
                onTap: () => Navigator.pop(sheetCtx, 'rename'),
              ),
              _sheetAction(
                icon: LucideIcons.folderKanban,
                label: 'Classer / Déplacer vers…',
                onTap: () => Navigator.pop(sheetCtx, 'move'),
              ),
              _sheetAction(
                icon: LucideIcons.trash2,
                label: 'Supprimer',
                danger: true,
                onTap: () => Navigator.pop(sheetCtx, 'delete'),
              ),
            ],
          ),
        ),
      ),
    );
    if (action != null && mounted) {
      await conversationAction(Map<String, dynamic>.from(c), action);
    }
  }

  Widget _sheetAction({
    required IconData icon,
    required String label,
    bool danger = false,
    required VoidCallback onTap,
  }) {
    final scheme = Theme.of(context).colorScheme;
    final color = danger ? const Color(0xffe74c3c) : scheme.onSurface;
    return Material(
      color: Colors.transparent,
      child: InkWell(
        borderRadius: BorderRadius.circular(13),
        onTap: () {
          HapticFeedback.selectionClick();
          onTap();
        },
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 13),
          child: Row(
            children: [
              Icon(icon, size: 18, color: color),
              const SizedBox(width: 13),
              Text(
                label,
                style: TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w500,
                  letterSpacing: -0.2,
                  color: color,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _headerTitleBlock() {
    final c = w.current;
    // Nouvelle conversation : on reste épuré (comme le web, titre vide).
    if (w.activeId == null || c == null) {
      return const Expanded(child: SizedBox.shrink());
    }
    final scheme = Theme.of(context).colorScheme;
    final project = _headerProject;
    final folder = _headerFolder;
    final profile = w.personality;
    const subStyle = TextStyle(
      fontSize: 11.5,
      color: Color(0xff86868b),
      fontWeight: FontWeight.w500,
    );
    IconData profileIcon(Json p) => switch (p['icon']) {
      'code' => LucideIcons.terminal,
      'check' => LucideIcons.check,
      'edit-2' => LucideIcons.pencil,
      _ => LucideIcons.bot,
    };
    return Expanded(
      child: GestureDetector(
        behavior: HitTestBehavior.translucent,
        onTap: _openConversationActions,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Flexible(
                  child: Text(
                    '${c['title'] ?? 'Conversation'}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context).textTheme.titleMedium?.copyWith(
                      fontSize: 15,
                      fontWeight: FontWeight.w700,
                      letterSpacing: -0.3,
                    ),
                  ),
                ),
                const SizedBox(width: 4),
                Icon(
                  LucideIcons.chevronDown,
                  size: 13,
                  color: scheme.onSurface.withValues(alpha: 0.45),
                ),
              ],
            ),
            const SizedBox(height: 2),
            Row(
              children: [
                if (project != null) ...[
                  Container(
                    width: 7,
                    height: 7,
                    decoration: BoxDecoration(
                      color: projectColor(project),
                      shape: BoxShape.circle,
                    ),
                  ),
                  const SizedBox(width: 5),
                  Flexible(
                    child: Text(
                      '${project['name']}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: subStyle.copyWith(fontWeight: FontWeight.w600),
                    ),
                  ),
                ],
                if (folder != null) ...[
                  if (project != null)
                    const Padding(
                      padding: EdgeInsets.symmetric(horizontal: 5),
                      child: Text('›', style: subStyle),
                    ),
                  Icon(
                    LucideIcons.folder,
                    size: 11,
                    color: const Color(0xff00b887),
                  ),
                  const SizedBox(width: 4),
                  Flexible(
                    child: Text(
                      '${folder['name']}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: subStyle,
                    ),
                  ),
                ],
                if (profile != null) ...[
                  if (project != null || folder != null)
                    Container(
                      width: 3,
                      height: 3,
                      margin: const EdgeInsets.symmetric(horizontal: 7),
                      decoration: const BoxDecoration(
                        color: Color(0xffc7c7cc),
                        shape: BoxShape.circle,
                      ),
                    ),
                  GestureDetector(
                    onTap: () => perform(context, choosePersonality),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(
                          profileIcon(profile),
                          size: 11,
                          color: scheme.primary,
                        ),
                        const SizedBox(width: 4),
                        Text(
                          '${profile['name'] ?? 'Profil'}',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: subStyle.copyWith(color: scheme.primary),
                        ),
                      ],
                    ),
                  ),
                ],
                if (project == null &&
                    folder == null &&
                    profile == null)
                  const Text('Indépendante', style: subStyle),
              ],
            ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) => CallbackShortcuts(
    bindings: {
      const SingleActivator(LogicalKeyboardKey.keyN, control: true): () =>
          select(null),
      const SingleActivator(LogicalKeyboardKey.keyN, meta: true): () =>
          select(null),
      const SingleActivator(LogicalKeyboardKey.comma, control: true): settings,
      const SingleActivator(LogicalKeyboardKey.comma, meta: true): settings,
      const SingleActivator(LogicalKeyboardKey.keyK, control: true): () =>
          openCommandPalette(
            context: context,
            workspace: w,
            onSelectConversation: select,
            onNewConversation: () => select(null),
            onOpenWorkspace: openRightSidebar,
          ),
      const SingleActivator(LogicalKeyboardKey.keyK, meta: true): () =>
          openCommandPalette(
            context: context,
            workspace: w,
            onSelectConversation: select,
            onNewConversation: () => select(null),
            onOpenWorkspace: openRightSidebar,
          ),
    },
    child: Focus(
      autofocus: true,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final wide = constraints.maxWidth >= 900;
          final screenW = MediaQuery.sizeOf(context).width;
          final drawerWidth = screenW * 0.84 < 280
              ? 280.0
              : screenW * 0.84 > 340
              ? 340.0
              : screenW * 0.84;

          Widget mainColumn() => Column(
            children: [
              SizedBox(
                height: 64,
                child: Row(
                  children: [
                    if (!wide)
                      IconButton(
                        tooltip: 'Ouvrir la barre latérale',
                        onPressed: openSidebar,
                        icon: const Icon(LucideIcons.panelLeft),
                      ),
                    if (wide) const SizedBox(width: 24),
                    _headerTitleBlock(),
                    IconButton(
                      tooltip: 'Recherche globale (Ctrl K)',
                      onPressed: () => openCommandPalette(
                        context: context,
                        workspace: w,
                        onSelectConversation: select,
                        onNewConversation: () => select(null),
                        onOpenWorkspace: openRightSidebar,
                      ),
                      icon: const Icon(LucideIcons.search, size: 20),
                    ),
                    IconButton(
                      tooltip: 'Nouvelle conversation',
                      onPressed: w.sending ? null : () => select(null),
                      icon: const Icon(LucideIcons.squarePen, size: 20),
                    ),
                    NotificationBell(workspace: w),
                    IconButton(
                      tooltip: 'Plan, fichiers et agents',
                      onPressed: openRightSidebar,
                      icon: const Icon(LucideIcons.panelRight, size: 21),
                    ),
                    const SizedBox(width: 8),
                  ],
                ),
              ),
              const Divider(),
              if (w.loading) const LinearProgressIndicator(minHeight: 2),
              if (w.error != null)
                Padding(
                  padding: const EdgeInsets.all(12),
                  child: _isOfflineError(w.error!)
                      ? Container(
                          padding: const EdgeInsets.all(14),
                          decoration: BoxDecoration(
                            color: const Color(0xffff9500).withValues(alpha: .1),
                            borderRadius: BorderRadius.circular(12),
                            border: Border.all(
                              color: const Color(
                                0xffff9500,
                              ).withValues(alpha: .3),
                            ),
                          ),
                          child: Row(
                            children: [
                              const Icon(
                                LucideIcons.wifiOff,
                                size: 18,
                                color: Color(0xffff9500),
                              ),
                              const SizedBox(width: 10),
                              const Expanded(
                                child: Text(
                                  'Hors-ligne : vos brouillons sont conservés sur ce téléphone.',
                                  style: TextStyle(
                                    fontSize: 12,
                                    color: Color(0xffff9500),
                                  ),
                                ),
                              ),
                              IconButton(
                                tooltip: 'Réessayer',
                                onPressed: () => w.activeId == null
                                    ? w.refresh()
                                    : w.select(w.activeId),
                                icon: const Icon(LucideIcons.refreshCw),
                              ),
                            ],
                          ),
                        )
                      : Notice(
                          w.error!,
                          error: true,
                          retry: () => w.activeId == null
                              ? w.refresh()
                              : w.select(w.activeId),
                        ),
                ),
              // Banniere IA compacte (2 lignes max) : sur petit ecran + clavier
              // ouvert, une notice detaillee deborderait. Le detail vit dans
              // Reglages › Modeles ; l'erreur d'envoi reste detaillee.
              if (w.aiStatusKnown && !w.aiCanGenerate)
                Padding(
                  padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
                  child: Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 14,
                      vertical: 10,
                    ),
                    decoration: BoxDecoration(
                      color: Theme.of(
                        context,
                      ).colorScheme.error.withValues(alpha: .08),
                      borderRadius: BorderRadius.circular(12),
                    ),
                    child: Row(
                      children: [
                        Icon(
                          LucideIcons.circleAlert,
                          size: 16,
                          color: Theme.of(context).colorScheme.error,
                        ),
                        const SizedBox(width: 10),
                        Expanded(
                          child: Text(
                            w.aiGuidanceShort(),
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              fontSize: 12,
                              color: Theme.of(context).colorScheme.error,
                            ),
                          ),
                        ),
                        IconButton(
                          tooltip: 'Réessayer',
                          onPressed: () => w.refreshAiStatus(),
                          icon: const Icon(LucideIcons.refreshCw, size: 16),
                          color: Theme.of(context).colorScheme.error,
                        ),
                      ],
                    ),
                  ),
                ),
              Expanded(
                child: w.messages.isEmpty ? emptyChat() : messageList(),
              ),
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 880),
                child: composerView(),
              ),
            ],
          );

          // --- Apple : sidebars gauche + droite avec swipe partout ---
          // Gauche : swipe droite pour ouvrir. Droite : swipe gauche pour ouvrir.
          // Miroir exact : même physique, mêmes ombres, mêmes durées.
          Widget rightPanel() => Container(
            decoration: BoxDecoration(
              color: Theme.of(context).colorScheme.surface,
              borderRadius: const BorderRadius.only(
                topLeft: Radius.circular(22),
                bottomLeft: Radius.circular(22),
              ),
              boxShadow: const [],
            ),
            child: ClipRRect(
              borderRadius: const BorderRadius.only(
                topLeft: Radius.circular(22),
                bottomLeft: Radius.circular(22),
              ),
              child: WorkspacePanel(workspace: w, onClose: closeRightSidebar),
            ),
          );

          Widget appleScaffold({required bool isWide}) => ListenableBuilder(
            listenable: Listenable.merge([_sidebar, _rightSidebar]),
            builder: (context, _) {
              final pL = _sidebar.value.clamp(0.0, 1.0);
              final pR = _rightSidebar.value.clamp(0.0, 1.0);
              final p = pL > pR ? pL : pR;
              final scale = 1 - 0.075 * p;
              final radius = 26 * p;
              // Gauche pousse vers la droite, droite pousse vers la gauche (miroir).
              final mainDx =
                  pL * drawerWidth * 0.22 - pR * drawerWidth * 0.22;
              final scrimOpacity = 0.38 * p;
              final leftDx = -drawerWidth * (1 - pL);
              // Panneau droit : plein écran, suit le doigt 1:1.
              final rightW = screenW;
              final rightDx = rightW * (1 - pR);

              Widget mainContent() {
                if (isWide) {
                  return Row(
                    children: [
                      SizedBox(width: 280, child: sidebarPanel(wide: true)),
                      const VerticalDivider(width: 1),
                      Expanded(child: mainColumn()),
                    ],
                  );
                }
                return mainColumn();
              }

              return PopScope(
                canPop: p < 0.05,
                onPopInvokedWithResult: (didPop, _) {
                  if (!didPop) closeSidebars();
                },
                child: Scaffold(
                  key: shell,
                  drawerEnableOpenDragGesture: false,
                  body: SafeArea(
                    child: GestureDetector(
                      behavior: HitTestBehavior.translucent,
                      onHorizontalDragStart: _onDragStart,
                      onHorizontalDragUpdate: (d) {
                        // En wide la gauche est fixe : on ignore le swipe droite.
                        if (isWide &&
                            _sidebar.value < 0.02 &&
                            _rightSidebar.value < 0.02 &&
                            (d.primaryDelta ?? 0) > 0) {
                          return;
                        }
                        _onDragUpdate(d, drawerWidth, rightW);
                      },
                      onHorizontalDragEnd: _onDragEnd,
                      child: Stack(
                        children: [
                          // Contenu principal : scale + coins arrondis façon iOS.
                          Transform.translate(
                            offset: Offset(mainDx, 0),
                            child: Transform.scale(
                              scale: scale,
                              alignment: Alignment.center,
                              child: ClipRRect(
                                borderRadius: BorderRadius.circular(radius),
                                child: AbsorbPointer(
                                  absorbing: p > 0.08,
                                  child: Container(
                                    color: Theme.of(
                                      context,
                                    ).scaffoldBackgroundColor,
                                    child: mainContent(),
                                  ),
                                ),
                              ),
                            ),
                          ),
                          // Voile sombre premium.
                          if (p > 0.01)
                            Positioned.fill(
                              child: GestureDetector(
                                onTap: closeSidebars,
                                child: Container(
                                  decoration: BoxDecoration(
                                    color: Colors.black.withValues(
                                      alpha: scrimOpacity,
                                    ),
                                    borderRadius: BorderRadius.circular(radius),
                                  ),
                                ),
                              ),
                            ),
                          // Sidebar gauche qui suit le doigt.
                          if (!isWide)
                            Positioned(
                              left: 0,
                              top: 0,
                              bottom: 0,
                              width: drawerWidth,
                              child: Transform.translate(
                                offset: Offset(leftDx, 0),
                                child: GestureDetector(
                                  onHorizontalDragStart: _onDragStart,
                                  onHorizontalDragUpdate: (d) =>
                                      _onDragUpdate(d, drawerWidth, rightW),
                                  onHorizontalDragEnd: _onDragEnd,
                                  child: Opacity(
                                    opacity: (0.35 + 0.65 * pL).clamp(0.0, 1.0),
                                    child: Container(
                                      decoration: BoxDecoration(
                                        color: Theme.of(
                                          context,
                                        ).colorScheme.surface,
                                        borderRadius: const BorderRadius.only(
                                          topRight: Radius.circular(22),
                                          bottomRight: Radius.circular(22),
                                        ),
                                        boxShadow: [
                                          BoxShadow(
                                            color: Colors.black.withValues(
                                              alpha: 0.28 * pL,
                                            ),
                                            blurRadius: 48,
                                            offset: const Offset(16, 0),
                                          ),
                                        ],
                                      ),
                                      child: ClipRRect(
                                        borderRadius: const BorderRadius.only(
                                          topRight: Radius.circular(22),
                                          bottomRight: Radius.circular(22),
                                        ),
                                        child: sidebarPanel(wide: isWide),
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                            ),
                          // Panneau droit plein écran : miroir de la gauche.
                          Positioned(
                            right: 0,
                            top: 0,
                            bottom: 0,
                            width: rightW,
                            child: Transform.translate(
                              offset: Offset(rightDx, 0),
                              child: GestureDetector(
                                onHorizontalDragStart: _onDragStart,
                                onHorizontalDragUpdate: (d) =>
                                    _onDragUpdate(d, drawerWidth, rightW),
                                onHorizontalDragEnd: _onDragEnd,
                                child: Opacity(
                                  opacity: (0.35 + 0.65 * pR).clamp(0.0, 1.0),
                                  child: Container(
                                    decoration: BoxDecoration(
                                      color: Theme.of(
                                        context,
                                      ).colorScheme.surface,
                                      borderRadius: const BorderRadius.only(
                                        topLeft: Radius.circular(22),
                                        bottomLeft: Radius.circular(22),
                                      ),
                                      boxShadow: [
                                        BoxShadow(
                                          color: Colors.black.withValues(
                                            alpha: 0.28 * pR,
                                          ),
                                          blurRadius: 48,
                                          offset: const Offset(-16, 0),
                                        ),
                                      ],
                                    ),
                                    child: rightPanel(),
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              );
            },
          );

          if (wide) {
            return appleScaffold(isWide: true);
          }

          return appleScaffold(isWide: false);
        },
      ),
    ),
  );

  /// Sidebar Apple : aérée, hiérarchisée, tactile (44px min).
  Widget sidebarPanel({required bool wide}) {
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: scheme.surface,
      child: SafeArea(
        bottom: false,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Header premium : logo + fermeture iOS.
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 12, 10),
              child: Row(
                children: [
                  const Brand(size: 28),
                  const Spacer(),
                  if (!wide)
                    Material(
                      color: scheme.onSurface.withValues(alpha: 0.06),
                      borderRadius: BorderRadius.circular(11),
                      child: InkWell(
                        borderRadius: BorderRadius.circular(11),
                        onTap: closeSidebar,
                        child: const SizedBox(
                          width: 34,
                          height: 34,
                          child: Icon(LucideIcons.x, size: 16),
                        ),
                      ),
                    )
                  else
                    const SizedBox.shrink(),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12),
              child: Column(
                children: [
                  // CTA principal façon Apple : dégradé, ombre, 48px.
                  Material(
                    color: Colors.transparent,
                    child: InkWell(
                      borderRadius: BorderRadius.circular(15),
                      onTap: w.sending ? null : () => select(null),
                      child: Ink(
                        height: 48,
                        decoration: BoxDecoration(
                          gradient: w.sending
                              ? null
                              : const LinearGradient(
                                  begin: Alignment.topCenter,
                                  end: Alignment.bottomCenter,
                                  colors: [
                                    Color(0xff2fa2ff),
                                    Color(0xff0071e3),
                                  ],
                                ),
                          color: w.sending
                              ? scheme.onSurface.withValues(alpha: 0.08)
                              : null,
                          borderRadius: BorderRadius.circular(15),
                          boxShadow: w.sending
                              ? null
                              : [
                                  BoxShadow(
                                    color: const Color(
                                      0xff0071e3,
                                    ).withValues(alpha: 0.32),
                                    blurRadius: 16,
                                    offset: const Offset(0, 6),
                                  ),
                                ],
                        ),
                        child: Center(
                          child: Row(
                            mainAxisSize: MainAxisSize.min,
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              Icon(
                                LucideIcons.squarePen,
                                size: 16,
                                color: w.sending
                                    ? scheme.onSurface.withValues(alpha: 0.4)
                                    : Colors.white,
                              ),
                              const SizedBox(width: 9),
                              Flexible(
                                child: Text(
                                  'Nouvelle conversation',
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                    fontSize: 14.5,
                                    fontWeight: FontWeight.w600,
                                    letterSpacing: -0.2,
                                    color: w.sending
                                        ? scheme.onSurface.withValues(
                                            alpha: 0.4,
                                          )
                                        : Colors.white,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 10),
                  // Recherche style iOS.
                  Container(
                    height: 42,
                    decoration: BoxDecoration(
                      color: scheme.onSurface.withValues(alpha: 0.055),
                      borderRadius: BorderRadius.circular(13),
                    ),
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                    child: Row(
                      children: [
                        Icon(
                          LucideIcons.search,
                          size: 16,
                          color: scheme.onSurface.withValues(alpha: 0.45),
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: TextField(
                            onChanged: (v) => setState(() => search = v),
                            style: const TextStyle(fontSize: 14),
                            decoration: InputDecoration(
                              hintText: 'Rechercher',
                              hintStyle: TextStyle(
                                fontSize: 14,
                                color: scheme.onSurface.withValues(alpha: 0.42),
                              ),
                              filled: false,
                              border: InputBorder.none,
                              enabledBorder: InputBorder.none,
                              focusedBorder: InputBorder.none,
                              isDense: true,
                              contentPadding: EdgeInsets.zero,
                            ),
                          ),
                        ),
                        if (search.isNotEmpty)
                          GestureDetector(
                            onTap: () => setState(() => search = ''),
                            child: Icon(
                              LucideIcons.x,
                              size: 15,
                              color: scheme.onSurface.withValues(alpha: 0.4),
                            ),
                          ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 6),
            Expanded(
              child: ScrollConfiguration(
                behavior: const ScrollBehavior().copyWith(overscroll: false),
                child: ListView(
                  physics: const BouncingScrollPhysics(
                    parent: AlwaysScrollableScrollPhysics(),
                  ),
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  children: [
                    sectionHeader('PROJETS', () => createContainer('projects')),
                    if (search.isEmpty) ...[
                      for (final project in ordered(w.projects, 'projects'))
                        projectTile(project),
                      for (final folder in ordered(
                        w.folders
                            .where((f) => f['projectId'] == null)
                            .toList(),
                        'folders',
                      ))
                        folderTile(folder),
                    ],
                    sectionHeader(
                      search.isEmpty ? 'CONVERSATIONS' : 'RÉSULTATS',
                      null,
                    ),
                    for (final c in matching(
                      search.isEmpty
                          ? w.conversations.where(
                              (c) =>
                                  c['projectId'] == null &&
                                  c['folderId'] == null,
                            )
                          : w.conversations,
                    ))
                      conversationTile(c),
                    if (w.conversations.isEmpty)
                      const Padding(
                        padding: EdgeInsets.all(16),
                        child: Text(
                          'Vos conversations apparaîtront ici.',
                          style: TextStyle(
                            fontSize: 13,
                            color: Color(0xff86868b),
                          ),
                        ),
                      ),
                    const SizedBox(height: 12),
                  ],
                ),
              ),
            ),
            // Bas premium : carte org + réglages.
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 4, 12, 6),
              child: Material(
                color: scheme.onSurface.withValues(alpha: 0.045),
                borderRadius: BorderRadius.circular(15),
                child: InkWell(
                  borderRadius: BorderRadius.circular(15),
                  onTap: () => Navigator.push(
                    context,
                    MaterialPageRoute(
                      builder: (_) => SettingsDetail(
                        workspace: w,
                        section: settingsSections.firstWhere(
                          (s) => s.id == 'organization',
                        ),
                      ),
                    ),
                  ),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 11,
                    ),
                    child: Row(
                      children: [
                        Container(
                          width: 30,
                          height: 30,
                          decoration: BoxDecoration(
                            color: const Color(
                              0xff22c55e,
                            ).withValues(alpha: 0.14),
                            borderRadius: BorderRadius.circular(9),
                          ),
                          child: const Icon(
                            LucideIcons.database,
                            size: 15,
                            color: Color(0xff16a34a),
                          ),
                        ),
                        const SizedBox(width: 10),
                        Expanded(
                          child: Text(
                            w.orgName,
                            style: TextStyle(
                              fontSize: 13.5,
                              fontWeight: FontWeight.w600,
                              color: scheme.onSurface,
                            ),
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        Icon(
                          LucideIcons.chevronsUpDown,
                          size: 14,
                          color: scheme.onSurface.withValues(alpha: 0.35),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 0, 12, 12),
              child: Material(
                color: Colors.transparent,
                child: InkWell(
                  borderRadius: BorderRadius.circular(13),
                  onTap: settings,
                  child: const Padding(
                    padding: EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 12,
                    ),
                    child: Row(
                      children: [
                        Icon(LucideIcons.settings, size: 17),
                        SizedBox(width: 11),
                        Text(
                          'Paramètres',
                          style: TextStyle(
                            fontSize: 14,
                            fontWeight: FontWeight.w500,
                          ),
                        ),
                        Spacer(),
                        Icon(LucideIcons.chevronRight, size: 15),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
  Widget projectTile(Json project) => QuietRow(
    builder: (show) => ExpansionTile(
      key: PageStorageKey('project-${project['id']}'),
      initiallyExpanded: true,
      controlAffinity: ListTileControlAffinity.leading,
      minTileHeight: 36,
      dense: true,
      visualDensity: VisualDensity.compact,
      tilePadding: const EdgeInsets.only(right: 4),
      shape: const Border(),
      collapsedShape: const Border(),
      title: Row(
        children: [
          Container(
            width: 6,
            height: 6,
            decoration: BoxDecoration(
              color: projectColor(project),
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              '${project['name']}',
              style: const TextStyle(fontSize: 12),
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
      trailing: show
          ? containerMenu(project, 'projects')
          : countBadge(
              w.conversations
                  .where((c) => c['projectId'] == project['id'])
                  .length,
            ),
      children: [
        for (final folder in ordered(
          w.folders.where((f) => f['projectId'] == project['id']).toList(),
          'folders',
        ))
          folderTile(folder),
        for (final c in matching(
          w.conversations.where(
            (c) => c['projectId'] == project['id'] && c['folderId'] == null,
          ),
        ))
          conversationTile(c, inset: 12),
        if (show)
          ListTile(
            dense: true,
            title: const Text(
              'Ajouter un dossier',
              style: TextStyle(fontSize: 12),
            ),
            leading: const Icon(LucideIcons.folderPlus, size: 14),
            onTap: () => createContainer('folders', projectId: project['id']),
          ),
      ],
    ),
  );
  Color projectColor(Json project) {
    final raw = '${project['color'] ?? '#0071e3'}'.replaceFirst('#', '');
    return Color(int.tryParse('ff$raw', radix: 16) ?? 0xff0071e3);
  }

  Iterable<Json> matching(Iterable<Json> list) => list.where(
    (c) => '${c['title']}'.toLowerCase().contains(search.toLowerCase()),
  );
  List<Json> ordered(List<Json> list, String kind) {
    final order =
        w.preferences.getStringList('aro.order.${w.api.accountKey}.$kind') ??
        [];
    return List.of(list)..sort((a, b) {
      final ai = order.indexOf('${a['id']}'), bi = order.indexOf('${b['id']}');
      return (ai < 0 ? 999999 : ai).compareTo(bi < 0 ? 999999 : bi);
    });
  }

  Widget sectionHeader(String title, VoidCallback? add) => Padding(
    padding: const EdgeInsets.only(left: 6, top: 12, bottom: 4),
    child: Row(
      children: [
        Text(
          title,
          style: const TextStyle(
            fontSize: 10,
            fontWeight: FontWeight.w600,
            letterSpacing: .5,
            color: Color(0xff86868b),
          ),
        ),
        if (title == 'CONVERSATIONS') ...[
          const SizedBox(width: 6),
          countBadge(w.conversations.length),
        ],
        const Spacer(),
        if (title == 'PROJETS')
          IconButton(
            tooltip: 'Ajouter dossiers',
            onPressed: () => createContainer('folders'),
            icon: const Icon(LucideIcons.folderPlus, size: 14),
          ),
        if (add != null)
          IconButton(
            tooltip: 'Ajouter ${title.toLowerCase()}',
            onPressed: w.sending ? null : add,
            icon: const Icon(LucideIcons.folderKanban, size: 14),
          ),
      ],
    ),
  );
  Widget folderTile(Json folder) => Padding(
    padding: const EdgeInsets.only(left: 8),
    child: QuietRow(
      builder: (show) => ExpansionTile(
        key: PageStorageKey('folder-${folder['id']}'),
        initiallyExpanded: true,
        controlAffinity: ListTileControlAffinity.leading,
        minTileHeight: 36,
        dense: true,
        visualDensity: VisualDensity.compact,
        tilePadding: const EdgeInsets.only(right: 4),
        shape: const Border(),
        collapsedShape: const Border(),
        title: Row(
          children: [
            const Icon(
              LucideIcons.folderOpen,
              size: 14,
              color: Color(0xff00b887),
            ),
            const SizedBox(width: 6),
            Expanded(
              child: Text(
                '${folder['name']}',
                style: const TextStyle(fontSize: 12),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
        trailing: show
            ? containerMenu(folder, 'folders')
            : countBadge(
                w.conversations
                    .where((c) => c['folderId'] == folder['id'])
                    .length,
              ),
        children: [
          for (final c in matching(
            w.conversations.where((c) => c['folderId'] == folder['id']),
          ))
            conversationTile(c, inset: 12),
        ],
      ),
    ),
  );
  Widget countBadge(int count) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
    decoration: BoxDecoration(
      color: Theme.of(context).colorScheme.onSurface.withValues(alpha: .05),
      borderRadius: BorderRadius.circular(9),
    ),
    child: Text(
      '$count',
      style: const TextStyle(fontSize: 10, color: Color(0xff86868b)),
    ),
  );
  String relativeTime(Json item) {
    final date = DateTime.tryParse(
      '${item['updatedAt'] ?? item['createdAt'] ?? ''}',
    );
    if (date == null) return '';
    final delta = DateTime.now().difference(date);
    if (delta.inMinutes < 60) return '${delta.inMinutes.clamp(0, 59)}min';
    if (delta.inHours < 24) return '${delta.inHours}h';
    if (delta.inDays < 7) return '${delta.inDays}j';
    return '${delta.inDays ~/ 7}sem';
  }

  Widget conversationTile(Json conversation, {double inset = 0}) => Padding(
    padding: EdgeInsets.only(left: inset, bottom: 2),
    child: QuietRow(
      selected: w.activeId == conversation['id'],
      builder: (show) => Container(
        decoration: BoxDecoration(
          border: Border(
            left: BorderSide(
              width: 3,
              color: w.activeId == conversation['id']
                  ? Theme.of(context).colorScheme.primary
                  : Colors.transparent,
            ),
          ),
        ),
        child: ListTile(
          selected: w.activeId == conversation['id'],
          selectedTileColor: Theme.of(
            context,
          ).colorScheme.onSurface.withValues(alpha: .045),
          dense: true,
          minTileHeight: 36,
          horizontalTitleGap: 10,
          contentPadding: const EdgeInsets.only(left: 7, right: 4),
          leading: Icon(
            LucideIcons.messageSquare,
            size: 14,
            color: w.activeId == conversation['id']
                ? Theme.of(context).colorScheme.primary
                : const Color(0xffa1a7b1),
          ),
          title: Text(
            '${conversation['title']}',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(fontSize: 12),
          ),
          onTap: w.sending ? null : () => select(conversation['id']),
          trailing: show
              ? PopupMenuButton<String>(
                  tooltip: 'Organiser la conversation',
                  icon: const Icon(LucideIcons.ellipsis, size: 16),
                  onSelected: (action) =>
                      conversationAction(conversation, action),
                  itemBuilder: (_) => const [
                    PopupMenuItem(value: 'rename', child: Text('Renommer')),
                    PopupMenuItem(value: 'move', child: Text('Déplacer vers…')),
                    PopupMenuItem(value: 'delete', child: Text('Supprimer')),
                  ],
                )
              : Text(
                  relativeTime(conversation),
                  style: const TextStyle(
                    fontSize: 10,
                    color: Color(0xff86868b),
                  ),
                ),
        ),
      ),
    ),
  );
  Widget containerMenu(Json item, String kind) => PopupMenuButton<String>(
    tooltip: 'Organiser ${item['name']}',
    icon: const Icon(LucideIcons.ellipsis, size: 16),
    onSelected: (action) async {
      if (action == 'new-chat') {
        await select(null);
        if (!mounted) return;
        setState(() {
          w.destinationProject = kind == 'projects'
              ? item['id']
              : item['projectId'];
          w.destinationFolder = kind == 'folders' ? item['id'] : null;
        });
        return;
      }
      if (action == 'up' || action == 'down') {
        final values = ordered(
          kind == 'projects' ? w.projects : w.folders,
          kind,
        ).map((e) => '${e['id']}').toList();
        final index = values.indexOf('${item['id']}');
        final target = (index + (action == 'up' ? -1 : 1)).clamp(
          0,
          values.length - 1,
        );
        values.removeAt(index);
        values.insert(target, '${item['id']}');
        await w.preferences.setStringList(
          'aro.order.${w.api.accountKey}.$kind',
          values,
        );
        if (mounted) setState(() {});
        return;
      }
      if (action == 'edit') {
        createContainer(kind, existing: item);
        return;
      }
      if (action == 'move') {
        moveFolder(item);
        return;
      }
      if (await confirmDelete(context, '${item['name']}') && mounted) {
        await perform(
          context,
          () => w.mutate('DELETE', '/$kind/${item['id']}'),
        );
      }
    },
    itemBuilder: (_) => [
      const PopupMenuItem(
        value: 'new-chat',
        child: Text('Nouvelle conversation ici'),
      ),
      const PopupMenuItem(value: 'edit', child: Text('Modifier')),
      if (kind == 'folders')
        const PopupMenuItem(value: 'move', child: Text('Changer de projet')),
      const PopupMenuItem(value: 'up', child: Text('Monter')),
      const PopupMenuItem(value: 'down', child: Text('Descendre')),
      const PopupMenuItem(value: 'delete', child: Text('Supprimer')),
    ],
  );

  Future<void> createContainer(
    String kind, {
    Json? existing,
    String? projectId,
  }) async {
    final result = await editFields(
      context,
      existing == null
          ? (kind == 'projects' ? 'Nouveau projet' : 'Nouveau dossier')
          : 'Modifier',
      [
        const SettingField('name', 'Nom', required: true),
        if (kind == 'projects') ...const [
          SettingField('description', 'Description'),
          SettingField(
            'instructions',
            'Instructions du projet',
            multiline: true,
          ),
          SettingField('color', 'Couleur (hexadécimal)'),
        ],
      ],
      {
        ...?existing,
        if (existing == null && kind == 'projects') 'color': '#0071e3',
      },
    );
    if (result == null || !mounted) return;
    await perform(
      context,
      () => w.mutate(
        existing == null ? 'POST' : 'PATCH',
        existing == null ? '/$kind' : '/$kind/${existing['id']}',
        body: {...result, 'projectId': ?projectId},
      ),
    );
  }

  Future<void> moveFolder(Json folder) async {
    final id = await showDialog<String>(
      context: context,
      builder: (context) => SimpleDialog(
        title: const Text('Déplacer le dossier'),
        children: [
          SimpleDialogOption(
            onPressed: () => Navigator.pop(context, ''),
            child: const Text('Sans projet'),
          ),
          for (final p in w.projects)
            SimpleDialogOption(
              onPressed: () => Navigator.pop(context, p['id']),
              child: Text('${p['name']}'),
            ),
        ],
      ),
    );
    if (id == null || !mounted) return;
    await perform(
      context,
      () => w.mutate(
        'PATCH',
        '/folders/${folder['id']}',
        body: {'projectId': id.isEmpty ? null : id},
      ),
    );
  }

  Future<void> conversationAction(Json c, String action) async {
    if (w.sending) return;
    if (action == 'rename') {
      final value = await editFields(
        context,
        'Renommer la conversation',
        const [SettingField('title', 'Titre', required: true)],
        c,
      );
      if (value != null && mounted) {
        await perform(
          context,
          () => w.mutate(
            'PATCH',
            '/conversations/${c['id']}',
            body: {'title': value['title']},
          ),
        );
      }
    }
    if (!mounted) return;
    if (action == 'delete' &&
        await confirmDelete(context, '${c['title']}') &&
        mounted) {
      await perform(context, () async {
        await w.mutate('DELETE', '/conversations/${c['id']}');
        if (w.activeId == c['id']) await select(null);
      });
    }
    if (action == 'move') {
      if (!mounted) return;
      final destination = await showDialog<Json>(
        context: context,
        builder: (context) => SimpleDialog(
          title: const Text('Déplacer la conversation'),
          children: [
            SimpleDialogOption(
              onPressed: () =>
                  Navigator.pop(context, {'projectId': null, 'folderId': null}),
              child: const Text('Conversations'),
            ),
            for (final p in w.projects)
              SimpleDialogOption(
                onPressed: () => Navigator.pop(context, {
                  'projectId': p['id'],
                  'folderId': null,
                }),
                child: Text('Projet · ${p['name']}'),
              ),
            for (final f in w.folders)
              SimpleDialogOption(
                onPressed: () => Navigator.pop(context, {
                  'projectId': f['projectId'],
                  'folderId': f['id'],
                }),
                child: Text('Dossier · ${f['name']}'),
              ),
          ],
        ),
      );
      if (destination != null && mounted) {
        await perform(
          context,
          () => w.mutate(
            'PATCH',
            '/conversations/${c['id']}/move',
            body: destination,
          ),
        );
      }
    }
  }

  Future<void> choosePersonality() async {
    final profiles = records(
      await w.api.request('GET', '/collections/personalities'),
    );
    if (!mounted) return;
    final selected = await showDialog<Json>(
      context: context,
      builder: (context) => SimpleDialog(
        title: const Text('Profil de conversation'),
        children: [
          SimpleDialogOption(
            onPressed: () => Navigator.pop(context, <String, dynamic>{}),
            child: const Text('Profil par défaut'),
          ),
          for (final p in profiles)
            SimpleDialogOption(
              onPressed: () => Navigator.pop(context, p),
              child: Text('${p['name']}'),
            ),
        ],
      ),
    );
    if (selected != null && mounted) {
      setState(() => w.personality = selected.isEmpty ? null : selected);
    }
  }

  Widget profileChip(Json profile) {
    final selected = (w.personality?['id'] ?? 'default') == profile['id'];
    final color = Color(profile['color'] as int? ?? 0xff0071e3);
    return ChoiceChip(
      selected: selected,
      showCheckmark: false,
      onSelected: w.sending
          ? null
          : (_) => setState(() => w.personality = profile),
      avatar: CircleAvatar(
        radius: 9,
        backgroundColor: color,
        child: Icon(
          switch (profile['icon']) {
            'code' => LucideIcons.terminal,
            'check' => LucideIcons.check,
            'edit-2' => LucideIcons.pencil,
            _ => LucideIcons.bot,
          },
          size: 11,
          color: Colors.white,
        ),
      ),
      label: Text(
        '${profile['name']}',
        style: TextStyle(
          fontSize: 12,
          color: selected ? Theme.of(context).colorScheme.primary : null,
        ),
      ),
      selectedColor: Theme.of(
        context,
      ).colorScheme.primary.withValues(alpha: .12),
      backgroundColor: Theme.of(context).colorScheme.surface,
      side: BorderSide(
        color: selected
            ? Theme.of(context).colorScheme.primary
            : Theme.of(context).colorScheme.outlineVariant,
      ),
      shape: const StadiumBorder(),
      padding: const EdgeInsets.symmetric(horizontal: 6),
      visualDensity: VisualDensity.compact,
    );
  }

  // --- Classement conversation (miroir web, design Apple) ---
  Json? get _selFolder {
    if (w.destinationFolder == null) return null;
    return w.folders
        .where((f) => '${f['id']}' == '${w.destinationFolder}')
        .firstOrNull;
  }

  Json? get _selProject {
    if (w.destinationProject != null) {
      final direct = w.projects
          .where((p) => '${p['id']}' == '${w.destinationProject}')
          .firstOrNull;
      if (direct != null) return direct;
    }
    final f = _selFolder;
    if (f != null && f['projectId'] != null) {
      return w.projects
          .where((p) => '${p['id']}' == '${f['projectId']}')
          .firstOrNull;
    }
    return null;
  }

  bool get _isClassified =>
      w.destinationProject != null || w.destinationFolder != null;

  String get _destinationLabel {
    final f = _selFolder, p = _selProject;
    if (f != null) {
      return p != null ? '${p['name']} / ${f['name']}' : '${f['name']}';
    }
    if (p != null) return '${p['name']}';
    if (w.destinationFolder != null) return 'Dossier';
    if (w.destinationProject != null) return 'Projet';
    return 'Sans classement';
  }

  String? get _effectiveRoot {
    final f = _selFolder, p = _selProject;
    final root =
        f?['rootPath']?.toString().trim() ?? p?['rootPath']?.toString().trim();
    return (root == null || root.isEmpty) ? null : root;
  }

  void setDestination(String? pid, String? fid) {
    HapticFeedback.lightImpact();
    setState(() {
      w.destinationProject = pid;
      w.destinationFolder = fid;
    });
  }

  Future<void> openDestinationPicker() async {
    if (w.sending) return;
    HapticFeedback.selectionClick();
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      showDragHandle: true,
      backgroundColor: Theme.of(context).colorScheme.surface,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(24)),
      ),
      builder: (sheetCtx) => DraggableScrollableSheet(
        expand: false,
        initialChildSize: 0.72,
        minChildSize: 0.45,
        maxChildSize: 0.92,
        builder: (_, scrollCtrl) => Padding(
          padding: const EdgeInsets.fromLTRB(20, 8, 20, 20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                'Classer la conversation',
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.titleMedium?.copyWith(
                  fontSize: 17,
                  fontWeight: FontWeight.w700,
                  letterSpacing: -0.3,
                ),
              ),
              const SizedBox(height: 4),
              Text(
                'Choisissez où créer la prochaine conversation',
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  fontSize: 13,
                ),
              ),
              const SizedBox(height: 16),
              Expanded(
                child: ListView(
                  controller: scrollCtrl,
                  physics: const BouncingScrollPhysics(
                    parent: AlwaysScrollableScrollPhysics(),
                  ),
                  children: [
                    _independentOption(),
                    _orDivider('OU UN PROJET'),
                    for (final proj in w.projects) ...[
                      _projectCard(proj),
                      const SizedBox(height: 10),
                    ],
                    if (w.projects.isEmpty)
                      Padding(
                        padding: const EdgeInsets.symmetric(vertical: 12),
                        child: Text(
                          'Aucun projet pour le moment. Créez-en un depuis la barre latérale.',
                          textAlign: TextAlign.center,
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                      ),
                    const SizedBox(height: 6),
                    _rootHint(),
                    const SizedBox(height: 12),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _orDivider(String label) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 12),
    child: Row(
      children: [
        Expanded(
          child: Divider(
            color: Theme.of(
              context,
            ).colorScheme.outlineVariant.withValues(alpha: 0.7),
          ),
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12),
          child: Text(
            label,
            style: const TextStyle(
              fontSize: 10.5,
              fontWeight: FontWeight.w700,
              letterSpacing: 0.8,
              color: Color(0xff86868b),
            ),
          ),
        ),
        Expanded(
          child: Divider(
            color: Theme.of(
              context,
            ).colorScheme.outlineVariant.withValues(alpha: 0.7),
          ),
        ),
      ],
    ),
  );

  Widget _independentOption() {
    final scheme = Theme.of(context).colorScheme;
    final selected = !_isClassified;
    return Material(
      color: selected
          ? scheme.primary.withValues(alpha: 0.08)
          : scheme.onSurface.withValues(alpha: 0.03),
      borderRadius: BorderRadius.circular(17),
      child: InkWell(
        borderRadius: BorderRadius.circular(17),
        onTap: () {
          setDestination(null, null);
          Navigator.pop(context);
        },
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 13),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(17),
            border: Border.all(
              color: selected
                  ? scheme.primary.withValues(alpha: 0.55)
                  : scheme.outlineVariant.withValues(alpha: 0.7),
              width: selected ? 1.4 : 1,
            ),
          ),
          child: Row(
            children: [
              Container(
                width: 36,
                height: 36,
                decoration: BoxDecoration(
                  color: selected
                      ? scheme.primary.withValues(alpha: 0.14)
                      : scheme.onSurface.withValues(alpha: 0.07),
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Icon(
                  LucideIcons.inbox,
                  size: 17,
                  color: selected ? scheme.primary : scheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(width: 12),
              const Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Conversation indépendante',
                      style: TextStyle(
                        fontSize: 14.5,
                        fontWeight: FontWeight.w600,
                        letterSpacing: -0.2,
                      ),
                    ),
                    SizedBox(height: 2),
                    Text(
                      'Sans projet ni dossier',
                      style: TextStyle(
                        fontSize: 12.5,
                        color: Color(0xff86868b),
                      ),
                    ),
                  ],
                ),
              ),
              Icon(
                selected
                    ? LucideIcons.circleCheck
                    : LucideIcons.circle,
                size: 21,
                color: selected ? scheme.primary : const Color(0xffc7c7cc),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _projectCard(Json proj) {
    final scheme = Theme.of(context).colorScheme;
    final pid = '${proj['id']}';
    final projFolders = w.folders
        .where((f) => '${f['projectId']}' == pid)
        .toList();
    final isRootSelected =
        '${w.destinationProject}' == pid && w.destinationFolder == null;
    final dot = projectColor(proj);
    return Container(
      padding: const EdgeInsets.all(13),
      decoration: BoxDecoration(
        color: scheme.onSurface.withValues(alpha: 0.025),
        borderRadius: BorderRadius.circular(17),
        border: Border.all(
          color: scheme.outlineVariant.withValues(alpha: 0.7),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          InkWell(
            borderRadius: BorderRadius.circular(10),
            onTap: () {
              setDestination(pid, null);
              Navigator.pop(context);
            },
            child: Padding(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Row(
                children: [
                  Container(
                    width: 9,
                    height: 9,
                    decoration: BoxDecoration(
                      color: dot,
                      shape: BoxShape.circle,
                    ),
                  ),
                  const SizedBox(width: 9),
                  Expanded(
                    child: Text(
                      '${proj['name']}',
                      style: const TextStyle(
                        fontSize: 14.5,
                        fontWeight: FontWeight.w700,
                        letterSpacing: -0.2,
                      ),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  if (isRootSelected)
                    Icon(
                      LucideIcons.circleCheck,
                      size: 19,
                      color: scheme.primary,
                    ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 9),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              _destPill(
                label: 'Racine du projet',
                icon: LucideIcons.folderKanban,
                selected: isRootSelected,
                onTap: () {
                  setDestination(pid, null);
                  Navigator.pop(context);
                },
              ),
              for (final fold in projFolders)
                _destPill(
                  label: '${fold['name']}',
                  icon: LucideIcons.folder,
                  selected:
                      '${w.destinationFolder}' == '${fold['id']}',
                  onTap: () {
                    setDestination(pid, '${fold['id']}');
                    Navigator.pop(context);
                  },
                ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _destPill({
    required String label,
    required IconData icon,
    required bool selected,
    required VoidCallback onTap,
  }) {
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: selected
          ? scheme.primary.withValues(alpha: 0.12)
          : scheme.surface,
      borderRadius: BorderRadius.circular(20),
      child: InkWell(
        borderRadius: BorderRadius.circular(20),
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(20),
            border: Border.all(
              color: selected
                  ? scheme.primary.withValues(alpha: 0.6)
                  : scheme.outlineVariant.withValues(alpha: 0.8),
              width: selected ? 1.3 : 1,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                selected ? LucideIcons.check : icon,
                size: 13,
                color: selected ? scheme.primary : scheme.onSurfaceVariant,
              ),
              const SizedBox(width: 6),
              Flexible(
                child: Text(
                  label,
                  style: TextStyle(
                    fontSize: 12.5,
                    fontWeight: selected
                        ? FontWeight.w700
                        : FontWeight.w500,
                    color: selected ? scheme.primary : scheme.onSurface,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _rootHint() {
    final scheme = Theme.of(context).colorScheme;
    final root = _effectiveRoot;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 13, vertical: 11),
      decoration: BoxDecoration(
        color: scheme.onSurface.withValues(alpha: 0.04),
        borderRadius: BorderRadius.circular(13),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            LucideIcons.info,
            size: 14,
            color: scheme.onSurface.withValues(alpha: 0.5),
          ),
          const SizedBox(width: 9),
          Expanded(
            child: Text(
              root != null
                  ? 'Root : $root'
                  : 'Aucun root — fichiers du dossier courant',
              style: TextStyle(
                fontSize: 12,
                color: scheme.onSurface.withValues(alpha: 0.62),
                height: 1.4,
              ),
            ),
          ),
        ],
      ),
    );
  }

  /// Pastille "Classer dans" façon Apple (home + composer).
  Widget destinationPill({bool small = false}) {
    final scheme = Theme.of(context).colorScheme;
    final classified = _isClassified;
    final dot = _selProject != null
        ? projectColor(_selProject!)
        : null;
    return Material(
      color: classified
          ? scheme.primary.withValues(alpha: 0.09)
          : scheme.onSurface.withValues(alpha: 0.05),
      borderRadius: BorderRadius.circular(small ? 12 : 20),
      child: InkWell(
        borderRadius: BorderRadius.circular(small ? 12 : 20),
        onTap: w.sending ? null : openDestinationPicker,
        child: Container(
          padding: EdgeInsets.symmetric(
            horizontal: small ? 10 : 14,
            vertical: small ? 7 : 10,
          ),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(small ? 12 : 20),
            border: Border.all(
              color: classified
                  ? scheme.primary.withValues(alpha: 0.4)
                  : scheme.outlineVariant.withValues(alpha: 0.8),
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (classified && dot != null) ...[
                Container(
                  width: 8,
                  height: 8,
                  decoration: BoxDecoration(
                    color: dot,
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: 7),
              ] else
                Icon(
                  LucideIcons.inbox,
                  size: small ? 13 : 15,
                  color: classified ? scheme.primary : scheme.onSurfaceVariant,
                ),
              if (classified && dot != null)
                const SizedBox.shrink()
              else
                const SizedBox(width: 7),
              Flexible(
                child: Text(
                  _destinationLabel,
                  style: TextStyle(
                    fontSize: small ? 12 : 13.5,
                    fontWeight: FontWeight.w600,
                    letterSpacing: -0.15,
                    color: classified ? scheme.primary : scheme.onSurface,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 6),
              Icon(
                LucideIcons.chevronDown,
                size: small ? 13 : 15,
                color: classified
                    ? scheme.primary
                    : scheme.onSurface.withValues(alpha: 0.5),
              ),
            ],
          ),
        ),
      ),
    );
  }

  bool _isOfflineError(String error) {
    final lower = error.toLowerCase();
    return lower.contains('connexion au serveur impossible') ||
        lower.contains('met trop de temps') ||
        lower.contains('réseau');
  }

  void _fillComposer(String value) {
    HapticFeedback.selectionClick();
    composer.text = value;
    w.saveDraft(value);
    setState(() => _mentionTrigger = null);
  }

  Widget emptyChat() => Center(
    child: SingleChildScrollView(
      padding: const EdgeInsets.symmetric(horizontal: 22, vertical: 24),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 520),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Image.asset(
              Theme.of(context).brightness == Brightness.dark
                  ? 'assets/aro-core-logo-dark.png'
                  : 'assets/aro-core-logo.png',
              width: 84,
              height: 84,
            ),
            const SizedBox(height: 18),
            Text(
              'Bonjour ${w.userName.split(' ').first}',
              style: Theme.of(context).textTheme.titleLarge,
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 6),
            const Text(
              'Posez une question, joignez un fichier (+) ou citez un outil (@).',
              textAlign: TextAlign.center,
              style: TextStyle(fontSize: 13, color: Color(0xff86868b)),
            ),
            const SizedBox(height: 14),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              alignment: WrapAlignment.center,
              children: [
                for (final s in [
                  'Explique ce projet en 3 points',
                  '@ pour citer un fichier',
                  'Établis un plan d’action',
                ])
                  ActionChip(
                    label: Text(s, style: const TextStyle(fontSize: 12)),
                    onPressed: s.startsWith('@')
                        ? () => _openMentions(initialQuery: '')
                        : () => _fillComposer(s),
                  ),
              ],
            ),
            const SizedBox(height: 18),
            const Text(
              'PROFIL ARO',
              style: TextStyle(
                fontSize: 10,
                fontWeight: FontWeight.w700,
                color: Color(0xff86868b),
                letterSpacing: .5,
              ),
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 2,
              alignment: WrapAlignment.center,
              children: [
                for (final profile in desktopPersonalities)
                  profileChip(profile),
              ],
            ),
            if (w.personality != null &&
                !desktopPersonalities.any(
                  (p) => p['id'] == w.personality!['id'],
                ))
              profileChip(w.personality!),
            const SizedBox(height: 12),
            TextButton.icon(
              onPressed: () => perform(context, choosePersonality),
              icon: const Icon(LucideIcons.chevronDown, size: 13),
              label: const Text('Mes profils', style: TextStyle(fontSize: 11)),
            ),
            const SizedBox(height: 14),
            const Text(
              'CLASSER DANS',
              style: TextStyle(
                fontSize: 10,
                fontWeight: FontWeight.w700,
                color: Color(0xff86868b),
                letterSpacing: .5,
              ),
            ),
            const SizedBox(height: 8),
            destinationPill(),
          ],
        ),
      ),
    ),
  );

  // ============ Messages façon web, polish Apple ============
  // Miroir de ConversationView.svelte : avatar 32, nom + badge AI + heure
  // "Hier à 05:57", bulle user dégradé bleu à droite, carte assistant,
  // actions pill (Copier / Retenir / Modifier / like), étapes d'agent,
  // bloc de réflexion <think>, skeleton pendant le streaming.

  static const _monthsFr = [
    'janv', 'févr', 'mars', 'avr', 'mai', 'juin',
    'juil', 'août', 'sept', 'oct', 'nov', 'déc',
  ];
  static const _daysFr = [
    'Lundi', 'Mardi', 'Mercredi', 'Jeudi', 'Vendredi', 'Samedi', 'Dimanche',
  ];

  String _msgTime(Json item) {
    final raw =
        '${item['createdAt'] ?? item['updatedAt'] ?? item['timestamp'] ?? ''}';
    if (raw.isEmpty || raw == 'null') return '';
    final date = DateTime.tryParse(raw)?.toLocal();
    if (date == null) return '';
    final now = DateTime.now();
    final hm =
        '${date.hour.toString().padLeft(2, '0')}:${date.minute.toString().padLeft(2, '0')}';
    final day = DateTime(date.year, date.month, date.day);
    final today = DateTime(now.year, now.month, now.day);
    final diff = today.difference(day).inDays;
    if (diff <= 0) return "Aujourd'hui à $hm";
    if (diff == 1) return 'Hier à $hm';
    if (diff < 7) return '${_daysFr[date.weekday - 1]} à $hm';
    if (date.year == now.year) {
      return '${date.day} ${_monthsFr[date.month - 1]} à $hm';
    }
    return '${date.day} ${_monthsFr[date.month - 1]} ${date.year} à $hm';
  }

  String _reasoningOf(String content) {
    final m = RegExp(r'<think>([\s\S]*?)</think>', caseSensitive: false)
        .firstMatch(content);
    return (m?.group(1) ?? '').trim();
  }

  String _contentWithoutThinking(String content) => content
      .replaceAll(
        RegExp(r'<think>[\s\S]*?</think>', caseSensitive: false),
        '',
      )
      .trim();

  void _markCopied(String id) {
    _copiedTimer?.cancel();
    setState(() => _copiedId = id);
    _copiedTimer = Timer(const Duration(seconds: 2), () {
      if (mounted) setState(() => _copiedId = null);
    });
  }

  Future<void> _rememberLocally(String content) async {
    final text = _contentWithoutThinking(content);
    if (text.isEmpty) return;
    await perform(context, () async {
      try {
        await w.api.request(
          'POST',
          '/collections/memories',
          body: {
            'content': text.substring(0, text.length.clamp(0, 2000)),
            'category': 'personal',
            'source': 'chat',
            'pinned': false,
          },
        );
      } catch (_) {
        final key = 'aro.saved.${w.api.accountKey}';
        final raw = w.preferences.getString(key);
        final List<dynamic> list = raw == null || raw.isEmpty
            ? []
            : List<dynamic>.from(jsonDecode(raw) as List);
        list.insert(0, {
          'text': text.substring(0, text.length.clamp(0, 2000)),
          'at': DateTime.now().toIso8601String(),
        });
        await w.preferences.setString(
          key,
          jsonEncode(list.take(100).toList()),
        );
        throw const ApiException(
          0,
          'Serveur injoignable — retenu localement sur ce téléphone.',
        );
      }
    }, success: 'Mémorisé ✓');
  }

  Widget _avatar(bool user) => Container(
    width: 32,
    height: 32,
    decoration: BoxDecoration(
      shape: BoxShape.circle,
      gradient: user
          ? null
          : const LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: [Color(0xff1c2232), Color(0xff2d3a56)],
            ),
      color: user ? Colors.white : null,
      border: user ? Border.all(color: const Color(0xffdfe4eb)) : null,
      boxShadow: [
        BoxShadow(
          color: Colors.black.withValues(alpha: 0.08),
          blurRadius: 8,
          offset: const Offset(0, 2),
        ),
      ],
    ),
    child: Icon(
      user ? LucideIcons.user : LucideIcons.bot,
      size: 16,
      color: user ? const Color(0xff1d1d1f) : Colors.white,
    ),
  );

  Widget _aiBadge() => Container(
    padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
    decoration: BoxDecoration(
      gradient: const LinearGradient(
        colors: [Color(0xff7f00ff), Color(0xff00f2fe)],
      ),
      borderRadius: BorderRadius.circular(4),
    ),
    child: const Text(
      'AI',
      style: TextStyle(
        fontSize: 9,
        fontWeight: FontWeight.w800,
        color: Colors.white,
        letterSpacing: 0.3,
      ),
    ),
  );

  Widget _actionPill({
    required IconData icon,
    required String label,
    required VoidCallback? onTap,
    bool active = false,
    Color? activeColor,
  }) {
    final scheme = Theme.of(context).colorScheme;
    final accent = activeColor ?? scheme.primary;
    return Material(
      color: active ? accent.withValues(alpha: 0.1) : Colors.transparent,
      borderRadius: BorderRadius.circular(9),
      child: InkWell(
        borderRadius: BorderRadius.circular(9),
        onTap: onTap == null
            ? null
            : () {
                HapticFeedback.selectionClick();
                onTap();
              },
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(9),
            border: Border.all(
              color: active
                  ? accent.withValues(alpha: 0.55)
                  : scheme.outlineVariant.withValues(alpha: 0.9),
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                icon,
                size: 13,
                color: active
                    ? accent
                    : scheme.onSurface.withValues(alpha: 0.62),
              ),
              const SizedBox(width: 6),
              Text(
                label,
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w500,
                  color: active
                      ? accent
                      : scheme.onSurface.withValues(alpha: 0.72),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  (IconData, Color) _stepKindStyle(String kind, Json step) {
    const blue = Color(0xff0071e3);
    const purple = Color(0xffbf5af2);
    const green = Color(0xff248a3d);
    const orange = Color(0xffd97a06);
    switch (kind) {
      case 'run-started':
        return (LucideIcons.rocket, blue);
      case 'context-built':
        return (LucideIcons.fileText, orange);
      case 'model':
        return (LucideIcons.brain, purple);
      case 'checkpoint':
        return (LucideIcons.save, green);
      case 'final':
        return (LucideIcons.circleCheck, green);
      case 'error':
        return (LucideIcons.circleAlert, const Color(0xffe74c3c));
      case 'tool':
        final name =
            '${step['name'] ?? object(step['input'])['tool'] ?? object(step['input'])['name'] ?? ''}'
                .toLowerCase();
        if (name.contains('memor')) return (LucideIcons.brain, purple);
        if (name.contains('search') || name.contains('web')) {
          return (LucideIcons.globe, blue);
        }
        if (name.contains('fetch') || name.contains('read')) {
          return (LucideIcons.bookOpen, orange);
        }
        return (LucideIcons.wrench, blue);
      default:
        return (LucideIcons.settings, const Color(0xff86868b));
    }
  }

  List<Json> _stepsOf(Json item) =>
      records(item['steps'] ?? item['agentSteps'] ?? item['agent_steps']);

  /// Carte "Agent Actif / Terminé" + étapes repliables (miroir web).
  Widget _agentStepsCard(String id, List<Json> steps, bool generating) {
    final scheme = Theme.of(context).colorScheme;
    final expanded = generating || _expandedSteps.contains(id);
    final done = steps.every(
      (s) => '${s['status']}' == 'completed' || '${s['status']}' == 'skipped',
    );
    final failed = steps.any((s) => '${s['status']}' == 'failed');
    final dotColor = generating
        ? const Color(0xff0071e3)
        : failed
        ? const Color(0xffe74c3c)
        : const Color(0xff34c759);
    return Container(
      margin: const EdgeInsets.only(bottom: 10),
      decoration: BoxDecoration(
        color: scheme.onSurface.withValues(alpha: 0.025),
        borderRadius: BorderRadius.circular(14),
        border: Border.all(
          color: scheme.outlineVariant.withValues(alpha: 0.8),
        ),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          InkWell(
            borderRadius: BorderRadius.circular(14),
            onTap: () {
              HapticFeedback.selectionClick();
              setState(() {
                if (_expandedSteps.contains(id)) {
                  _expandedSteps.remove(id);
                } else {
                  _expandedSteps.add(id);
                }
              });
            },
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
              child: Row(
                children: [
                  Container(
                    width: 8,
                    height: 8,
                    decoration: BoxDecoration(
                      color: dotColor,
                      shape: BoxShape.circle,
                    ),
                  ),
                  const SizedBox(width: 9),
                  Text(
                    generating
                        ? 'Agent actif'
                        : done
                        ? 'Agent terminé'
                        : 'Agent',
                    style: const TextStyle(
                      fontSize: 12.5,
                      fontWeight: FontWeight.w700,
                      letterSpacing: -0.1,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 7,
                      vertical: 2,
                    ),
                    decoration: BoxDecoration(
                      color: scheme.onSurface.withValues(alpha: 0.06),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: Text(
                      '${steps.length}',
                      style: TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.w700,
                        color: scheme.onSurface.withValues(alpha: 0.6),
                      ),
                    ),
                  ),
                  const Spacer(),
                  Text(
                    expanded ? 'Masquer' : 'Étapes',
                    style: TextStyle(
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                      color: scheme.primary,
                    ),
                  ),
                  const SizedBox(width: 4),
                  Icon(
                    expanded
                        ? LucideIcons.chevronUp
                        : LucideIcons.chevronDown,
                    size: 14,
                    color: scheme.primary,
                  ),
                ],
              ),
            ),
          ),
          if (expanded)
            Padding(
              padding: const EdgeInsets.fromLTRB(8, 0, 8, 8),
              child: Column(
                children: [
                  for (final s in steps) _stepRow(id, s),
                ],
              ),
            ),
        ],
      ),
    );
  }

  Widget _stepRow(String messageId, Json step) {
    final scheme = Theme.of(context).colorScheme;
    final status = '${step['status'] ?? 'completed'}';
    final kind = '${step['kind'] ?? step['type'] ?? 'tool'}';
    final title = '${step['title'] ?? step['name'] ?? kind}';
    final seq = '${step['sequence'] ?? step['id'] ?? title}';
    final detailKey = '$messageId-$seq';
    final showDetails = _expandedStepDetails.contains(detailKey);
    final output = object(step['output']);
    final summary =
        '${output['summary'] ?? output['text'] ?? output['result'] ?? ''}';
    final results = records(output['results']);
    final error = '${step['error'] ?? ''}';
    final running = status == 'running';
    final failed = status == 'failed';
    final (kindIcon, kindColor) = _stepKindStyle(kind, step);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 20,
            child: running
                ? const Padding(
                    padding: EdgeInsets.only(top: 4),
                    child: SizedBox(
                      width: 11,
                      height: 11,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  )
                : Icon(
                    failed
                        ? LucideIcons.x
                        : status == 'skipped'
                        ? LucideIcons.minus
                        : LucideIcons.check,
                    size: 14,
                    color: failed
                        ? const Color(0xffe74c3c)
                        : status == 'skipped'
                        ? const Color(0xff86868b)
                        : const Color(0xff2ecc71),
                  ),
          ),
          Container(
            width: 28,
            height: 28,
            decoration: BoxDecoration(
              color: kindColor.withValues(alpha: 0.1),
              borderRadius: BorderRadius.circular(9),
            ),
            child: Icon(kindIcon, size: 14, color: kindColor),
          ),
          const SizedBox(width: 9),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        title,
                        style: const TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          letterSpacing: -0.15,
                        ),
                      ),
                    ),
                    if ((kind == 'tool' &&
                            (summary.isNotEmpty || results.isNotEmpty)) ||
                        error.isNotEmpty && error != 'null')
                      GestureDetector(
                        onTap: () {
                          HapticFeedback.selectionClick();
                          setState(() {
                            if (showDetails) {
                              _expandedStepDetails.remove(detailKey);
                            } else {
                              _expandedStepDetails.add(detailKey);
                            }
                          });
                        },
                        child: Padding(
                          padding: const EdgeInsets.only(left: 8),
                          child: Text(
                            showDetails ? 'Fermer' : 'Détails',
                            style: TextStyle(
                              fontSize: 12,
                              fontWeight: FontWeight.w600,
                              color: scheme.primary,
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
                if (failed && error.isNotEmpty && error != 'null')
                  Container(
                    margin: const EdgeInsets.only(top: 6),
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 8,
                    ),
                    decoration: BoxDecoration(
                      color: const Color(0xffe74c3c).withValues(alpha: 0.08),
                      borderRadius: BorderRadius.circular(9),
                    ),
                    child: Text(
                      error,
                      style: const TextStyle(
                        fontSize: 12,
                        color: Color(0xffe74c3c),
                        height: 1.4,
                      ),
                    ),
                  ),
                if (showDetails) ...[
                  const SizedBox(height: 6),
                  Container(
                    width: double.infinity,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 11,
                      vertical: 9,
                    ),
                    decoration: BoxDecoration(
                      color: scheme.onSurface.withValues(alpha: 0.04),
                      borderRadius: BorderRadius.circular(10),
                      border: Border.all(
                        color: scheme.outlineVariant.withValues(alpha: 0.6),
                      ),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        if (summary.isNotEmpty)
                          Text(
                            summary.length > 600
                                ? '${summary.substring(0, 600)}…'
                                : summary,
                            style: TextStyle(
                              fontSize: 12.5,
                              height: 1.5,
                              color: scheme.onSurface.withValues(alpha: 0.85),
                            ),
                          ),
                        for (final r in results.take(3))
                          Padding(
                            padding: const EdgeInsets.only(top: 6),
                            child: GestureDetector(
                              onTap: () async {
                                final uri = Uri.tryParse(
                                  '${r['url'] ?? ''}',
                                );
                                if (uri != null &&
                                    ['http', 'https']
                                        .contains(uri.scheme)) {
                                  await launchUrl(
                                    uri,
                                    mode: LaunchMode.externalApplication,
                                  );
                                }
                              },
                              child: Text(
                                '↗ ${r['title'] ?? r['url'] ?? 'Résultat'}',
                                style: TextStyle(
                                  fontSize: 12.5,
                                  fontWeight: FontWeight.w600,
                                  color: scheme.primary,
                                ),
                                maxLines: 2,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }

  /// Bloc de réflexion `think` repliable (miroir web, ouvert par défaut).
  Widget _thinkingCard(String id, String reasoning, bool generating) {
    final scheme = Theme.of(context).colorScheme;
    final collapsed = _collapsedThinking.contains(id);
    final open = !collapsed;
    return Container(
      margin: const EdgeInsets.only(bottom: 10),
      decoration: BoxDecoration(
        color: scheme.primary.withValues(alpha: 0.045),
        borderRadius: BorderRadius.circular(14),
        border: Border.all(
          color: scheme.primary.withValues(alpha: 0.16),
        ),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          InkWell(
            borderRadius: BorderRadius.circular(14),
            onTap: () {
              HapticFeedback.selectionClick();
              setState(() {
                if (collapsed) {
                  _collapsedThinking.remove(id);
                } else {
                  _collapsedThinking.add(id);
                }
              });
            },
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: 12,
                vertical: 10,
              ),
              child: Row(
                children: [
                  if (generating)
                    const SizedBox(
                      width: 14,
                      height: 14,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  else
                    Icon(
                      LucideIcons.brain,
                      size: 15,
                      color: scheme.primary,
                    ),
                  const SizedBox(width: 9),
                  Text(
                    generating ? 'ARO réfléchit…' : 'Réflexion',
                    style: TextStyle(
                      fontSize: 12.5,
                      fontWeight: FontWeight.w700,
                      color: scheme.primary,
                    ),
                  ),
                  const Spacer(),
                  Icon(
                    open ? LucideIcons.chevronUp : LucideIcons.chevronDown,
                    size: 14,
                    color: scheme.primary,
                  ),
                ],
              ),
            ),
          ),
          if (open)
            Padding(
              padding: const EdgeInsets.fromLTRB(13, 0, 13, 12),
              child: Text(
                reasoning,
                style: TextStyle(
                  fontSize: 13,
                  height: 1.55,
                  color: scheme.onSurface.withValues(alpha: 0.72),
                  fontStyle: FontStyle.italic,
                ),
              ),
            ),
        ],
      ),
    );
  }

  /// Skeleton premium pendant la génération (remplace le spinner brut).
  Widget _generatingSkeleton() => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    mainAxisSize: MainAxisSize.min,
    children: [
      TweenAnimationBuilder<double>(
        tween: Tween(begin: 0.35, end: 1),
        duration: const Duration(milliseconds: 900),
        builder: (context, value, _) => Opacity(
          opacity: 0.35 + 0.65 * (1 - (value - 0.5).abs() * 2).clamp(0.0, 1.0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Container(
                height: 12,
                width: double.infinity,
                decoration: BoxDecoration(
                  color: Theme.of(
                    context,
                  ).colorScheme.onSurface.withValues(alpha: 0.1),
                  borderRadius: BorderRadius.circular(6),
                ),
              ),
              const SizedBox(height: 8),
              Container(
                height: 12,
                width: MediaQuery.sizeOf(context).width * 0.45,
                decoration: BoxDecoration(
                  color: Theme.of(
                    context,
                  ).colorScheme.onSurface.withValues(alpha: 0.1),
                  borderRadius: BorderRadius.circular(6),
                ),
              ),
              const SizedBox(height: 8),
              Container(
                height: 12,
                width: MediaQuery.sizeOf(context).width * 0.3,
                decoration: BoxDecoration(
                  color: Theme.of(
                    context,
                  ).colorScheme.onSurface.withValues(alpha: 0.1),
                  borderRadius: BorderRadius.circular(6),
                ),
              ),
            ],
          ),
        ),
      ),
    ],
  );

  Widget _markdown(Json item, String content, bool user) => MarkdownBody(
    data: content,
    selectable: true,
    onTapLink: (_, url, _) async {
      final uri = Uri.tryParse(url ?? '');
      if (uri != null && ['http', 'https'].contains(uri.scheme)) {
        await launchUrl(uri, mode: LaunchMode.externalApplication);
      }
    },
    styleSheet: MarkdownStyleSheet.fromTheme(Theme.of(context)).copyWith(
      p: Theme.of(context).textTheme.bodyLarge?.copyWith(
        fontSize: 15,
        height: 1.55,
        color: user ? Colors.white : null,
      ),
      codeblockDecoration: BoxDecoration(
        color: (user
                ? Colors.white
                : Theme.of(context).colorScheme.onSurface)
            .withValues(alpha: user ? 0.14 : 0.05),
        borderRadius: BorderRadius.circular(12),
      ),
      blockquoteDecoration: BoxDecoration(
        border: Border(
          left: BorderSide(
            color: user
                ? Colors.white.withValues(alpha: 0.6)
                : Theme.of(context).colorScheme.primary.withValues(alpha: 0.5),
            width: 3,
          ),
        ),
      ),
    ),
  );

  Widget messageList() => RefreshIndicator(
    onRefresh: () => w.activeId == null ? w.refresh() : w.select(w.activeId),
    child: ListView.builder(
      controller: scroll,
      physics: const BouncingScrollPhysics(
        parent: AlwaysScrollableScrollPhysics(),
      ),
      keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 24),
      itemCount: w.messages.length,
    itemBuilder: (context, index) {
      final item = w.messages[index], user = item['role'] == 'user';
      final raw = '${item['content'] ?? ''}';
      final id = '${item['id'] ?? 'm$index'}';
      final time = _msgTime(item);
      final isLast = index == w.messages.length - 1;
      final live = !user && w.sending && isLast;
      final reasoning = user ? '' : _reasoningOf(raw);
      final content = user ? raw : _contentWithoutThinking(raw);
      final steps = user ? const <Json>[] : _stepsOf(item);
      final copied = _copiedId == id;
      final vote = _feedback[id];
      return Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 820),
          child: Padding(
            padding: const EdgeInsets.only(bottom: 28),
            child: Column(
              crossAxisAlignment: user
                  ? CrossAxisAlignment.end
                  : CrossAxisAlignment.start,
              children: [
                // Méta façon web : avatar 32 + nom + badge AI + heure.
                // Tap dessus = révèle les actions (façon iMessage).
                GestureDetector(
                  behavior: HitTestBehavior.translucent,
                  onTap: content.isEmpty ? null : () => _toggleActions(id),
                  child: Padding(
                    padding:
                        const EdgeInsets.only(bottom: 9, left: 2, right: 2),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (!user) ...[
                        _avatar(false),
                        const SizedBox(width: 9),
                      ],
                      Flexible(
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Flexible(
                              child: Text(
                                user ? 'Vous' : 'ARO',
                                style: const TextStyle(
                                  fontSize: 13,
                                  fontWeight: FontWeight.w600,
                                  letterSpacing: -0.15,
                                ),
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                            if (!user) ...[
                              const SizedBox(width: 6),
                              _aiBadge(),
                            ],
                            if (time.isNotEmpty) ...[
                              const SizedBox(width: 7),
                              Flexible(
                                child: Text(
                                  time,
                                  style: const TextStyle(
                                    fontSize: 11,
                                    color: Color(0xff86868b),
                                  ),
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                            ],
                          ],
                        ),
                      ),
                      if (user) ...[
                        const SizedBox(width: 9),
                        _avatar(true),
                      ],
                    ],
                  ),
                  ),
                ),
                // Étapes d'agent + réflexion au-dessus du contenu (comme web).
                if (!user && steps.isNotEmpty)
                  _agentStepsCard(id, steps, live && content.isEmpty),
                if (!user && reasoning.isNotEmpty)
                  _thinkingCard(id, reasoning, live && content.isEmpty),
                if (!user && live && content.isEmpty && steps.isEmpty)
                  _agentStepsCard(id, const [], true),
                // Bulle : tap = révèle les actions. Sélection = anneau discret.
                GestureDetector(
                  behavior: HitTestBehavior.translucent,
                  onTap: content.isEmpty ? null : () => _toggleActions(id),
                  child: Align(
                    alignment: user
                        ? Alignment.centerRight
                        : Alignment.centerLeft,
                    child: AnimatedContainer(
                      duration: const Duration(milliseconds: 220),
                      curve: Curves.easeOutCubic,
                      constraints: BoxConstraints(
                        maxWidth:
                            MediaQuery.sizeOf(context).width * (user ? 0.78 : 0.92),
                      ),
                      padding: EdgeInsets.symmetric(
                        horizontal: user ? 15 : 17,
                        vertical: user ? 10 : 14,
                      ),
                      decoration: BoxDecoration(
                        color: user
                            ? null
                            : Theme.of(context).colorScheme.surface,
                        gradient: user
                            ? const LinearGradient(
                                begin: Alignment.topLeft,
                                end: Alignment.bottomRight,
                                colors: [Color(0xff2f96f8), Color(0xff0071e3)],
                              )
                            : null,
                        border: Border.all(
                          color: _actionsFor == id
                              ? (user
                                    ? Colors.white.withValues(alpha: 0.85)
                                    : Theme.of(
                                        context,
                                      ).colorScheme.primary.withValues(alpha: 0.65))
                              : (user
                                    ? Colors.transparent
                                    : Theme.of(
                                        context,
                                      ).colorScheme.outlineVariant),
                          width: _actionsFor == id ? 1.6 : 1,
                        ),
                        borderRadius: BorderRadius.only(
                          topLeft: const Radius.circular(18),
                          topRight: const Radius.circular(18),
                          bottomLeft: Radius.circular(user ? 18 : 6),
                          bottomRight: Radius.circular(user ? 6 : 18),
                        ),
                        boxShadow: [
                          BoxShadow(
                            color: (user
                                    ? const Color(0xff0071e3)
                                    : Colors.black)
                                .withValues(
                                  alpha: user
                                      ? (_actionsFor == id ? .32 : .22)
                                      : (_actionsFor == id ? .07 : .04),
                                ),
                            blurRadius:
                                _actionsFor == id ? 18 : (user ? 14 : 12),
                            offset: const Offset(0, 4),
                          ),
                        ],
                      ),
                      child: content.isEmpty
                          ? (live
                                ? _generatingSkeleton()
                                : Text(
                                    '…',
                                    style: TextStyle(
                                      color: Theme.of(context)
                                          .colorScheme
                                          .onSurface
                                          .withValues(alpha: 0.4),
                                    ),
                                  ))
                          : _markdown(item, content, user),
                    ),
                  ),
                ),
                for (final a in records(item['attachments']))
                  Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: Chip(
                      avatar: const Icon(LucideIcons.paperclip, size: 15),
                      label: Text('${a['displayName'] ?? 'Fichier'}'),
                    ),
                  ),
                // Actions pill : révélées au tap sur le message, repli animé.
                ClipRect(
                  child: AnimatedSize(
                    duration: const Duration(milliseconds: 260),
                    curve: Curves.easeOutCubic,
                    child: AnimatedOpacity(
                      duration: const Duration(milliseconds: 200),
                      opacity: _actionsFor == id && content.isNotEmpty ? 1 : 0,
                      child: _actionsFor == id && content.isNotEmpty
                          ? Padding(
                              padding: const EdgeInsets.only(top: 9),
                              child: Wrap(
                                spacing: 8,
                                runSpacing: 8,
                                alignment: user
                                    ? WrapAlignment.end
                                    : WrapAlignment.start,
                                children: [
                      _actionPill(
                        icon: copied ? LucideIcons.check : LucideIcons.copy,
                        label: copied ? 'Copié' : 'Copier',
                        active: copied,
                        activeColor: const Color(0xff248a3d),
                        onTap: () {
                          Clipboard.setData(ClipboardData(text: content));
                          _markCopied(id);
                          HapticFeedback.lightImpact();
                          ScaffoldMessenger.of(context).showSnackBar(
                            const SnackBar(
                              content: Text('Copié'),
                              duration: Duration(seconds: 1),
                            ),
                          );
                        },
                      ),
                      _actionPill(
                        icon: LucideIcons.brain,
                        label: 'Retenir',
                        onTap: () => _rememberLocally(raw),
                      ),
                      if (user && item['id'] != null)
                        _actionPill(
                          icon: LucideIcons.pencil,
                          label: 'Modifier',
                          onTap: () => editMessage(item),
                        ),
                      if (!user) ...[
                        _actionPill(
                          icon: LucideIcons.volume2,
                          label: 'Lire',
                          onTap: () => tts.speak(content),
                        ),
                        if (isLast && !w.sending)
                          _actionPill(
                            icon: LucideIcons.refreshCw,
                            label: 'Régénérer',
                            onTap: () => regenerate(),
                          ),
                        _actionPill(
                          icon: LucideIcons.thumbsUp,
                          label: 'Utile',
                          active: vote == true,
                          onTap: () => setState(() {
                            _feedback[id] = vote == true ? null : true;
                          }),
                        ),
                        _actionPill(
                          icon: LucideIcons.thumbsDown,
                          label: 'À revoir',
                          active: vote == false,
                          activeColor: const Color(0xffe74c3c),
                          onTap: () => setState(() {
                            _feedback[id] = vote == false ? null : false;
                          }),
                        ),
                      ],
                    ],
                  ),
                )
                      : const SizedBox(width: double.infinity, height: 0),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      );
    },
  ),
  );
  Future<void> editMessage(Json item) async {
    final result = await editFields(context, 'Modifier le message', const [
      SettingField('content', 'Message', multiline: true, required: true),
    ], item);
    if (result != null && mounted) {
      await perform(context, () async {
        await w.api.request(
          'PATCH',
          '/messages/${item['id']}',
          body: {'content': result['content']},
        );
        await w.select(w.activeId);
      });
    }
  }

  Widget permissionButton() => OutlinedButton.icon(
    onPressed: () => Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => SettingsDetail(
          workspace: w,
          section: settingsSections.firstWhere((s) => s.id == 'permissions'),
        ),
      ),
    ),
    icon: const Icon(LucideIcons.shield, size: 13),
    label: const Text(
      'Droits',
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: TextStyle(fontSize: 11, fontWeight: FontWeight.w600),
    ),
    style: OutlinedButton.styleFrom(
      minimumSize: const Size(0, 40),
      maximumSize: const Size(130, 40),
      padding: const EdgeInsets.symmetric(horizontal: 8),
      tapTargetSize: MaterialTapTargetSize.shrinkWrap,
      foregroundColor: const Color(0xff009ee3),
      backgroundColor: const Color(0xff009ee3).withValues(alpha: .06),
      side: BorderSide(color: const Color(0xff009ee3).withValues(alpha: .25)),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(11)),
    ),
  );

  /// Sélecteur modèle du composer : surcharge PAR MESSAGE (comme desktop),
  /// sans muter le défaut d'organisation. Le serveur doit servir exactement
  /// le modèle choisi (local ou cloud opt-in) ou échouer honnêtement.
  Future<void> chooseModel() async {
    final model = object(w.settings['model']);
    final ai = w.aiStatus;
    final runnable = <String>{
      for (final m in (ai['runnableModelIds'] as List? ?? [])) '$m',
    };
    bool refRunnable(Json ref) =>
        ai.isEmpty || runnable.contains('${ref['modelId']}');
    final refs = <Json>[
      for (final provider in records(model['providers']))
        if (provider['enabled'] == true)
          for (final ref in records(provider['models']))
            {
              ...ref,
              'providerId': ref['providerId'] ?? provider['id'],
              'providerKind': ref['providerKind'] ?? provider['kind'],
              'providerName': provider['displayName'] ?? provider['kind'],
            },
    ];
    final current = w.effectiveModelRef;
    bool isCurrent(Json ref) =>
        '${current['modelId']}' == '${ref['modelId']}' &&
        '${current['providerId']}' == '${ref['providerId']}';
    final activeDefault = object(model['activeModelRef']);
    final selected = await showModalBottomSheet<Json>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (context) => SafeArea(
        child: SizedBox(
          height: MediaQuery.sizeOf(context).height * .65,
          child: Column(
            children: [
              const Padding(
                padding: EdgeInsets.all(16),
                child: Text(
                  'Sélectionner un modèle',
                  style: TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
                ),
              ),
              Expanded(
                child: ListView(
                  children: [
                    ListTile(
                      leading: const Icon(LucideIcons.building2, size: 18),
                      title: const Text('Modèle par défaut (organisation)'),
                      subtitle: Text(
                        '${activeDefault['label'] ?? activeDefault['modelId'] ?? ''}',
                      ),
                      trailing: w.modelOverride == null
                          ? const Icon(LucideIcons.check, size: 18)
                          : null,
                      onTap: () => Navigator.pop(context, const {'__default': true}),
                    ),
                    const Divider(),
                    if (refs.isEmpty)
                      const Padding(
                        padding: EdgeInsets.all(24),
                        child: Text(
                          'Aucun modèle disponible. Configurez vos fournisseurs dans Paramètres › Modèle IA.',
                        ),
                      ),
                    for (final ref in refs)
                      ListTile(
                        leading: const Icon(LucideIcons.cpu, size: 18),
                        title: Text('${ref['label'] ?? ref['modelId']}'),
                        subtitle: Text(
                          refRunnable(ref)
                              ? '${ref['providerName']}'
                              : '${ref['providerName']} · Non exécuté ici',
                        ),
                        trailing: isCurrent(ref)
                            ? const Icon(LucideIcons.check, size: 18)
                            : null,
                        onTap: refRunnable(ref)
                            ? () => Navigator.pop(context, ref)
                            : null,
                      ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
    if (selected == null) return;
    if (selected['__default'] == true) {
      w.setModelOverride(null);
      return;
    }
    final ref = Map<String, dynamic>.of(selected)
      ..remove('providerName')
      ..remove('__default');
    w.setModelOverride(ref);
  }

  // Padding bas FIXE : le Scaffold reactive deja au clavier
  // (resizeToAvoidBottomInset). L'ancien `viewInsets * 0.35` faisait gonfler
  // le composer PENDANT que le corps retrecissait -> double comptage et
  // "BOTTOM OVERFLOWED" des que le clavier s'ouvrait. Le fixe supprime
  // aussi le decalage d'animation de 200 ms qui depassait en transitoire.
  Widget composerView() => Padding(
    padding: EdgeInsets.fromLTRB(
      16,
      8,
      16,
      10 + MediaQuery.viewPaddingOf(context).bottom * 0.2,
    ),
    child: Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surface,
            borderRadius: BorderRadius.circular(22),
            border: Border.all(
              color: Theme.of(
                context,
              ).colorScheme.primary.withValues(alpha: .16),
            ),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: .025),
                blurRadius: 20,
                offset: const Offset(0, 6),
              ),
            ],
          ),
          padding: const EdgeInsets.fromLTRB(10, 10, 10, 8),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (w.activeId == null)
                Padding(
                  padding: const EdgeInsets.fromLTRB(2, 0, 2, 6),
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: destinationPill(small: true),
                  ),
                ),
              if (w.attachments.isNotEmpty)
                SingleChildScrollView(
                  scrollDirection: Axis.horizontal,
                  child: Row(
                    children: [
                      for (final a in w.attachments)
                        Padding(
                          padding: const EdgeInsets.only(right: 8),
                          child: InputChip(
                            avatar: const Icon(LucideIcons.fileText, size: 16),
                            label: Text('${a['displayName']}'),
                            onDeleted: w.sending
                                ? null
                                : () => setState(() => w.attachments.remove(a)),
                          ),
                        ),
                    ],
                  ),
                ),
              if (_mentionTrigger != null)
                Padding(
                  padding: const EdgeInsets.fromLTRB(2, 0, 2, 6),
                  child: Material(
                    color: Theme.of(context).colorScheme.primary.withValues(
                      alpha: .07,
                    ),
                    borderRadius: BorderRadius.circular(12),
                    child: InkWell(
                      borderRadius: BorderRadius.circular(12),
                      onTap: () => _openMentions(),
                      child: Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 10,
                          vertical: 8,
                        ),
                        child: Row(
                          children: [
                            const Icon(LucideIcons.atSign, size: 14),
                            const SizedBox(width: 8),
                            Expanded(
                              child: Text(
                                _mentionTrigger!.query.isEmpty
                                    ? 'Mentionner un fichier, skill, agent…'
                                    : '@${_mentionTrigger!.query}',
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(fontSize: 12),
                              ),
                            ),
                            const SizedBox(width: 8),
                            const Text(
                              'Voir',
                              style: TextStyle(
                                fontSize: 12,
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              TextField(
                controller: composer,
                enabled: !w.sending,
                minLines: 1,
                maxLines: 6,
                onChanged: _onComposerChanged,
                textCapitalization: TextCapitalization.sentences,
                decoration: const InputDecoration(
                  hintText: "Demandez n'importe quoi à ARO (@ pour citer)",
                  filled: false,
                  border: InputBorder.none,
                  enabledBorder: InputBorder.none,
                  focusedBorder: InputBorder.none,
                  contentPadding: EdgeInsets.fromLTRB(10, 8, 10, 16),
                ),
              ),
              LayoutBuilder(
                builder: (context, constraints) {
                  final ref = w.effectiveModelRef;
                  final label =
                      '${ref['label'] ?? ref['modelId'] ?? 'Modèle IA'}${w.modelOverride != null ? ' · choix' : ''}';
                  final controls = [
                    DesktopTool(
                      icon: uploading ? LucideIcons.loader : LucideIcons.plus,
                      label: 'Joindre un fichier',
                      onPressed: w.sending || uploading ? null : attach,
                    ),
                    const SizedBox(width: 6),
                    DesktopTool(
                      icon: LucideIcons.atSign,
                      label: 'Mentionner un fichier ou un outil (@)',
                      onPressed: w.sending
                          ? null
                          : () => _openMentions(initialQuery: ''),
                    ),
                    const SizedBox(width: 6),
                    DesktopTool(
                      icon: LucideIcons.globe,
                      label: w.webAccess
                          ? 'Désactiver la recherche web'
                          : 'Activer la recherche web',
                      color: const Color(0xff008c36),
                      onPressed: w.sending
                          ? null
                          : () => setState(() => w.webAccess = !w.webAccess),
                    ),
                    const SizedBox(width: 6),
                    DesktopTool(
                      icon: listening ? LucideIcons.square : LucideIcons.mic,
                      label: listening
                          ? 'Arrêter la dictée'
                          : 'Dicter un message',
                      onPressed: w.sending ? null : dictate,
                    ),
                  ];
                  final selector = OutlinedButton(
                    onPressed: w.sending
                        ? null
                        : () => perform(context, chooseModel),
                    style: OutlinedButton.styleFrom(
                      minimumSize: const Size(0, 40),
                      padding: const EdgeInsets.symmetric(horizontal: 10),
                      backgroundColor: const Color(
                        0xffef4444,
                      ).withValues(alpha: .035),
                      side: BorderSide(
                        color: const Color(0xffef4444).withValues(alpha: .2),
                      ),
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(11),
                      ),
                    ),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        const Icon(LucideIcons.circle, size: 13),
                        const SizedBox(width: 7),
                        Flexible(
                          child: Text(
                            label,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              fontSize: 12,
                              fontWeight: FontWeight.w700,
                              color: Theme.of(context).colorScheme.onSurface,
                            ),
                          ),
                        ),
                        const SizedBox(width: 8),
                        const Icon(LucideIcons.chevronDown, size: 13),
                      ],
                    ),
                  );
                  final sendButton = DesktopTool(
                    icon: w.sending ? LucideIcons.square : LucideIcons.send,
                    label: w.sending ? 'Arrêter la réception' : 'Envoyer',
                    filled: true,
                    onPressed: uploading
                        ? null
                        : w.sending
                        ? w.stop
                        : send,
                  );
                  // Keep every control readable on narrow phones; larger screens
                  // use exactly the desktop's single toolbar.
                  if (constraints.maxWidth < 390) {
                    return Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Row(
                          children: [...controls, const Spacer(), sendButton],
                        ),
                        const SizedBox(height: 6),
                        Row(
                          children: [
                            Flexible(
                              flex: 0,
                              child: ConstrainedBox(
                                constraints: const BoxConstraints(
                                  maxWidth: 110,
                                ),
                                child: permissionButton(),
                              ),
                            ),
                            const SizedBox(width: 8),
                            Expanded(child: selector),
                          ],
                        ),
                      ],
                    );
                  }
                  return Row(
                    children: [
                      ...controls,
                      const Spacer(),
                      permissionButton(),
                      const SizedBox(width: 8),
                      Flexible(child: selector),
                      const SizedBox(width: 8),
                      sendButton,
                    ],
                  );
                },
              ),
            ],
          ),
        ),
        const SizedBox(height: 8),
        Text(
          'ARO peut faire des erreurs. Vérifiez les informations importantes.',
          textAlign: TextAlign.center,
          style: Theme.of(context).textTheme.bodySmall?.copyWith(fontSize: 10),
        ),
      ],
    ),
  );

  Future<void> attach() async {
    setState(() => uploading = true);
    await perform(context, () async {
      final picked = await FilePicker.platform.pickFiles(
        withData: true,
        allowMultiple: false,
      );
      if (picked == null) return;
      final file = picked.files.single;
      if (file.size > 20 * 1024 * 1024) {
        throw const ApiException(0, 'Le fichier dépasse 20 Mo.');
      }
      final bytes = file.bytes;
      if (bytes == null) {
        throw const ApiException(0, 'Le fichier ne peut pas être lu.');
      }
      final result = await w.api.upload(
        file.name,
        bytes,
        sha256.convert(bytes).toString(),
      );
      w.attachments.add({
        'fileId': result['id'],
        'mode': 'cloud-object',
        'displayName': file.name,
        'sizeBytes': file.size,
        'mimeType': 'application/octet-stream',
      });
    });
    if (mounted) setState(() => uploading = false);
  }

  Future<void> dictate() async {
    if (listening) {
      await speech.stop();
      if (mounted) setState(() => listening = false);
      return;
    }
    await perform(context, () async {
      final enabled = await speech.initialize(
        onStatus: (status) {
          if (mounted && (status == 'done' || status == 'notListening')) {
            setState(() => listening = false);
          }
        },
        onError: (error) {
          if (mounted) {
            setState(() => listening = false);
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(
                content: Text('Dictée indisponible : ${error.errorMsg}'),
              ),
            );
          }
        },
      );
      if (!enabled) {
        throw const ApiException(
          0,
          'Autorisez le microphone et la reconnaissance vocale dans les réglages du téléphone.',
        );
      }
      if (mounted) setState(() => listening = true);
      final prefix = composer.text;
      await speech.listen(
        listenOptions: SpeechListenOptions(localeId: 'fr_FR'),
        onResult: (result) {
          composer.text =
              '$prefix${prefix.isEmpty ? '' : ' '}${result.recognizedWords}';
          w.saveDraft(composer.text);
        },
      );
    });
  }
}
