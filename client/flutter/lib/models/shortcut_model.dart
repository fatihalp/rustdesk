import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../common.dart';
import '../common/shared_state.dart' show PrivacyModeState;
import '../common/widgets/dialog.dart'
    show desktopTryShowTabAuditDialogCloseCancelled;
import '../common/widgets/keyboard_shortcuts/shortcut_utils.dart';
import '../common/widgets/keyboard_shortcuts/shortcut_actions.dart';
import '../common/widgets/toolbar.dart'
    show allowDisplaySwitchInPrivacyMode, showVirtualDisplayMenu;
import '../consts.dart';
import '../desktop/widgets/remote_toolbar.dart' show ToolbarState;
import 'chat_model.dart' show ChatModel, MessageKey, VoiceCallStatus;
import '../desktop/widgets/tabbar_widget.dart' show DesktopTabController;
import '../models/model.dart';
import '../models/platform_model.dart';
import '../models/state_model.dart';

typedef ShortcutCallback = FutureOr<void> Function();

class ShortcutModel {
  static WeakReference<ShortcutModel>? _activeWebModel;

  final WeakReference<FFI> parent;
  final Map<String, ShortcutCallback> _callbacks = {};
  final Map<String, ShortcutCallback> _refreshers = {};

  ShortcutModel(this.parent);

  void register(String actionId, ShortcutCallback callback) {
    if (parent.target?.closed != false) return;
    _callbacks[actionId] = callback;
    _activeWebModel = WeakReference(this);
  }

  void unregister(String actionId) {
    _callbacks.remove(actionId);
  }

  void registerRefresher(List<String> actionIds, ShortcutCallback refresh) {
    if (parent.target?.closed != false) return;
    for (final actionId in actionIds) {
      _refreshers[actionId] = refresh;
    }
  }

  void clear() {
    _callbacks.clear();
    _refreshers.clear();
    if (identical(_activeWebModel?.target, this)) {
      _activeWebModel = null;
    }
  }

  static void onWebTriggered(String actionId) {
    final model = _activeWebModel?.target;
    if (model != null) {
      model.onTriggered(actionId);
    } else {
      debugPrint('shortcut_triggered: no active web shortcut model');
    }
  }

  void onTriggered(String actionId) {
    if (parent.target?.closed != false) return;
    unawaited(Future.sync(() async {
      final refresh = _refreshers[actionId];
      if (refresh != null) {
        await refresh();
        if (!identical(_refreshers[actionId], refresh)) return;
      }
      if (parent.target?.closed != false) return;
      final cb = _callbacks[actionId];
      if (cb != null) {
        await cb();
      } else {
        debugPrint('shortcut_triggered: no handler for $actionId');
      }
    }).catchError((e, st) {
      debugPrint('shortcut_triggered: handler failed for $actionId: $e\n$st');
    }));
  }

  static String _configRaw = '';
  static ShortcutConfig _config = ShortcutConfig.parse('');
  static ShortcutConfig _activeConfig = _config;

  static ShortcutConfig config({bool active = false}) {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    if (raw != _configRaw) {
      _config = ShortcutConfig.parse(raw);
      _activeConfig = _config.forPlatform(currentPlatformCapabilities());
      _configRaw = raw;
    }
    return active ? _activeConfig : _config;
  }

  static List<Map<String, dynamic>> readBindings() => [
        for (final binding in config().bindings)
          Map<String, dynamic>.of(binding)
      ];

  static bool isEnabled() => config().enabled;

  static bool isPassThrough() => config().passThrough;

  static Future<void> setPassThrough(bool v) async {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    Map<String, dynamic> json = {};
    if (raw.isNotEmpty) {
      try {
        json = jsonDecode(raw) as Map<String, dynamic>;
      } catch (_) {
        json = {};
      }
    }
    json['pass_through'] = v;
    await bind.mainSetLocalOption(
        key: kShortcutLocalConfigKey, value: jsonEncode(json));
    bind.mainReloadKeyboardShortcuts();
  }

  static Future<void> setEnabled(bool v) async {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    Map<String, dynamic> json = {};
    if (raw.isNotEmpty) {
      try {
        json = jsonDecode(raw) as Map<String, dynamic>;
      } catch (_) {
        json = {};
      }
    }
    json['enabled'] = v;
    if (v && shouldSeedDefaultShortcutBindings(json)) {
      json['bindings'] = filterDefaultBindingsForPlatform(
        jsonDecode(bind.mainGetDefaultKeyboardShortcuts()) as List,
        currentPlatformCapabilities(),
      );
    } else if (json.containsKey('bindings')) {
      json['bindings'] = shortcutBindingMapsFrom(json['bindings']);
    }
    await bind.mainSetLocalOption(
        key: kShortcutLocalConfigKey, value: jsonEncode(json));
    bind.mainReloadKeyboardShortcuts();
  }

  static ShortcutPlatformCapabilities currentPlatformCapabilities() {
    final desktopLayout = isDesktop || isWeb;
    return ShortcutPlatformCapabilities(
      includeFullscreenShortcut: desktopLayout,
      includeScreenshotShortcut: isDesktop,
      includeRelativeMouseModeShortcut:
          isDesktop && !(isLinux && bind.mainCurrentIsWayland()),
      includeTabShortcuts: isDesktop,
      includeToolbarShortcut: desktopLayout,
      includeCloseTabShortcut: isDesktop,
      includeSwitchSidesShortcut: isDesktop,
      includeRecordingShortcut: !isWeb && !isIOS,
      includeResetCanvasShortcut: isMobile,
      includePinToolbarShortcut: desktopLayout,
      includeViewModeShortcut: true,
      includeVoiceCallShortcut: !isWeb && !isIOS,
      excludedActions: Set.unmodifiable({
        if (!isDesktop) ...[
          kShortcutActionSwitchDisplayAll,
          kShortcutActionToggleEnableFileCopyPaste,
        ],
        if (isMobile) ...[
          kShortcutActionKeyboardModeLegacy,
          kShortcutActionKeyboardModeMap,
          kShortcutActionKeyboardModeTranslate,
          kShortcutActionToggleShowMyCursor,
          kShortcutActionToggleZoomCursor,
        ],
      }),
    );
  }
}

class ShortcutConfig {
  final bool enabled;
  final bool passThrough;
  final List<Map<String, dynamic>> bindings;

  const ShortcutConfig._(this.enabled, this.passThrough, this.bindings);

  ShortcutConfig forPlatform(ShortcutPlatformCapabilities capabilities) {
    final visible = allActionEntries(
            filterKeyboardShortcutActionGroupsForPlatform(capabilities))
        .map((entry) => entry.id)
        .toSet();
    final hidden = allActionEntries(kKeyboardShortcutActionGroups)
        .map((entry) => entry.id)
        .where((id) => !visible.contains(id))
        .toSet();
    return ShortcutConfig._(
      enabled,
      passThrough,
      List.unmodifiable(bindings.where((b) => !hidden.contains(b['action']))),
    );
  }

  static ShortcutConfig parse(String raw) {
    if (raw.isEmpty) return const ShortcutConfig._(false, false, []);
    try {
      final parsed = jsonDecode(raw) as Map<String, dynamic>;
      return ShortcutConfig._(
        parsed['enabled'] == true,
        parsed['pass_through'] == true,
        List.unmodifiable(shortcutBindingMapsFrom(parsed['bindings'])),
      );
    } catch (_) {
      return const ShortcutConfig._(false, false, []);
    }
  }
}

void registerSessionShortcutActions(
  FFI ffi, {
  DesktopTabController? tabController,
  ToolbarState? toolbarState,
}) {
  final sessionId = ffi.sessionId;


  if (isDesktop || isWeb) {
    ffi.shortcutModel.register(kShortcutActionToggleFullscreen, () {
      stateGlobal.setFullscreen(!stateGlobal.fullscreen.value);
    });
  }

  if (isDesktop) {
    ffi.shortcutModel.register(kShortcutActionToggleRecording, () {
      ffi.recordingModel.toggle();
    });
  }

  void openDisplay(int display, PeerInfo pi, {bool updateCursorPos = true}) {
    if (isDesktop &&
        pi.isSupportMultiDisplay &&
        bind.sessionGetDisplaysAsIndividualWindows(sessionId: sessionId) ==
            'Y') {
      openMonitorInNewTabOrWindow(display, ffi.id, pi);
    } else {
      openMonitorInTheSameTab(display, ffi, pi,
          updateCursorPos: updateCursorPos);
    }
  }

  void switchDisplayBy(int delta) {
    final pi = ffi.ffiModel.pi;
    final privacyMode = PrivacyModeState.find(ffi.id).value;
    if (privacyMode.isNotEmpty &&
        !allowDisplaySwitchInPrivacyMode(pi, privacyMode)) {
      return;
    }
    final count = pi.displays.length;
    if (count <= 1) return;
    final current = pi.currentDisplay;
    final int next;
    if (current == kAllDisplayValue) {
      next = delta > 0 ? 0 : count - 1;
    } else {
      next = ((current + delta) % count + count) % count;
    }
    openDisplay(next, pi, updateCursorPos: isMobile);
  }

  ffi.shortcutModel.register(kShortcutActionSwitchDisplayNext, () {
    switchDisplayBy(1);
  });
  ffi.shortcutModel.register(kShortcutActionSwitchDisplayPrev, () {
    switchDisplayBy(-1);
  });

  ffi.shortcutModel.register(kShortcutActionSwitchDisplayAll, () {
    final pi = ffi.ffiModel.pi;
    final privacyMode = PrivacyModeState.find(ffi.id).value;
    if (privacyMode.isNotEmpty &&
        !allowDisplaySwitchInPrivacyMode(pi, privacyMode)) {
      return;
    }
    if (!pi.isSupportMultiDisplay || pi.displays.length <= 1) return;
    if (pi.currentDisplay == kAllDisplayValue) return;
    openDisplay(kAllDisplayValue, pi);
  });

  if (tabController != null) {
    void switchTabBy(int delta) {
      final tabs = tabController.state.value.tabs;
      if (tabs.length <= 1) return;
      final cur = tabs.indexWhere((t) => t.key == ffi.id);
      if (cur < 0) return;
      final next = (cur + delta + tabs.length) % tabs.length;
      tabController.jumpTo(next);
    }

    ffi.shortcutModel
        .register(kShortcutActionSwitchTabNext, () => switchTabBy(1));
    ffi.shortcutModel
        .register(kShortcutActionSwitchTabPrev, () => switchTabBy(-1));

    ffi.shortcutModel.register(kShortcutActionCloseTab, () async {
      if (await desktopTryShowTabAuditDialogCloseCancelled(
        id: ffi.id,
        tabController: tabController,
      )) {
        return;
      }
      tabController.closeBy(ffi.id);
    });
  }

  if (toolbarState != null) {
    ffi.shortcutModel.register(kShortcutActionToggleToolbar, () {
      toolbarState.switchHide(sessionId);
    });
    ffi.shortcutModel.register(kShortcutActionPinToolbar, () {
      toolbarState.switchPin();
    });
  }

  ffi.shortcutModel.register(kShortcutActionToggleChat, () {
    ffi.chatModel.changeCurrentKey(MessageKey(ffi.id, ChatModel.clientModeID));
    ffi.chatModel.toggleChatOverlay();
  });

  if (!isWeb && !isIOS) {
    ffi.shortcutModel.register(kShortcutActionToggleVoiceCall, () {
      final status = ffi.chatModel.voiceCallStatus.value;
      if (status == VoiceCallStatus.connected ||
          status == VoiceCallStatus.waitingForResponse) {
        bind.sessionCloseVoiceCall(sessionId: sessionId);
      } else {
        bind.sessionRequestVoiceCall(sessionId: sessionId);
      }
    });
  }


  ffi.shortcutModel.register(kShortcutActionToggleShowMyCursor, () async {
    await bind.sessionToggleOption(
        sessionId: sessionId, value: kOptionToggleShowMyCursor);
    final showMyCursor = await bind.sessionGetToggleOption(
            sessionId: sessionId, arg: kOptionToggleShowMyCursor) ??
        false;
    ffi.ffiModel.setShowMyCursor(showMyCursor);
    if (showMyCursor && !ffi.ffiModel.viewOnly) {
      await bind.sessionToggleOption(
          sessionId: sessionId, value: kOptionToggleViewOnly);
      final viewOnly = await bind.sessionGetToggleOption(
              sessionId: sessionId, arg: kOptionToggleViewOnly) ??
          false;
      ffi.ffiModel.setViewOnly(ffi.id, viewOnly);
    }
  });

  void registerKeyboardMode(String actionId, String mode) {
    ffi.shortcutModel.register(actionId, () async {
      if (!ffi.ffiModel.keyboard ||
          ffi.ffiModel.viewOnly ||
          !bind.sessionIsKeyboardModeSupported(
              sessionId: sessionId, mode: mode) ||
          (ffi.ffiModel.pi.isWayland && mode != kKeyMapMode)) {
        return;
      }
      if (isInputSourceFlutter && isDesktop) {
        final modeOnly = bind.sessionIsKeyboardModeSupported(
                sessionId: sessionId, mode: kKeyMapMode)
            ? kKeyMapMode
            : kKeyLegacyMode;
        if (mode != modeOnly) return;
      }
      await bind.sessionSetKeyboardMode(sessionId: sessionId, value: mode);
      await ffi.inputModel.updateKeyboardMode();
    });
  }

  registerKeyboardMode(kShortcutActionKeyboardModeMap, kKeyMapMode);
  registerKeyboardMode(kShortcutActionKeyboardModeTranslate, kKeyTranslateMode);
  registerKeyboardMode(kShortcutActionKeyboardModeLegacy, kKeyLegacyMode);

  ffi.shortcutModel.register(kShortcutActionPlugOutAllVirtualDisplays, () {
    if (ffi.connType != ConnType.defaultConn ||
        !showVirtualDisplayMenu(ffi) ||
        PrivacyModeState.find(ffi.id).value.isNotEmpty ||
        (ffi.ffiModel.pi.isAmyuniIdd &&
            ffi.ffiModel.pi.amyuniVirtualDisplayCount == 0)) {
      return;
    }
    bind.sessionToggleVirtualDisplay(
      sessionId: sessionId,
      index: kAllVirtualDisplay,
      on: false,
    );
  });

  String? findPrivacyImpl(String nameKey) {
    final impls = ffi.ffiModel.pi
            .platformAdditions[kPlatformAdditionsSupportedPrivacyModeImpl]
        as List<dynamic>?;
    if (impls == null) return null;
    for (final e in impls) {
      if (e is List && e.length >= 2 && e[1] == nameKey) return e[0] as String;
    }
    return null;
  }

  bool checkPrivacyModeAllowed(String implKey) {
    final ffiModel = ffi.ffiModel;
    final pi = ffiModel.pi;
    final privacyMode = PrivacyModeState.find(ffi.id).value;
    if (ffi.connType != ConnType.defaultConn ||
        ffiModel.viewOnly ||
        (privacyMode.isEmpty &&
            (!pi.features.privacyMode || !ffiModel.keyboard))) {
      return false;
    }
    if (privacyMode == implKey) return true;
    if (ffiModel.permissions['privacy_mode'] == false) return false;
    if (allowDisplaySwitchInPrivacyMode(pi, implKey) ||
        (pi.currentDisplay == 0 &&
            !bind.sessionIsMultiUiSession(sessionId: sessionId))) {
      return true;
    }
    msgBox(sessionId, 'custom-nook-nocancel-hasclose', 'info',
        'Please switch to Display 1 first', '', ffi.dialogManager);
    return false;
  }

  ffi.shortcutModel.register(kShortcutActionPrivacyMode1, () {
    if (ffi.ffiModel.pi
            .platformAdditions[kPlatformAdditionsSupportedPrivacyModeImpl] ==
        null) {
      if (!checkPrivacyModeAllowed(kPrivacyModeImplMag)) return;
      bind.sessionToggleOption(sessionId: sessionId, value: 'privacy-mode');
      return;
    }
    final implKey = findPrivacyImpl('privacy_mode_impl_mag_tip');
    if (implKey == null) return;
    if (!checkPrivacyModeAllowed(implKey)) return;
    bind.sessionTogglePrivacyMode(
      sessionId: sessionId,
      implKey: implKey,
      on: PrivacyModeState.find(ffi.id).value != implKey,
    );
  });
  ffi.shortcutModel.register(kShortcutActionPrivacyMode2, () {
    final implKey = findPrivacyImpl('privacy_mode_impl_virtual_display_tip');
    if (implKey == null) return;
    if (!checkPrivacyModeAllowed(implKey)) return;
    bind.sessionTogglePrivacyMode(
      sessionId: sessionId,
      implKey: implKey,
      on: PrivacyModeState.find(ffi.id).value != implKey,
    );
  });

  ffi.shortcutModel.register(kShortcutActionToggleViewOnly, () async {
    await bind.sessionToggleOption(
        sessionId: sessionId, value: kOptionToggleViewOnly);
    final viewOnly = await bind.sessionGetToggleOption(
            sessionId: sessionId, arg: kOptionToggleViewOnly) ??
        false;
    ffi.ffiModel.setViewOnly(ffi.id, viewOnly);
    final showMyCursor = await bind.sessionGetToggleOption(
            sessionId: sessionId, arg: kOptionToggleShowMyCursor) ??
        false;
    ffi.ffiModel.setShowMyCursor(showMyCursor);
  });

  ffi.shortcutModel.register(kShortcutActionToggleReverseMouseWheel, () async {
    if (!ffi.ffiModel.keyboard || ffi.ffiModel.viewOnly) return;
    var cur = bind.sessionGetReverseMouseWheelSync(sessionId: sessionId) ?? '';
    if (cur == '') {
      cur = bind.mainGetUserDefaultOption(key: kKeyReverseMouseWheel);
    }
    final next = cur == 'Y' ? 'N' : 'Y';
    await bind.sessionSetReverseMouseWheel(sessionId: sessionId, value: next);
  });

  if (isDesktop && !isWeb) {
    ffi.shortcutModel.register(kShortcutActionToggleRelativeMouseMode, () {
      ffi.inputModel.toggleRelativeMouseMode();
    });
  }
}
