
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../common.dart';
import 'shortcut_utils.dart';

class RecordingResult {
  final Map<String, dynamic> binding;

  final String? clearActionId;

  RecordingResult(this.binding, this.clearActionId);
}

Future<RecordingResult?> showRecordingDialog({
  required BuildContext context,
  required String actionId,
  required String actionLabel,
  required List<Map<String, dynamic>> existingBindings,
  required String Function(String) actionLabelLookup,
}) {
  return showDialog<RecordingResult>(
    context: context,
    barrierDismissible: false,
    builder: (ctx) => _RecordingDialog(
      actionId: actionId,
      actionLabel: actionLabel,
      existingBindings: existingBindings,
      actionLabelLookup: actionLabelLookup,
    ),
  );
}

class _RecordingDialog extends StatefulWidget {
  final String actionId;
  final String actionLabel;
  final List<Map<String, dynamic>> existingBindings;
  final String Function(String) actionLabelLookup;

  const _RecordingDialog({
    required this.actionId,
    required this.actionLabel,
    required this.existingBindings,
    required this.actionLabelLookup,
  });

  @override
  State<_RecordingDialog> createState() => _RecordingDialogState();
}

class _RecordingDialogState extends State<_RecordingDialog> {
  final FocusNode _focusNode = FocusNode();

  Set<String> _mods = {};
  String? _key;

  String? _unsupportedKey;

  static final _modifierKeys = <LogicalKeyboardKey>{
    LogicalKeyboardKey.shift,
    LogicalKeyboardKey.shiftLeft,
    LogicalKeyboardKey.shiftRight,
    LogicalKeyboardKey.control,
    LogicalKeyboardKey.controlLeft,
    LogicalKeyboardKey.controlRight,
    LogicalKeyboardKey.alt,
    LogicalKeyboardKey.altLeft,
    LogicalKeyboardKey.altRight,
    LogicalKeyboardKey.meta,
    LogicalKeyboardKey.metaLeft,
    LogicalKeyboardKey.metaRight,
    LogicalKeyboardKey.capsLock,
    LogicalKeyboardKey.numLock,
    LogicalKeyboardKey.scrollLock,
    LogicalKeyboardKey.fn,
    LogicalKeyboardKey.fnLock,
  };

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _focusNode.requestFocus();
    });
  }

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  bool get _isMac =>
      defaultTargetPlatform == TargetPlatform.macOS ||
      defaultTargetPlatform == TargetPlatform.iOS;

  bool get _hasRequiredPrefix => _mods.isNotEmpty;

  String? get _conflictActionId {
    if (_key == null || !_hasRequiredPrefix) return null;
    for (final b in widget.existingBindings) {
      final otherAction = b['action'] as String?;
      if (otherAction == null || otherAction == widget.actionId) continue;
      final otherKey = b['key'] as String?;
      final otherMods = shortcutModSetFrom(b['mods']);
      if (otherKey == _key &&
          otherMods.length == _mods.length &&
          otherMods.containsAll(_mods)) {
        return otherAction;
      }
    }
    return null;
  }

  KeyEventResult _onKeyEvent(FocusNode node, KeyEvent event) {
    if (event is KeyDownEvent &&
        event.logicalKey == LogicalKeyboardKey.escape) {
      Navigator.of(context).pop();
      return KeyEventResult.handled;
    }
    if (event is! KeyDownEvent) return KeyEventResult.handled;

    final logical = event.logicalKey;
    final keyName = shortcutKeyNameForEvent(event);

    final mods = <String>{};
    if (HardwareKeyboard.instance.isAltPressed) mods.add('alt');
    if (HardwareKeyboard.instance.isShiftPressed) mods.add('shift');
    if (_isMac) {
      if (HardwareKeyboard.instance.isMetaPressed) mods.add('primary');
      if (HardwareKeyboard.instance.isControlPressed) mods.add('ctrl');
    } else {
      if (HardwareKeyboard.instance.isControlPressed) mods.add('primary');
    }

    setState(() {
      _mods = mods;
      if (!_isMac &&
          HardwareKeyboard.instance.logicalKeysPressed
              .contains(LogicalKeyboardKey.altGraph)) {
        _key = null;
        _unsupportedKey = LogicalKeyboardKey.altGraph.keyLabel;
        return;
      }
      _key = keyName;
      _unsupportedKey = null;
      if (keyName == null && !_modifierKeys.contains(logical)) {
        final label = logical.keyLabel.isNotEmpty
            ? logical.keyLabel
            : (logical.debugName ?? 'this key');
        _unsupportedKey = label;
      }
    });
    return KeyEventResult.handled;
  }

  KeyEventResult _onRawKeyEvent(FocusNode node, RawKeyEvent event) {
    final data = event.data;
    const gtkMod5Mask = 1 << 7;
    if (event is RawKeyDownEvent &&
        data is RawKeyEventDataLinux &&
        data.keyHelper is GtkKeyHelper &&
        (data.modifiers & gtkMod5Mask) != 0) {
      setState(() {
        _key = null;
        _unsupportedKey = LogicalKeyboardKey.altGraph.keyLabel;
      });
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  void _onSave() {
    if (_key == null || !_hasRequiredPrefix) return;
    final ordered = canonicalShortcutModsForSave(_mods);
    final binding = <String, dynamic>{
      'action': widget.actionId,
      'mods': ordered,
      'key': _key!,
    };
    Navigator.of(context).pop(RecordingResult(binding, _conflictActionId));
  }

  String _formatPrefix() {
    if (_isMac) return 'Cmd / Control / Option / Shift';
    return 'Ctrl / Alt / Shift';
  }

  String _formatCombo() {
    final parts = <String>[];
    for (final m in ['primary', 'ctrl', 'alt', 'shift']) {
      if (!_mods.contains(m)) continue;
      switch (m) {
        case 'primary':
          parts.add(_isMac ? 'Cmd' : 'Ctrl');
          break;
        case 'ctrl':
          parts.add(_isMac ? 'Control' : 'Ctrl');
          break;
        case 'alt':
          parts.add(_isMac ? 'Option' : 'Alt');
          break;
        case 'shift':
          parts.add('Shift');
          break;
      }
    }
    if (_key != null) {
      parts.add(_keyDisplay(_key!));
    }
    if (parts.isEmpty) return translate('shortcut-recording-press-keys-tip');
    return parts.join('+');
  }

  String _keyDisplay(String key) {
    switch (key) {
      case 'delete':     return 'Del';
      case 'backspace':  return 'Backspace';
      case 'enter':      return 'Enter';
      case 'tab':        return 'Tab';
      case 'space':      return 'Space';
      case 'arrow_left': return 'Left';
      case 'arrow_right':return 'Right';
      case 'arrow_up':   return 'Up';
      case 'arrow_down': return 'Down';
      case 'home':       return 'Home';
      case 'end':        return 'End';
      case 'page_up':    return 'PgUp';
      case 'page_down':  return 'PgDn';
      case 'insert':     return 'Ins';
    }
    if (key.startsWith('digit')) return key.substring(5);
    return key.toUpperCase();
  }

  @override
  Widget build(BuildContext context) {
    final hasKey = _key != null;
    final conflictId = _conflictActionId;
    final hasConflict = conflictId != null;
    final canSave = hasKey && _hasRequiredPrefix;

    Widget statusLine;
    if (_unsupportedKey != null) {
      statusLine = Row(
        children: [
          const Icon(Icons.close, size: 16, color: Colors.red),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              translate('shortcut-key-not-supported')
                  .replaceAll('{}', _unsupportedKey!),
              style: const TextStyle(color: Colors.red),
            ),
          ),
        ],
      );
    } else if (!hasKey) {
      statusLine = Text(
        translate('shortcut-recording-press-keys-tip'),
        style: TextStyle(color: Theme.of(context).hintColor),
      );
    } else if (!_hasRequiredPrefix) {
      statusLine = Row(
        children: [
          Icon(Icons.close, size: 16, color: Colors.red),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              translate('shortcut-must-include-modifiers')
                  .replaceAll('{}', _formatPrefix()),
              style: const TextStyle(color: Colors.red),
            ),
          ),
        ],
      );
    } else if (hasConflict) {
      final otherLabel = widget.actionLabelLookup(conflictId);
      statusLine = Row(
        children: [
          Icon(Icons.warning_amber_outlined,
              size: 16, color: Colors.orange.shade700),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              '${translate('shortcut-already-bound-to')} "$otherLabel"',
              style: TextStyle(color: Colors.orange.shade700),
            ),
          ),
        ],
      );
    } else {
      statusLine = Row(
        children: [
          const Icon(Icons.check, size: 16, color: Colors.green),
          const SizedBox(width: 6),
          Text(translate('Valid'), style: const TextStyle(color: Colors.green)),
        ],
      );
    }

    final saveLabel = hasConflict ? 'Replace' : 'Save';

    return AlertDialog(
      title: Text(
        '${translate('Set shortcut')}: ${widget.actionLabel}',
      ),
      content: Focus(
        focusNode: _focusNode,
        autofocus: true,
        onKeyEvent: _onKeyEvent,
        onKey: _onRawKeyEvent,
        child: ConstrainedBox(
          constraints: const BoxConstraints(minWidth: 380),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(translate('shortcut-recording-instruction')),
              const SizedBox(height: 12),
              Container(
                width: double.infinity,
                padding:
                    const EdgeInsets.symmetric(vertical: 18, horizontal: 12),
                decoration: BoxDecoration(
                  border: Border.all(color: Theme.of(context).dividerColor),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  _formatCombo(),
                  textAlign: TextAlign.center,
                  style: TextStyle(
                    fontSize: 18,
                    fontWeight: FontWeight.w600,
                    color: hasKey
                        ? Theme.of(context).textTheme.titleLarge?.color
                        : Theme.of(context).hintColor,
                  ),
                ),
              ),
              const SizedBox(height: 12),
              statusLine,
            ],
          ),
        ),
      ),
      actions: [
        dialogButton('Cancel',
            onPressed: () => Navigator.of(context).pop(), isOutline: true),
        dialogButton(saveLabel, onPressed: canSave ? _onSave : null),
      ],
    );
  }
}
