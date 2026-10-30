import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

enum TerminalInputSource {
  keyboard,
  paste,
}

bool shouldReleaseStaleMobileShift({
  required bool isMobile,
  required bool cachedShiftPressed,
  required bool actualShiftPressed,
  required LogicalKeyboardKey logicalKey,
  required bool hasTrackedShiftKeyDown,
}) {
  if (!isMobile || !cachedShiftPressed || actualShiftPressed) {
    return false;
  }
  if (!hasTrackedShiftKeyDown) {
    return false;
  }
  if (logicalKey == LogicalKeyboardKey.shiftLeft ||
      logicalKey == LogicalKeyboardKey.shiftRight) {
    return false;
  }
  return true;
}

String applyTerminalInputModifiers(
  String data, {
  required bool ctrlLocked,
  required bool altLocked,
}) {
  var result = data;
  if (ctrlLocked) {
    result = _applyTerminalCtrlModifier(result);
  }
  if (altLocked) {
    result = '\x1B$result';
  }
  return result;
}

String terminalPastePayload(String text, {required bool bracketedPasteMode}) {
  if (!bracketedPasteMode) {
    return text;
  }
  return '\x1B[200~$text\x1B[201~';
}

bool shouldApplyTerminalInputModifiers(String data) {
  if (data.characters.length != 1) return false;
  final codeUnit = data.codeUnitAt(0);
  return codeUnit >= 0x20 && codeUnit != 0x7F;
}

String prepareTerminalInputPayload(
  String data, {
  required TerminalInputSource source,
  required bool isMobileOrWebMobile,
  required bool bracketedPasteMode,
  required bool ctrlLocked,
  required bool altLocked,
}) {
  if (source == TerminalInputSource.paste) {
    return terminalPastePayload(
      data,
      bracketedPasteMode: bracketedPasteMode,
    );
  }

  var result = data;
  if (isMobileOrWebMobile && result == '\n') {
    result = '\r';
  }
  if ((ctrlLocked || altLocked) && shouldApplyTerminalInputModifiers(result)) {
    result = applyTerminalInputModifiers(
      result,
      ctrlLocked: ctrlLocked,
      altLocked: altLocked,
    );
  }
  return result;
}

bool shouldHandleTerminalPasteShortcut({
  required TargetPlatform platform,
  required LogicalKeyboardKey logicalKey,
  required bool isKeyDown,
  required bool isKeyRepeat,
  required bool controlPressed,
  required bool metaPressed,
  required bool altPressed,
  required bool shiftPressed,
  required bool modifierLockActive,
}) {
  if (!modifierLockActive) return false;
  if (!isKeyDown && !isKeyRepeat) return false;
  if (logicalKey != LogicalKeyboardKey.keyV) return false;
  if (altPressed) return false;
  switch (platform) {
    case TargetPlatform.linux:
      return controlPressed && !metaPressed && shiftPressed;
    case TargetPlatform.iOS:
    case TargetPlatform.macOS:
      return !controlPressed && metaPressed && !shiftPressed;
    case TargetPlatform.android:
    case TargetPlatform.fuchsia:
    case TargetPlatform.windows:
      return controlPressed && !metaPressed && !shiftPressed;
  }
}

bool shouldClearTerminalModifiersWhenRow3Collapses({
  required bool wasExpanded,
  required bool willExpand,
  required bool ctrlLocked,
  required bool altLocked,
}) {
  return wasExpanded && !willExpand && (ctrlLocked || altLocked);
}

String _applyTerminalCtrlModifier(String data) {
  final graphemes = data.characters.toList(growable: false);
  if (graphemes.length != 1) {
    return data;
  }

  final runes = graphemes.single.runes.toList(growable: false);
  if (runes.length != 1) {
    return data;
  }

  final code = runes.single;
  if (code >= 0x61 && code <= 0x7A) {
    return String.fromCharCode(code - 0x60);
  }
  if (code >= 0x41 && code <= 0x5A) {
    return String.fromCharCode(code - 0x40);
  }
  if (code == 0x20) {
    return String.fromCharCode(0);
  }
  if (code == 0x5B) {
    return String.fromCharCode(27);
  }
  if (code == 0x5C) {
    return String.fromCharCode(28);
  }
  if (code == 0x5D) {
    return String.fromCharCode(29);
  }
  if (code == 0x5E) {
    return String.fromCharCode(30);
  }
  if (code == 0x5F || code == 0x2F) {
    return String.fromCharCode(31);
  }
  return data;
}
