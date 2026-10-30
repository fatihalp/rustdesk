import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:xterm/xterm.dart';

enum TerminalClipboardWritePermission { denied, unconfigured, allowed }

class SupportDeskTerminal extends Terminal {
  SupportDeskTerminal({
    super.maxLines,
    required TerminalClipboardWritePermission Function()
        clipboardWritePermission,
    required Future<bool> Function(String) onClipboardWrite,
    ValueChanged<String>? onClipboardWriteBlocked,
    ValueChanged<String>? onClipboardWriteSucceeded,
  })  : _clipboardWritePermission = clipboardWritePermission,
        _onClipboardWrite = onClipboardWrite,
        _onClipboardWriteBlocked = onClipboardWriteBlocked,
        _onClipboardWriteSucceeded = onClipboardWriteSucceeded {
    onPrivateOSC = _handlePrivateOsc;
  }

  static const _clipboardOscCode = '52';
  static const _systemClipboardSelection = 'c';
  static const _maxClipboardWriteBytes = 16 * 1024 * 1024;
  static const _base64InputBytesPerBlock = 3;
  static const _base64EncodedCharsPerBlock = 4;
  static final _osc52Selection = RegExp(r'^[cpqs0-7]*$');
  final TerminalClipboardWritePermission Function() _clipboardWritePermission;
  final Future<bool> Function(String) _onClipboardWrite;
  final ValueChanged<String>? _onClipboardWriteBlocked;
  final ValueChanged<String>? _onClipboardWriteSucceeded;

  bool get isClipboardWriteAllowed =>
      _clipboardWritePermission() == TerminalClipboardWritePermission.allowed;

  void _handlePrivateOsc(String code, List<String> args) {
    if (code != _clipboardOscCode) return;
    if (args.length != 2 || !_osc52Selection.hasMatch(args.first)) {
      debugPrint('[SupportDeskTerminal] Rejected malformed OSC 52 command');
      return;
    }
    if (args.last == '?') {
      debugPrint('[SupportDeskTerminal] Rejected OSC 52 clipboard query');
      return;
    }
    final permission = _clipboardWritePermission();
    if (permission == TerminalClipboardWritePermission.denied) {
      debugPrint('[SupportDeskTerminal] Rejected unauthorized OSC 52 write');
      return;
    }
    final selection = args.first;
    if (selection.isNotEmpty &&
        !selection.contains(_systemClipboardSelection)) {
      debugPrint('[SupportDeskTerminal] Ignored unsupported OSC 52 selection');
      return;
    }
    if (selection.replaceAll(_systemClipboardSelection, '').isNotEmpty) {
      debugPrint('[SupportDeskTerminal] Ignored unsupported OSC 52 selections');
    }
    final text = _decodeClipboardPayload(args.last);
    if (text == null) return;
    if (permission == TerminalClipboardWritePermission.unconfigured) {
      debugPrint('[SupportDeskTerminal] Blocked OSC 52 write pending consent');
      _onClipboardWriteBlocked?.call(text);
      return;
    }
    unawaited(_writeClipboard(text));
  }

  Future<void> _writeClipboard(String text) async {
    final succeeded = await _onClipboardWrite(text);
    if (succeeded) {
      _onClipboardWriteSucceeded?.call(text);
      return;
    }
    debugPrint(
        '[SupportDeskTerminal] OSC 52 clipboard write requires interaction');
    _onClipboardWriteBlocked?.call(text);
  }

  String? _decodeClipboardPayload(String payload) {
    if (payload.length > _maxBase64EncodedLength(_maxClipboardWriteBytes)) {
      debugPrint('[SupportDeskTerminal] Rejected oversized OSC 52 payload');
      return null;
    }
    try {
      final bytes = base64.decode(payload);
      if (bytes.length > _maxClipboardWriteBytes) {
        debugPrint('[SupportDeskTerminal] Rejected oversized OSC 52 payload');
        return null;
      }
      return utf8.decode(bytes);
    } on FormatException {
      debugPrint('[SupportDeskTerminal] Rejected malformed OSC 52 payload');
      return null;
    }
  }

  static int _maxBase64EncodedLength(int maxBytes) =>
      ((maxBytes + _base64InputBytesPerBlock - 1) ~/
          _base64InputBytesPerBlock) *
      _base64EncodedCharsPerBlock;

  @override
  void eraseScrollbackOnly() {
    final scrollBack = buffer.scrollBack;
    if (scrollBack == 0) return;

    buffer.lines.remove(0, scrollBack);
  }
}
