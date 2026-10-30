import 'shortcut_constants.dart';
import 'shortcut_utils.dart';

sealed class KeyboardShortcutActionGroupChild {
  const KeyboardShortcutActionGroupChild();
}

class KeyboardShortcutActionEntry extends KeyboardShortcutActionGroupChild {
  final String id;
  final String labelKey;
  const KeyboardShortcutActionEntry(this.id, this.labelKey);
}

class KeyboardShortcutActionSubgroup extends KeyboardShortcutActionGroupChild {
  final String titleKey;
  final List<KeyboardShortcutActionEntry> entries;
  const KeyboardShortcutActionSubgroup(this.titleKey, this.entries);
}

class KeyboardShortcutActionGroup {
  final String titleKey;
  final List<KeyboardShortcutActionGroupChild> children;
  const KeyboardShortcutActionGroup(this.titleKey, this.children);
}

final List<KeyboardShortcutActionGroup> kKeyboardShortcutActionGroups = [
  KeyboardShortcutActionGroup('Monitor', [
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchDisplayNext, 'Switch to next display'),
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchDisplayPrev, 'Switch to previous display'),
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchDisplayAll, 'All monitors'),
  ]),
  KeyboardShortcutActionGroup('Control Actions', [
    KeyboardShortcutActionEntry(
        kShortcutActionSendClipboardKeystrokes, 'Send clipboard keystrokes'),
    KeyboardShortcutActionEntry(kShortcutActionResetCanvas, 'Reset canvas'),
    KeyboardShortcutActionEntry(
        kShortcutActionSendCtrlAltDel, 'Insert Ctrl + Alt + Del'),
    KeyboardShortcutActionEntry(
        kShortcutActionRestartRemote, 'Restart remote device'),
    KeyboardShortcutActionEntry(kShortcutActionInsertLock, 'Insert Lock'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleBlockInput, 'Block user input'),
    KeyboardShortcutActionEntry(kShortcutActionSwitchSides, 'Switch Sides'),
    KeyboardShortcutActionEntry(kShortcutActionRefresh, 'Refresh'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleRecording, 'Toggle session recording'),
    KeyboardShortcutActionEntry(kShortcutActionScreenshot, 'Take screenshot'),
  ]),
  KeyboardShortcutActionGroup('Display', [
    KeyboardShortcutActionSubgroup('View Mode', [
      KeyboardShortcutActionEntry(
          kShortcutActionViewModeOriginal, 'Scale original'),
      KeyboardShortcutActionEntry(
          kShortcutActionViewModeAdaptive, 'Scale adaptive'),
      KeyboardShortcutActionEntry(
          kShortcutActionViewModeCustom, 'Scale custom'),
    ]),
    KeyboardShortcutActionSubgroup('Image Quality', [
      KeyboardShortcutActionEntry(
          kShortcutActionImageQualityBest, 'Good image quality'),
      KeyboardShortcutActionEntry(
          kShortcutActionImageQualityBalanced, 'Balanced'),
      KeyboardShortcutActionEntry(
          kShortcutActionImageQualityLow, 'Optimize reaction time'),
    ]),
    KeyboardShortcutActionSubgroup('Codec', [
      KeyboardShortcutActionEntry(kShortcutActionCodecAuto, 'Auto'),
      KeyboardShortcutActionEntry(kShortcutActionCodecVp8, 'VP8'),
      KeyboardShortcutActionEntry(kShortcutActionCodecVp9, 'VP9'),
      KeyboardShortcutActionEntry(kShortcutActionCodecAv1, 'AV1'),
      KeyboardShortcutActionEntry(kShortcutActionCodecH264, 'H264'),
      KeyboardShortcutActionEntry(kShortcutActionCodecH265, 'H265'),
    ]),
    KeyboardShortcutActionSubgroup('Virtual display', [
      KeyboardShortcutActionEntry(
          kShortcutActionPlugOutAllVirtualDisplays, 'Plug out all'),
    ]),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleShowRemoteCursor, 'Show remote cursor'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleFollowRemoteCursor, 'Follow remote cursor'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleFollowRemoteWindow, 'Follow remote window focus'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleZoomCursor, 'Zoom cursor'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleQualityMonitor, 'Show quality monitor'),
    KeyboardShortcutActionEntry(kShortcutActionToggleMute, 'Mute'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleEnableFileCopyPaste, 'Enable file copy and paste'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleDisableClipboard, 'Disable clipboard'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleLockAfterSessionEnd, 'Lock after session end'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleTrueColor, 'True color (4:4:4)'),
    KeyboardShortcutActionSubgroup('Privacy mode', [
      KeyboardShortcutActionEntry(
          kShortcutActionPrivacyMode1, 'privacy_mode_impl_mag_tip'),
      KeyboardShortcutActionEntry(
          kShortcutActionPrivacyMode2, 'privacy_mode_impl_virtual_display_tip'),
    ]),
  ]),
  KeyboardShortcutActionGroup('Keyboard', [
    KeyboardShortcutActionSubgroup('Keyboard mode', [
      KeyboardShortcutActionEntry(
          kShortcutActionKeyboardModeLegacy, 'Legacy mode'),
      KeyboardShortcutActionEntry(kShortcutActionKeyboardModeMap, 'Map mode'),
      KeyboardShortcutActionEntry(
          kShortcutActionKeyboardModeTranslate, 'Translate mode'),
    ]),
    KeyboardShortcutActionEntry(kShortcutActionToggleViewOnly, 'View Mode'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleShowMyCursor, 'Show my cursor'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleSwapCtrlCmd, 'Swap control-command key'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleRelativeMouseMode, 'Relative mouse mode'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleReverseMouseWheel, 'Reverse mouse wheel'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleSwapLeftRightMouse, 'swap-left-right-mouse'),
  ]),
  KeyboardShortcutActionGroup('Chat', [
    KeyboardShortcutActionEntry(kShortcutActionToggleChat, 'Text chat'),
    KeyboardShortcutActionEntry(kShortcutActionToggleVoiceCall, 'Voice call'),
  ]),
  KeyboardShortcutActionGroup('Other', [
    KeyboardShortcutActionEntry(kShortcutActionPinToolbar, 'Pin Toolbar'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleFullscreen, 'Toggle fullscreen'),
    KeyboardShortcutActionEntry(kShortcutActionToggleToolbar, 'Toggle toolbar'),
    KeyboardShortcutActionEntry(kShortcutActionCloseTab, 'Close tab'),
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchTabNext, 'Switch to next tab'),
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchTabPrev, 'Switch to previous tab'),
  ]),
];

Iterable<KeyboardShortcutActionEntry> allActionEntries(
  Iterable<KeyboardShortcutActionGroup> groups,
) sync* {
  for (final group in groups) {
    for (final child in group.children) {
      switch (child) {
        case KeyboardShortcutActionEntry():
          yield child;
        case KeyboardShortcutActionSubgroup():
          yield* child.entries;
      }
    }
  }
}

List<KeyboardShortcutActionGroup> filterKeyboardShortcutActionGroupsForPlatform(
  ShortcutPlatformCapabilities cap,
) {
  bool allowed(String id) {
    if (cap.excludedActions.contains(id)) return false;
    if (!cap.includeFullscreenShortcut &&
        id == kShortcutActionToggleFullscreen) {
      return false;
    }
    if (!cap.includeScreenshotShortcut && id == kShortcutActionScreenshot) {
      return false;
    }
    if (!cap.includeRelativeMouseModeShortcut &&
        id == kShortcutActionToggleRelativeMouseMode) {
      return false;
    }
    if (!cap.includeTabShortcuts && isSwitchTabShortcutAction(id)) return false;
    if (!cap.includeToolbarShortcut && id == kShortcutActionToggleToolbar) {
      return false;
    }
    if (!cap.includeCloseTabShortcut && id == kShortcutActionCloseTab) {
      return false;
    }
    if (!cap.includeSwitchSidesShortcut && id == kShortcutActionSwitchSides) {
      return false;
    }
    if (!cap.includeRecordingShortcut && id == kShortcutActionToggleRecording) {
      return false;
    }
    if (!cap.includeResetCanvasShortcut && id == kShortcutActionResetCanvas) {
      return false;
    }
    if (!cap.includePinToolbarShortcut && id == kShortcutActionPinToolbar) {
      return false;
    }
    if (!cap.includeViewModeShortcut &&
        (id == kShortcutActionViewModeOriginal ||
            id == kShortcutActionViewModeAdaptive ||
            id == kShortcutActionViewModeCustom)) {
      return false;
    }
    if (!cap.includeVoiceCallShortcut && id == kShortcutActionToggleVoiceCall) {
      return false;
    }
    return true;
  }

  final out = <KeyboardShortcutActionGroup>[];
  for (final group in kKeyboardShortcutActionGroups) {
    final filteredChildren = <KeyboardShortcutActionGroupChild>[];
    for (final child in group.children) {
      switch (child) {
        case KeyboardShortcutActionEntry():
          if (allowed(child.id)) filteredChildren.add(child);
        case KeyboardShortcutActionSubgroup():
          final entries =
              child.entries.where((e) => allowed(e.id)).toList();
          if (entries.isNotEmpty) {
            filteredChildren.add(
                KeyboardShortcutActionSubgroup(child.titleKey, entries));
          }
      }
    }
    if (filteredChildren.isNotEmpty) {
      out.add(KeyboardShortcutActionGroup(group.titleKey, filteredChildren));
    }
  }
  return out;
}
