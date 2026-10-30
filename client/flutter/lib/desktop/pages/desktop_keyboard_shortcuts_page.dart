
import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../common/widgets/keyboard_shortcuts/page_body.dart';

class DesktopKeyboardShortcutsPage extends StatefulWidget {
  const DesktopKeyboardShortcutsPage({Key? key}) : super(key: key);

  @override
  State<DesktopKeyboardShortcutsPage> createState() =>
      _DesktopKeyboardShortcutsPageState();
}

class _DesktopKeyboardShortcutsPageState
    extends State<DesktopKeyboardShortcutsPage> {
  final GlobalKey<KeyboardShortcutsPageBodyState> _bodyKey = GlobalKey();

  @override
  Widget build(BuildContext context) {
    final foregroundColor =
        AppBarTheme.of(context).titleTextStyle?.color ?? Colors.white;
    return Scaffold(
      appBar: AppBar(
        title: Text(translate('Keyboard shortcuts')),
        actions: [
          TextButton.icon(
            style: TextButton.styleFrom(foregroundColor: foregroundColor),
            onPressed: () =>
                _bodyKey.currentState?.resetToDefaultsWithConfirm(),
            icon: const Icon(Icons.restore),
            label: Text(translate('Reset to defaults')),
          ).marginOnly(right: 12),
        ],
      ),
      body: KeyboardShortcutsPageBody(
        key: _bodyKey,
        compact: true,
        showMasterToggles: false,
      ),
    );
  }
}
