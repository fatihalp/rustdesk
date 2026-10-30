import 'dart:async';
import 'dart:convert';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/utils/relative_mouse_accumulator.dart';
import 'package:get/get.dart';

import '../common.dart';
import '../consts.dart';
import 'platform_model.dart';

class RelativeMouseModel {
  final SessionID sessionId;
  final RxBool enabled;

  final bool Function() keyboardPerm;
  final bool Function() isViewCamera;
  final String Function() peerVersion;
  final String? Function() peerPlatform;

  final Map<String, dynamic> Function(Map<String, dynamic> msg) modify;

  final bool Function() getPointerInsideImage;
  final void Function(bool inside) setPointerInsideImage;

  RelativeMouseModel({
    required this.sessionId,
    required this.enabled,
    required this.keyboardPerm,
    required this.isViewCamera,
    required this.peerVersion,
    required this.peerPlatform,
    required this.modify,
    required this.getPointerInsideImage,
    required this.setPointerInsideImage,
  });

  final RelativeMouseAccumulator _accumulator = RelativeMouseAccumulator();

  static MethodChannel? _hostChannel;
  static RelativeMouseModel? _activeNativeModel;
  static bool _hostChannelInitialized = false;

  static void initHostChannel() {
    if (!isMacOS) return;
    if (_hostChannelInitialized) return;
    _hostChannelInitialized = true;

    _hostChannel = const MethodChannel('org.supportdesk.supportdesk/host');
    _hostChannel!.setMethodCallHandler((call) async {
      if (call.method == 'onMouseDelta') {
        final args = call.arguments as Map<dynamic, dynamic>;
        final dx = args['dx'] as int;
        final dy = args['dy'] as int;
        _activeNativeModel?._onNativeMouseDelta(dx, dy);
      }
      return null;
    });
  }

  void _onNativeMouseDelta(int dx, int dy) {
    if (!enabled.value) return;
    _sendMouseMessageToSession({
      'type': 'move_relative',
      'x': '$dx',
      'y': '$dy',
    });
  }

  Future<bool> _enableNativeRelativeMouseMode() async {
    if (!isMacOS) return false;
    if (_hostChannel == null) {
      initHostChannel();
      if (_hostChannel == null) return false;
    }

    if (_activeNativeModel != null && _activeNativeModel != this) {
      debugPrint(
          '[RelMouse] Another model already has native relative mouse mode active');
      return false;
    }

    try {
      final result =
          await _hostChannel!.invokeMethod('enableNativeRelativeMouseMode');
      if (result == true) {
        _activeNativeModel = this;
        return true;
      }
    } catch (e) {
      debugPrint('[RelMouse] Failed to enable native relative mouse mode: $e');
    }
    return false;
  }

  Future<void> _disableNativeRelativeMouseMode() async {
    if (!isMacOS) return;
    if (_hostChannel == null) return;

    if (_activeNativeModel != this) {
      return;
    }

    try {
      await _hostChannel!.invokeMethod('disableNativeRelativeMouseMode');
    } catch (e) {
      debugPrint('[RelMouse] Failed to disable native relative mouse mode: $e');
    } finally {
      if (_activeNativeModel == this) {
        _activeNativeModel = null;
      }
    }
  }

  bool get _isNativeRelativeMouseModeActive =>
      isMacOS && _activeNativeModel == this;

  Offset? _pointerLockCenterLocal;
  Offset? _pointerLockCenterScreen;
  Offset? _pointerRegionTopLeftGlobal;
  Offset? _lastPointerLocalPos;

  bool _cursorClipApplied = false;

  bool _recenterInProgress = false;

  int _enableRequestId = 0;

  int _pendingDeltaX = 0;
  int _pendingDeltaY = 0;
  Timer? _throttleTimer;
  static const Duration _throttleInterval = Duration(milliseconds: 16);

  Size? _imageWidgetSize;

  DateTime? _lastToggle;

  bool _exitShortcutKeyDown = false;

  VoidCallback? onDisabled;

  bool get isSupported {
    if (isDesktop && isLinux && bind.mainCurrentIsWayland()) {
      return false;
    }
    final platform = peerPlatform();
    if (platform == kPeerPlatformLinux) {
      return false;
    }
    final v = peerVersion();
    if (v.isEmpty) return false;
    return versionCmp(v, kMinVersionForRelativeMouseMode) >= 0;
  }

  Size? get imageWidgetSize => _imageWidgetSize;

  void updateImageWidgetSize(Size size) {
    _imageWidgetSize = size;
    if (enabled.value) {
      _pointerLockCenterLocal = Offset(size.width / 2, size.height / 2);
    }
  }

  void updatePointerRegionTopLeftGlobal(PointerEvent e) {
    _pointerRegionTopLeftGlobal = e.position - e.localPosition;
  }

  bool _handleExitShortcut({
    required LogicalKeyboardKey logicalKey,
    required bool isKeyUp,
    required bool isKeyDown,
    required bool ctrlPressed,
    required bool altPressed,
    required bool commandPressed,
  }) {
    if (!isDesktop || !keyboardPerm() || isViewCamera()) return false;

    if (!enabled.value) return false;

    if (isKeyUp && _exitShortcutKeyDown) {
      _exitShortcutKeyDown = false;
      return true;
    }

    if (!isKeyDown) return false;

    if (isMacOS) {
      final isGKey = logicalKey == LogicalKeyboardKey.keyG;
      if (isGKey && commandPressed) {
        _exitShortcutKeyDown = true;
        setRelativeMouseMode(false);
        return true;
      }
      return false;
    }

    final isCtrlKey = logicalKey == LogicalKeyboardKey.controlLeft ||
        logicalKey == LogicalKeyboardKey.controlRight;
    final isAltKey = logicalKey == LogicalKeyboardKey.altLeft ||
        logicalKey == LogicalKeyboardKey.altRight;

    if ((isCtrlKey && altPressed) || (isAltKey && ctrlPressed)) {
      _exitShortcutKeyDown = true;
      setRelativeMouseMode(false);
      return true;
    }

    return false;
  }

  bool handleKeyEvent(
    KeyEvent e, {
    required bool ctrlPressed,
    required bool shiftPressed,
    required bool altPressed,
    required bool commandPressed,
  }) {
    return _handleExitShortcut(
      logicalKey: e.logicalKey,
      isKeyUp: e is KeyUpEvent,
      isKeyDown: e is KeyDownEvent,
      ctrlPressed: ctrlPressed,
      altPressed: altPressed,
      commandPressed: commandPressed,
    );
  }

  bool handleRawKeyEvent(RawKeyEvent e) {
    final modifiers = e.data;
    return _handleExitShortcut(
      logicalKey: e.logicalKey,
      isKeyUp: e is RawKeyUpEvent,
      isKeyDown: e is RawKeyDownEvent,
      ctrlPressed: modifiers.isControlPressed,
      altPressed: modifiers.isAltPressed,
      commandPressed: modifiers.isMetaPressed,
    );
  }

  void onEnterOrLeaveImage(bool enter) {
    if (!enabled.value) return;

    setPointerInsideImage(enter);

    if (_isNativeRelativeMouseModeActive) {
      return;
    }

    if (!enter) {
      _releaseCursorClip();
      return;
    }

    updatePointerLockCenter().then((_) {
      _recenterMouse();
    });
  }

  void onWindowBlur() {
    if (!enabled.value) return;

    setPointerInsideImage(false);
    if (!_isNativeRelativeMouseModeActive) {
      _releaseCursorClip();
    }
  }

  void onWindowFocus() {
    if (!enabled.value) return;

    if (_isNativeRelativeMouseModeActive) {
      setPointerInsideImage(false);
      return;
    }

    if (_imageWidgetSize == null) {
      _disableWithCleanup();
      return;
    }

    setPointerInsideImage(false);
    _releaseCursorClip();

    updatePointerLockCenter();
  }

  void toggleRelativeMouseMode() {
    final now = DateTime.now();
    if (_lastToggle != null &&
        now.difference(_lastToggle!).inMilliseconds <
            kRelativeMouseModeToggleDebounceMs) {
      return;
    }
    _lastToggle = now;
    setRelativeMouseMode(!enabled.value);
  }

  bool setRelativeMouseMode(bool value) {
    if (isWeb) {
      return false;
    }

    if (value) {
      if (!keyboardPerm() || isViewCamera()) {
        return false;
      }

      if (isDesktop && _imageWidgetSize == null) {
        showToast(translate('rel-mouse-not-ready-tip'));
        return false;
      }

      if (!isSupported) {
        showToast(translate('rel-mouse-not-supported-peer-tip'));
        return false;
      }
    }

    if (value) {
      try {
        if (isDesktop) {
          final requestId = ++_enableRequestId;
          if (isMacOS) {
            _enableNativeRelativeMouseMode().then((success) {
              if (_enableRequestId != requestId) {
                return;
              }
              if (success) {
                _completeEnableRelativeMouseMode();
              }
            });
          } else {
            if (!getPointerInsideImage()) {
              _releaseCursorClip();
            }

            updatePointerLockCenter().then((_) => _recenterMouse()).then((_) {
              if (_enableRequestId != requestId) {
                return;
              }
              _completeEnableRelativeMouseMode();
            }).catchError((e) {
              if (_enableRequestId != requestId) {
                return;
              }
              debugPrint('[RelMouse] Platform setup failed: $e');
              _resetState();
            });
          }
        } else {
          _completeEnableRelativeMouseMode();
        }
      } catch (e) {
        _disableWithCleanup();
        return false;
      }
    } else {
      _sendMouseMessageToSession(
        {
          'relative_mouse_mode': '0',
        },
        disableRelativeOnError: false,
        bypassKeyboardPerm: true,
      );

      if (isDesktop) {
        if (isMacOS) {
          _disableNativeRelativeMouseMode();
        } else {
          _releaseCursorClip();
        }
      }
      enabled.value = false;
      _resetState();
      onDisabled?.call();
    }

    return true;
  }

  void _completeEnableRelativeMouseMode() {
    enabled.value = true;

    if (isDesktop) {
      showToast(
          translate('rel-mouse-exit-{${isMacOS ? "Cmd+G" : "Ctrl+Alt"}}-tip'),
          alignment: Alignment.center);
    }

    _sendMouseMessageToSession(
      {
        'relative_mouse_mode': '1',
        'type': 'move_relative',
        'x': '0',
        'y': '0',
      },
      disableRelativeOnError: false,
    ).catchError((e) {
      debugPrint('[RelMouse] Failed to send enable marker: $e');
      return false;
    });
  }

  bool _skipNextMouseMove = false;

  bool handleRelativeMouseMove(Offset localPosition) {
    if (!enabled.value) return false;

    if (_isNativeRelativeMouseModeActive) {
      return true;
    }

    _ensurePointerLockEngaged();

    if (_skipNextMouseMove) {
      _skipNextMouseMove = false;
      _lastPointerLocalPos = localPosition;
      return true;
    }

    final lastLocal = _lastPointerLocalPos;
    _lastPointerLocalPos = localPosition;

    if (isLinux) {
      _recenterIfNearEdgeLinux(localPosition);
    }

    if (lastLocal != null) {
      final delta = localPosition - lastLocal;
      if (delta.dx != 0 || delta.dy != 0) {
        sendRelativeMouseMove(delta.dx, delta.dy);
      }
    }

    return true;
  }

  void _recenterIfNearEdgeLinux(Offset localPosition) {
    final size = _imageWidgetSize;
    if (size == null) return;

    final edgeThreshold = _calculateEdgeThreshold(size);

    final nearLeft = localPosition.dx < edgeThreshold;
    final nearRight = localPosition.dx > size.width - edgeThreshold;
    final nearTop = localPosition.dy < edgeThreshold;
    final nearBottom = localPosition.dy > size.height - edgeThreshold;

    if (nearLeft || nearRight || nearTop || nearBottom) {
      _recenterMouse();
    }
  }

  void sendRelativeMouseMove(double dx, double dy) {
    if (!isDesktop) return;

    final delta = _accumulator.add(dx, dy, maxDelta: kMaxRelativeMouseDelta);
    if (delta == null) return;

    _pendingDeltaX += delta.x;
    _pendingDeltaY += delta.y;

    if (_throttleTimer == null || !_throttleTimer!.isActive) {
      _throttleTimer = Timer(_throttleInterval, () => _flushPendingDelta());
    }
  }

  Future<void> _flushPendingDelta() async {
    if (!isDesktop) return;
    if (_pendingDeltaX == 0 && _pendingDeltaY == 0) return;

    final x = _pendingDeltaX;
    final y = _pendingDeltaY;
    _pendingDeltaX = 0;
    _pendingDeltaY = 0;

    final ok = await _sendMouseMessageToSession({
      'type': 'move_relative',
      'x': '$x',
      'y': '$y',
    });
    if (!ok) return;

    _recenterIfNearEdge();
  }

  static const double _edgeThresholdFraction = 0.1;
  static const double _edgeThresholdMax =
      100.0;
  static const double _edgeThresholdMin =
      20.0;

  static const double _edgeThresholdFractionLinux =
      0.25;
  static const double _edgeThresholdMaxLinux =
      200.0;
  static const double _edgeThresholdMinLinux =
      50.0;

  double _calculateEdgeThreshold(Size size) {
    final smallerDimension = math.min(size.width, size.height);
    if (isLinux) {
      final dynamicThreshold = smallerDimension * _edgeThresholdFractionLinux;
      return dynamicThreshold.clamp(
          _edgeThresholdMinLinux, _edgeThresholdMaxLinux);
    }
    final dynamicThreshold = smallerDimension * _edgeThresholdFraction;
    return dynamicThreshold.clamp(_edgeThresholdMin, _edgeThresholdMax);
  }

  void _recenterIfNearEdge() {
    final lastPos = _lastPointerLocalPos;
    final size = _imageWidgetSize;
    if (lastPos == null || size == null) return;

    final edgeThreshold = _calculateEdgeThreshold(size);

    final nearLeft = lastPos.dx < edgeThreshold;
    final nearRight = lastPos.dx > size.width - edgeThreshold;
    final nearTop = lastPos.dy < edgeThreshold;
    final nearBottom = lastPos.dy > size.height - edgeThreshold;

    if (nearLeft || nearRight || nearTop || nearBottom) {
      _recenterMouse();
    }
  }

  Future<void> sendRelativeMouseButton(Map<String, dynamic> evt) async {
    if (!enabled.value) return;
    _ensurePointerLockEngaged();

    final rawType = evt['type'];
    final rawButtons = evt['buttons'];
    if (rawType is! String || rawButtons is! int) return;

    final type = _mouseEventTypeToPeer(rawType);
    if (type.isEmpty) return;

    final buttons = mouseButtonsToPeer(rawButtons);
    if (buttons.isEmpty) return;

    await _sendMouseMessageToSession({
      'type': type,
      'buttons': buttons,
    });
  }

  static String _mouseEventTypeToPeer(String type) {
    switch (type) {
      case 'mousedown':
        return kMouseEventTypeDown;
      case 'mouseup':
        return kMouseEventTypeUp;
      default:
        return '';
    }
  }

  Future<bool> _sendMouseMessageToSession(
    Map<String, dynamic> msg, {
    bool disableRelativeOnError = true,
    bool bypassKeyboardPerm = false,
  }) async {
    if (!bypassKeyboardPerm && !keyboardPerm()) return false;
    if (isViewCamera()) return false;

    try {
      await bind.sessionSendMouse(
        sessionId: sessionId,
        msg: json.encode(modify(msg)),
      );
      return true;
    } catch (e) {
      debugPrint('[RelMouse] Error sending mouse message: $e');
      if (disableRelativeOnError && enabled.value) {
        _disableWithCleanup();
      }
      return false;
    }
  }

  static const int _recenterMaxRetries = 3;
  static const Duration _recenterRetryDelay = Duration(milliseconds: 100);

  Future<void> _recenterMouse() async {
    if (_recenterInProgress) return;
    _recenterInProgress = true;

    try {
      if (!enabled.value) return;
      if (!getPointerInsideImage()) return;

      final center = _pointerLockCenterScreen;
      if (center == null) {
        return;
      }

      for (int attempt = 0; attempt < _recenterMaxRetries; attempt++) {
        if (!enabled.value || !getPointerInsideImage()) return;

        final ok = bind.mainSetCursorPosition(
          x: center.dx.toInt(),
          y: center.dy.toInt(),
        );
        if (ok) {
          _skipNextMouseMove = true;
          return;
        }

        if (attempt < _recenterMaxRetries - 1) {
          await Future.delayed(_recenterRetryDelay);
        }
      }

      _disableWithCleanup();
      showToast(translate('rel-mouse-lock-failed-tip'));
    } catch (e, st) {
      debugPrint('[RelMouse] Unexpected error in _recenterMouse: $e\n$st');
    } finally {
      _recenterInProgress = false;
    }
  }

  Future<void> updatePointerLockCenter({Offset? localCenter}) async {
    if (!isDesktop) return;

    if (kWindowId == null) {
      if (enabled.value) {
        _disableWithCleanup();
      }
      return;
    }

    try {
      final wc = WindowController.fromWindowId(kWindowId!);
      final frame = await wc.getFrame();

      if (frame.width <= 0 || frame.height <= 0) {
        if (enabled.value) {
          _disableWithCleanup();
        }
        return;
      }

      if (localCenter != null) {
        _pointerLockCenterLocal = localCenter;
      } else if (_imageWidgetSize != null) {
        _pointerLockCenterLocal = Offset(
          _imageWidgetSize!.width / 2,
          _imageWidgetSize!.height / 2,
        );
      } else {
        if (enabled.value) {
          _disableWithCleanup();
        }
        return;
      }

      final view = ui.PlatformDispatcher.instance.views.firstOrNull;
      if (view == null) {
        debugPrint('[RelMouse] No view available for coordinate calculation');
        if (enabled.value) {
          _disableWithCleanup();
        }
        return;
      }
      final scale = view.devicePixelRatio;

      if (_pointerRegionTopLeftGlobal != null && scale > 0) {
        final centerInView =
            _pointerRegionTopLeftGlobal! + _pointerLockCenterLocal!;

        final clientPhysical = view.physicalSize;

        if (isMacOS) {
          final clientHeightPoints = clientPhysical.height / scale;
          final titleBarHeight = frame.height - clientHeightPoints;

          _pointerLockCenterScreen = Offset(
            frame.left + centerInView.dx,
            frame.top + titleBarHeight + centerInView.dy,
          );
        } else {
          final extraW = frame.width - clientPhysical.width;
          final extraH = frame.height - clientPhysical.height;
          final borderX = extraW > 0 ? extraW / 2 : 0.0;
          final borderBottom = borderX;
          final borderTop = extraH > borderBottom ? extraH - borderBottom : 0.0;
          final clientTopLeftScreen =
              Offset(frame.left + borderX, frame.top + borderTop);

          final tentativeCenter = Offset(
            clientTopLeftScreen.dx + centerInView.dx * scale,
            clientTopLeftScreen.dy + centerInView.dy * scale,
          );
          final withinFrame = tentativeCenter.dx >= frame.left &&
              tentativeCenter.dx <= frame.left + frame.width &&
              tentativeCenter.dy >= frame.top &&
              tentativeCenter.dy <= frame.top + frame.height;
          _pointerLockCenterScreen = withinFrame
              ? tentativeCenter
              : Offset(
                  frame.left + frame.width / 2, frame.top + frame.height / 2);
        }
      } else {
        _pointerLockCenterScreen = Offset(
          frame.left + frame.width / 2,
          frame.top + frame.height / 2,
        );
      }

      if (enabled.value && isWindows && getPointerInsideImage()) {
        _applyCursorClipForFrame(frame);
      } else if (enabled.value && isWindows && _cursorClipApplied) {
        _releaseCursorClip();
      }
    } catch (e) {
      if (enabled.value) {
        _disableWithCleanup();
      } else {
        _pointerLockCenterLocal = null;
        _pointerLockCenterScreen = null;
      }
    }
  }

  void _ensurePointerLockEngaged() {
    if (!enabled.value) return;
    if (!isDesktop) return;

    setPointerInsideImage(true);

    final needsCenter =
        _pointerLockCenterLocal == null || _pointerLockCenterScreen == null;
    final needsClip = isWindows && !_cursorClipApplied;
    if (needsCenter || needsClip) {
      updatePointerLockCenter()
          .then((_) => _recenterMouse())
          .catchError((Object e, StackTrace st) {
        debugPrint('[RelMouse] updatePointerLockCenter failed: $e\n$st');
        _disableWithCleanup();
      });
    }
  }

  void _applyCursorClipForFrame(Rect frame) {
    if (!isWindows) return;

    final view = ui.PlatformDispatcher.instance.views.firstOrNull;
    final scale = view?.devicePixelRatio ?? 1.0;

    final clientPhysical = view?.physicalSize ?? ui.Size.zero;

    final extraW = frame.width - clientPhysical.width;
    final extraH = frame.height - clientPhysical.height;

    final borderX = extraW > 0 ? extraW / 2 : 0.0;
    final borderBottom = borderX;
    final borderTop = extraH > borderBottom ? extraH - borderBottom : 0.0;

    final clientTopLeftScreen =
        Offset(frame.left + borderX, frame.top + borderTop);

    int left, top, right, bottom;

    if (_pointerRegionTopLeftGlobal != null &&
        _imageWidgetSize != null &&
        scale > 0) {
      left = (clientTopLeftScreen.dx + _pointerRegionTopLeftGlobal!.dx * scale)
          .toInt();
      top = (clientTopLeftScreen.dy + _pointerRegionTopLeftGlobal!.dy * scale)
          .toInt();
      right = (left + _imageWidgetSize!.width * scale).toInt();
      bottom = (top + _imageWidgetSize!.height * scale).toInt();
    } else {
      left = clientTopLeftScreen.dx.toInt();
      top = clientTopLeftScreen.dy.toInt();
      right = (frame.left + frame.width - borderX).toInt();
      bottom = (frame.top + frame.height - borderBottom).toInt();
    }

    _cursorClipApplied = bind.mainClipCursor(
      left: left,
      top: top,
      right: right,
      bottom: bottom,
      enable: true,
    );
  }

  void _releaseCursorClip() {
    if (!_cursorClipApplied) return;
    _cursorClipApplied = false;
    if (!isWindows) return;

    bind.mainClipCursor(
      left: 0,
      top: 0,
      right: 0,
      bottom: 0,
      enable: false,
    );
  }

  void _resetState() {
    if (_throttleTimer != null || _pendingDeltaX != 0 || _pendingDeltaY != 0) {
      _throttleTimer?.cancel();
      _throttleTimer = null;
      if (_pendingDeltaX != 0 || _pendingDeltaY != 0) {
        final x = _pendingDeltaX;
        final y = _pendingDeltaY;
        _pendingDeltaX = 0;
        _pendingDeltaY = 0;
        _sendMouseMessageToSession({
          'type': 'move_relative',
          'x': '$x',
          'y': '$y',
        }, disableRelativeOnError: false);
      }
    }
    _accumulator.reset();
    _pointerLockCenterLocal = null;
    _pointerLockCenterScreen = null;
    _pointerRegionTopLeftGlobal = null;
    _lastPointerLocalPos = null;
    _skipNextMouseMove = false;
    setPointerInsideImage(false);
    _cursorClipApplied = false;
    _exitShortcutKeyDown = false;
  }

  void _performCleanupCore() {
    _sendMouseMessageToSession(
      {
        'relative_mouse_mode': '0',
      },
      disableRelativeOnError: false,
      bypassKeyboardPerm: true,
    );

    if (isMacOS) {
      _disableNativeRelativeMouseMode();
    } else {
      _releaseCursorClip();
    }

    _resetState();
  }

  void _disableWithCleanup() {
    _performCleanupCore();
    enabled.value = false;
    onDisabled?.call();
  }

  bool _disposed = false;

  void dispose() {
    if (_disposed) return;
    _disposed = true;

    _performCleanupCore();
    _imageWidgetSize = null;
    _lastToggle = null;
    enabled.value = false;
    onDisabled?.call();
    onDisabled = null;
  }
}
