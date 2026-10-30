import 'dart:convert';
import 'package:flutter/foundation.dart';
import '../../../consts.dart';
import '../../../models/platform_model.dart';
import 'shortcut_utils.dart';

class ShortcutDisplay {
  static String? _cachedRaw;
  static Map<String, dynamic>? _cachedParsed;

  @visibleForTesting
  static void resetCache() {
    _cachedRaw = null;
    _cachedParsed = null;
  }

  static String? formatFor(String actionId, {bool requireEnabled = true}) {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    if (raw.isEmpty) return null;
    Map<String, dynamic>? parsed;
    if (raw == _cachedRaw) {
      parsed = _cachedParsed;
    } else {
      try {
        parsed = jsonDecode(raw) as Map<String, dynamic>;
      } catch (_) {
        parsed = null;
      }
      _cachedRaw = raw;
      _cachedParsed = parsed;
    }
    if (parsed == null) return null;
    if (requireEnabled && parsed['enabled'] != true) return null;
    if (requireEnabled && parsed['pass_through'] == true) return null;
    final list = shortcutBindingMapsFrom(parsed['bindings']);
    final found = list.firstWhere(
      (b) => b['action'] == actionId,
      orElse: () => {},
    );
    if (found.isEmpty) return null;

    final keyValue = found['key'];
    if (keyValue is! String) return null;

    final isMac = defaultTargetPlatform == TargetPlatform.macOS ||
                  defaultTargetPlatform == TargetPlatform.iOS;
    final modsRaw = found['mods'];
    final mods = modsRaw is List
        ? modsRaw.whereType<String>().toList()
        : const <String>[];
    final parts = <String>[];
    for (final m in ['primary', 'ctrl', 'alt', 'shift']) {
      if (!mods.contains(m)) continue;
      switch (m) {
        case 'primary': parts.add(isMac ? 'Cmd' : 'Ctrl'); break;
        case 'ctrl':    parts.add(isMac ? 'Control' : 'Ctrl'); break;
        case 'alt':     parts.add(isMac ? 'Option' : 'Alt'); break;
        case 'shift':   parts.add('Shift'); break;
      }
    }
    parts.add(_keyDisplay(keyValue));
    return parts.join('+');
  }

  static String _keyDisplay(String key) {
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
}
