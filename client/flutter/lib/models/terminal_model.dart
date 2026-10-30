import 'dart:async';
import 'dart:convert';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:xterm/xterm.dart';

import 'input_modifier_utils.dart';
import 'model.dart';
import 'platform_model.dart';
import 'supportdesk_terminal.dart';
import 'terminal_copy_shortcut.dart';
import 'terminal_mouse_handler.dart';

bool canConfigureTerminalClipboardPermission({
  required bool settingsDisabled,
  required bool optionFixed,
}) =>
    !settingsDisabled && !optionFixed;

bool canHandleTerminalClipboardWriteRequest({
  required String localOption,
  required bool canConfigurePermission,
}) =>
    canConfigurePermission || localOption == kTerminalClipboardWriteAllowed;

TerminalClipboardWritePermission terminalClipboardWritePermission(
  String localOption, {
  required bool remoteClipboardEnabled,
  bool canRequestConsent = true,
}) {
  if (!remoteClipboardEnabled) {
    return TerminalClipboardWritePermission.denied;
  }
  if (localOption == kTerminalClipboardWriteAllowed) {
    return TerminalClipboardWritePermission.allowed;
  }
  if (localOption == kTerminalClipboardWriteUnconfigured && canRequestConsent) {
    return TerminalClipboardWritePermission.unconfigured;
  }
  return TerminalClipboardWritePermission.denied;
}

class TerminalModel with ChangeNotifier {
  final String id;
  final FFI parent;
  final int terminalId;
  late final Terminal terminal;
  late final TerminalController terminalController;

  bool _terminalOpened = false;
  bool get terminalOpened => _terminalOpened;

  bool _disposed = false;

  bool Function()? isCtrlLocked;

  void Function()? clearCtrlLock;

  bool Function()? isAltLocked;

  void Function()? clearAltLock;

  final _inputBuffer = <String>[];

  final _pendingOutputChunks = <String>[];
  final _pendingOutputSuppressFlags = <bool>[];
  int _pendingOutputSize = 0;
  static const int _kMaxOutputBufferChars = 8 * 1024;
  bool _terminalViewReady = false;
  bool _markViewReadyScheduled = false;
  bool _suppressTerminalOutput = false;
  bool _suppressNextTerminalDataOutput = false;

  void Function(int w, int h, int pw, int ph)? onResizeExternal;

  VoidCallback? onClosed;

  ValueChanged<String>? onClipboardWriteBlocked;
  ValueChanged<String>? onClipboardWriteSucceeded;

  Future<void> _handleInput(String data) async {
    if (_disposed) return;

    final ctrlLocked = isCtrlLocked?.call() ?? false;
    final altLocked = isAltLocked?.call() ?? false;
    final modifiersActive = ctrlLocked || altLocked;
    final shouldConsumeModifiers =
        modifiersActive && shouldApplyTerminalInputModifiers(data);
    data = prepareTerminalInputPayload(
      data,
      source: TerminalInputSource.keyboard,
      isMobileOrWebMobile: isMobile || (isWeb && !isWebDesktop),
      bracketedPasteMode: terminal.bracketedPasteMode,
      ctrlLocked: ctrlLocked,
      altLocked: altLocked,
    );
    if (shouldConsumeModifiers) {
      if (ctrlLocked) clearCtrlLock?.call();
      if (altLocked) clearAltLock?.call();
    }
    return _sendInputPayload(data);
  }

  Future<void> _sendInputPayload(String data) async {
    if (_disposed) return;

    if (_terminalOpened) {
      try {
        await bind.sessionSendTerminalInput(
          sessionId: parent.sessionId,
          terminalId: terminalId,
          data: data,
        );
      } catch (e) {
        debugPrint('[TerminalModel] Error sending terminal input: $e');
      }
    } else {
      debugPrint('[TerminalModel] Terminal not opened yet, buffering input');
      _inputBuffer.add(data);
    }
  }

  TerminalModel(this.parent, [this.terminalId = 0]) : id = parent.id {
    terminal = SupportDeskTerminal(
      maxLines: 10000,
      onClipboardWrite: writeTerminalClipboard,
      clipboardWritePermission: () => terminalClipboardWritePermission(
        bind.mainGetLocalOption(key: kOptionAllowTerminalClipboardWrite),
        remoteClipboardEnabled:
            parent.ffiModel.permissions['clipboard'] != false,
        canRequestConsent: onClipboardWriteBlocked != null,
      ),
      onClipboardWriteBlocked: (text) => onClipboardWriteBlocked?.call(text),
      onClipboardWriteSucceeded: (text) =>
          onClipboardWriteSucceeded?.call(text),
    );
    terminal.mouseHandler = const WheelButtonFixMouseHandler();
    terminalController = TerminalController();

    terminal.onOutput = (data) {
      if (_suppressTerminalOutput) return;
      _handleInput(data);
    };

    terminal.onResize = (w, h, pw, ph) async {
      if (w > 0 && h > 0 && pw > 0 && ph > 0) {
        debugPrint(
            '[TerminalModel] Terminal resized to ${w}x$h (pixel: ${pw}x$ph)');

        onResizeExternal?.call(w, h, pw, ph);

        if (!_terminalViewReady) {
          _scheduleMarkViewReady();
        }

        if (_terminalOpened) {
          try {
            await bind.sessionResizeTerminal(
              sessionId: parent.sessionId,
              terminalId: terminalId,
              rows: h,
              cols: w,
            );
          } catch (e) {
            debugPrint('[TerminalModel] Error resizing terminal: $e');
          }
        }
      } else {
        debugPrint(
            '[TerminalModel] Invalid terminal dimensions: ${w}x$h (pixel: ${pw}x$ph)');
      }
    };
  }

  void onReady() {
    parent.dialogManager.dismissAll();

    openTerminal(force: _terminalOpened).catchError((e) {
      debugPrint('[TerminalModel] Error opening terminal: $e');
    });
  }

  Future<void> openTerminal({bool force = false}) async {
    if (_terminalOpened && !force) return;

    int rows = 24;
    int cols = 80;

    if (terminal.viewHeight > 0) {
      rows = terminal.viewHeight;
    }
    if (terminal.viewWidth > 0) {
      cols = terminal.viewWidth;
    }

    debugPrint(
        '[TerminalModel] Opening terminal $terminalId, sessionId: ${parent.sessionId}, size: ${cols}x$rows');
    try {
      await bind
          .sessionOpenTerminal(
        sessionId: parent.sessionId,
        terminalId: terminalId,
        rows: rows,
        cols: cols,
      )
          .timeout(
        const Duration(seconds: 5),
        onTimeout: () {
          throw TimeoutException(
              'sessionOpenTerminal timed out after 5 seconds');
        },
      );
      debugPrint('[TerminalModel] sessionOpenTerminal called successfully');
    } catch (e) {
      debugPrint('[TerminalModel] Error calling sessionOpenTerminal: $e');
      if (e is TimeoutException) {
        _writeToTerminal('Failed to open terminal: Connection timeout\r\n');
      }
    }
  }

  Future<void> sendVirtualKey(String data) async {
    return _handleInput(data);
  }

  Future<void> pasteText(String data) async {
    final payload = prepareTerminalInputPayload(
      data,
      source: TerminalInputSource.paste,
      isMobileOrWebMobile: false,
      bracketedPasteMode: terminal.bracketedPasteMode,
      ctrlLocked: false,
      altLocked: false,
    );
    return _sendInputPayload(payload);
  }

  Future<void> closeTerminal() async {
    if (_terminalOpened) {
      try {
        await bind
            .sessionCloseTerminal(
          sessionId: parent.sessionId,
          terminalId: terminalId,
        )
            .timeout(
          const Duration(seconds: 3),
          onTimeout: () {
            throw TimeoutException(
                'sessionCloseTerminal timed out after 3 seconds');
          },
        );
        debugPrint('[TerminalModel] sessionCloseTerminal called successfully');
      } catch (e) {
        debugPrint('[TerminalModel] Error calling sessionCloseTerminal: $e');
      }
      _terminalOpened = false;
      notifyListeners();
    }
  }

  static int getTerminalIdFromEvt(Map<String, dynamic> evt) {
    if (evt.containsKey('terminal_id')) {
      final v = evt['terminal_id'];
      if (v is int) {
        return v;
      } else if (v is String) {
        final parsed = int.tryParse(v);
        if (parsed != null) {
          return parsed;
        } else {
          debugPrint(
              '[TerminalModel] Failed to parse terminal_id as integer: $v. Expected a numeric string.');
          return 0;
        }
      } else {
        debugPrint(
            '[TerminalModel] Unexpected terminal_id type: ${v.runtimeType}, value: $v. Expected int or String.');
        return 0;
      }
    } else {
      debugPrint('[TerminalModel] Event does not contain terminal_id');
      return 0;
    }
  }

  static bool getSuccessFromEvt(Map<String, dynamic> evt) {
    if (evt.containsKey('success')) {
      final v = evt['success'];
      if (v is bool) {
        return v;
      } else if (v is String) {
        return v.toLowerCase() == 'true';
      } else {
        debugPrint(
            '[TerminalModel] Unexpected success type: ${v.runtimeType}, value: $v. Expected bool or String.');
        return false;
      }
    } else {
      debugPrint('[TerminalModel] Event does not contain success');
      return false;
    }
  }

  static int getExitCodeFromEvt(Map<String, dynamic> evt) {
    if (evt.containsKey('exit_code')) {
      final v = evt['exit_code'];
      if (v is int) {
        return v;
      } else if (v is String) {
        final parsed = int.tryParse(v);
        if (parsed != null) {
          return parsed;
        } else {
          debugPrint(
              '[TerminalModel] Failed to parse exit_code as integer: $v. Expected a numeric string.');
          return 0;
        }
      } else {
        debugPrint(
            '[TerminalModel] Unexpected exit_code type: ${v.runtimeType}, value: $v. Expected int or String.');
        return 0;
      }
    } else {
      debugPrint('[TerminalModel] Event does not contain exit_code');
      return 0;
    }
  }

  void handleTerminalResponse(Map<String, dynamic> evt) {
    final String? type = evt['type'];
    final int evtTerminalId = getTerminalIdFromEvt(evt);

    if (evtTerminalId != terminalId) {
      debugPrint(
          '[TerminalModel] Ignoring event for terminal $evtTerminalId (not mine)');
      return;
    }

    switch (type) {
      case 'opened':
        _handleTerminalOpened(evt);
        break;
      case 'data':
        _handleTerminalData(evt);
        break;
      case 'closed':
        _handleTerminalClosed(evt);
        break;
      case 'error':
        _handleTerminalError(evt);
        break;
    }
  }

  void _handleTerminalOpened(Map<String, dynamic> evt) {
    final bool success = getSuccessFromEvt(evt);
    final String message = evt['message']?.toString() ?? '';
    final String? serviceId = evt['service_id']?.toString();

    debugPrint(
        '[TerminalModel] Terminal opened response: success=$success, message=$message, service_id=$serviceId');

    if (success) {
      _terminalOpened = true;

      final replayTerminalOutput = evt['replay_terminal_output'];
      _suppressNextTerminalDataOutput = replayTerminalOutput == true ||
          message == 'Reconnected to existing terminal with pending output';

      if (!_terminalViewReady &&
          terminal.viewWidth > 0 &&
          terminal.viewHeight > 0) {
        _scheduleMarkViewReady();
      }

      _processBufferedInputAsync().then((_) {
        notifyListeners();
      }).catchError((e) {
        debugPrint('[TerminalModel] Error processing buffered input: $e');
        notifyListeners();
      });

      final persistentSessions =
          (evt['persistent_sessions'] as List<dynamic>? ?? [])
              .whereType<int>()
              .where((id) => !parent.terminalModels.containsKey(id))
              .toList();
      if (kWindowId != null && persistentSessions.isNotEmpty) {
        DesktopMultiWindow.invokeMethod(
            kWindowId!,
            kWindowEventRestoreTerminalSessions,
            jsonEncode({
              'peer_id': id,
              'persistent_sessions': persistentSessions,
            }));
      }
    } else {
      _writeToTerminal('Failed to open terminal: $message\r\n');
    }
  }

  Future<void> _processBufferedInputAsync() async {
    final buffer = List<String>.from(_inputBuffer);
    _inputBuffer.clear();

    for (final data in buffer) {
      try {
        await bind.sessionSendTerminalInput(
          sessionId: parent.sessionId,
          terminalId: terminalId,
          data: data,
        );
      } catch (e) {
        debugPrint('[TerminalModel] Error sending buffered input: $e');
      }
    }
  }

  void _handleTerminalData(Map<String, dynamic> evt) {
    final data = evt['data'];

    if (data != null) {
      final suppressTerminalOutput = _suppressNextTerminalDataOutput;
      _suppressNextTerminalDataOutput = false;
      try {
        String text = '';
        if (data is String) {
          try {
            final bytes = base64Decode(data);
            text = utf8.decode(bytes, allowMalformed: true);
          } catch (e) {
            text = data;
          }
        } else if (data is List) {
          text = utf8.decode(List<int>.from(data), allowMalformed: true);
        } else {
          debugPrint('[TerminalModel] Unknown data type: ${data.runtimeType}');
          return;
        }

        _writeToTerminal(text, suppressTerminalOutput: suppressTerminalOutput);
      } catch (e) {
        debugPrint('[TerminalModel] Failed to process terminal data: $e');
      }
    }
  }

  void _writeToTerminal(
    String text, {
    bool suppressTerminalOutput = false,
  }) {
    if (!_terminalViewReady) {
      if (text.length >= _kMaxOutputBufferChars) {
        final truncated = text.substring(text.length - _kMaxOutputBufferChars);
        _pendingOutputChunks
          ..clear()
          ..add(truncated);
        _pendingOutputSuppressFlags
          ..clear()
          ..add(suppressTerminalOutput);
        _pendingOutputSize = truncated.length;
      } else {
        _pendingOutputChunks.add(text);
        _pendingOutputSuppressFlags.add(suppressTerminalOutput);
        _pendingOutputSize += text.length;
        while (_pendingOutputSize > _kMaxOutputBufferChars &&
            _pendingOutputChunks.length > 1) {
          final removed = _pendingOutputChunks.removeAt(0);
          _pendingOutputSuppressFlags.removeAt(0);
          _pendingOutputSize -= removed.length;
        }
      }
      return;
    }
    _writeTerminalChunk(text, suppressTerminalOutput: suppressTerminalOutput);
  }

  void _flushOutputBuffer() {
    if (_pendingOutputChunks.isEmpty) return;
    debugPrint(
        '[TerminalModel] Flushing $_pendingOutputSize buffered chars (${_pendingOutputChunks.length} chunks)');
    for (var i = 0; i < _pendingOutputChunks.length; i++) {
      _writeTerminalChunk(
        _pendingOutputChunks[i],
        suppressTerminalOutput: _pendingOutputSuppressFlags[i],
      );
    }
    _pendingOutputChunks.clear();
    _pendingOutputSuppressFlags.clear();
    _pendingOutputSize = 0;
  }

  void _writeTerminalChunk(
    String text, {
    required bool suppressTerminalOutput,
  }) {
    if (!suppressTerminalOutput) {
      terminal.write(text);
      return;
    }
    final previous = _suppressTerminalOutput;
    _suppressTerminalOutput = true;
    try {
      terminal.write(text);
    } finally {
      _suppressTerminalOutput = previous;
    }
  }

  void _scheduleMarkViewReady() {
    if (_disposed || _terminalViewReady || _markViewReadyScheduled) return;
    _markViewReadyScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _markViewReadyScheduled = false;
      if (_disposed || _terminalViewReady) return;
      if (terminal.viewWidth > 0 && terminal.viewHeight > 0) {
        _markViewReady();
      }
    });
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  void _markViewReady() {
    if (_terminalViewReady) return;
    _terminalViewReady = true;
    _flushOutputBuffer();
  }

  void _handleTerminalClosed(Map<String, dynamic> evt) {
    final int exitCode = getExitCodeFromEvt(evt);
    _writeToTerminal('\r\nTerminal closed with exit code: $exitCode\r\n');
    _terminalOpened = false;
    notifyListeners();
    onClosed?.call();
  }

  void _handleTerminalError(Map<String, dynamic> evt) {
    final String message = evt['message'] ?? 'Unknown error';
    _writeToTerminal('\r\nTerminal error: $message\r\n');
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    terminal.onOutput = null;
    terminal.onResize = null;
    isCtrlLocked = null;
    clearCtrlLock = null;
    isAltLocked = null;
    clearAltLock = null;
    onResizeExternal = null;
    onClosed = null;
    onClipboardWriteBlocked = null;
    onClipboardWriteSucceeded = null;
    _inputBuffer.clear();
    _pendingOutputChunks.clear();
    _pendingOutputSuppressFlags.clear();
    _pendingOutputSize = 0;
    _markViewReadyScheduled = false;
    _suppressNextTerminalDataOutput = false;
    super.dispose();
  }
}
