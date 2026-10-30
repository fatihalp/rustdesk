
import 'package:flutter/material.dart';

import '../../common.dart';
import '../../common/widgets/keyboard_shortcuts/page_body.dart';

class MobileKeyboardShortcutsPage extends StatefulWidget {
  const MobileKeyboardShortcutsPage({Key? key}) : super(key: key);

  @override
  State<MobileKeyboardShortcutsPage> createState() =>
      _MobileKeyboardShortcutsPageState();
}

class _MobileKeyboardShortcutsPageState
    extends State<MobileKeyboardShortcutsPage> {
  final GlobalKey<KeyboardShortcutsPageBodyState> _bodyKey = GlobalKey();

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: Text(translate('Keyboard shortcuts')),
        actions: [
          IconButton(
            tooltip: translate('Reset to defaults'),
            onPressed: () =>
                _bodyKey.currentState?.resetToDefaultsWithConfirm(),
            icon: const Icon(Icons.restore),
          ),
        ],
      ),
      body: KeyboardShortcutsPageBody(
        key: _bodyKey,
        compact: false,
        editButtonHint: translate('shortcut-mobile-physical-keyboard-tip'),
        headerBanner: _PhysicalKeyboardHintBanner(theme: theme),
      ),
    );
  }
}

class _PhysicalKeyboardHintBanner extends StatelessWidget {
  final ThemeData theme;
  const _PhysicalKeyboardHintBanner({required this.theme});

  @override
  Widget build(BuildContext context) {
    final color = theme.colorScheme.primary.withOpacity(0.08);
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(Icons.info_outline,
              size: 18, color: theme.colorScheme.primary),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              translate('shortcut-mobile-physical-keyboard-tip'),
              style: TextStyle(color: theme.colorScheme.onSurface),
            ),
          ),
        ],
      ),
    );
  }
}
