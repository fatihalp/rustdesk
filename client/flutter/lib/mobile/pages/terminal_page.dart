import 'dart:async';
import 'dart:math';
import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/models/input_modifier_utils.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/terminal_copy_shortcut.dart';
import 'package:flutter_hbb/models/terminal_model.dart';
import 'package:flutter_hbb/models/terminal_mouse_handler.dart';
import 'package:flutter_hbb/mobile/terminal_keyboard_utils.dart';
import 'package:flutter_hbb/web/dummy.dart'
    if (dart.library.html) 'package:flutter_hbb/web/terminal_font.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:xterm/xterm.dart';
import '../../desktop/pages/terminal_connection_manager.dart';
import '../../consts.dart';

const _terminalBackgroundOpacity = 0.7;

Widget _buildTerminalViewForPlatform({
  required bool reportMouseInput,
  required bool reportTouchInput,
  required Terminal terminal,
  required TerminalController controller,
  required TerminalStyle textStyle,
  required EdgeInsets padding,
  required bool deleteDetection,
  required Map<ShortcutActivator, Intent>? shortcuts,
  required FocusOnKeyEventCallback onKeyEvent,
  required void Function(TapDownDetails, CellOffset) onSecondaryTapDown,
}) {
  if (reportMouseInput || reportTouchInput) {
    return TerminalMouseInteraction(
      terminal,
      controller: controller,
      autofocus: true,
      textStyle: textStyle,
      deleteDetection: deleteDetection,
      reportTouchInput: reportTouchInput,
      shortcuts: shortcuts,
      onKeyEvent: onKeyEvent,
      backgroundOpacity: _terminalBackgroundOpacity,
      padding: padding,
      onSecondaryTapDown: onSecondaryTapDown,
    );
  }
  return TerminalView(
    terminal,
    controller: controller,
    autofocus: true,
    textStyle: textStyle,
    deleteDetection: deleteDetection,
    shortcuts: shortcuts,
    onKeyEvent: onKeyEvent,
    backgroundOpacity: _terminalBackgroundOpacity,
    padding: padding,
    onSecondaryTapDown: onSecondaryTapDown,
  );
}

class TerminalPage extends StatefulWidget {
  const TerminalPage({
    Key? key,
    required this.id,
    required this.password,
    required this.isSharedPassword,
    this.forceRelay,
    this.connToken,
  }) : super(key: key);
  final String id;
  final String? password;
  final bool? forceRelay;
  final bool? isSharedPassword;
  final String? connToken;
  final terminalId = 0;

  @override
  State<TerminalPage> createState() => _TerminalPageState();
}

class _TerminalPageState extends State<TerminalPage>
    with AutomaticKeepAliveClientMixin, WidgetsBindingObserver {
  bool get _canConfigureTerminalClipboardPermission =>
      canConfigureTerminalClipboardPermission(
        settingsDisabled: bind.isDisableSettings(),
        optionFixed: isOptionFixed(kOptionAllowTerminalClipboardWrite),
      );
  bool get _canHandleTerminalClipboardWriteRequest =>
      canHandleTerminalClipboardWriteRequest(
        localOption: bind.mainGetLocalOption(
          key: kOptionAllowTerminalClipboardWrite,
        ),
        canConfigurePermission: _canConfigureTerminalClipboardPermission,
      );

  late FFI _ffi;
  late TerminalModel _terminalModel;
  double? _cellHeight;
  double _sysKeyboardHeight = 0;
  Timer? _keyboardDebounce;
  final GlobalKey _keyboardKey = GlobalKey();
  double _keyboardHeight = 0;
  late bool _showTerminalExtraKeys;
  bool _ctrlLocked = false;
  bool _altLocked = false;
  bool _row3Expanded = false;
  double _swipeStartX = 0;
  double _swipeCurrentX = 0;
  ScaffoldFeatureController<MaterialBanner, MaterialBannerClosedReason>?
      _terminalClipboardNoticeController;
  final _terminalClipboardNotice = TerminalClipboardNoticeCoordinator<int>();

  final String _robotoMonoFontFamily = isWeb
      ? (GoogleFonts.robotoMono().fontFamily ?? 'monospace')
      : 'monospace';

  SessionID get sessionId => _ffi.sessionId;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);

    if (isWeb) {
      loadLocalTerminalFontIfNeeded();
    }

    debugPrint(
        '[TerminalPage] Initializing terminal ${widget.terminalId} for peer ${widget.id}');

    _ffi = TerminalConnectionManager.getConnection(
      peerId: widget.id,
      password: widget.password,
      isSharedPassword: widget.isSharedPassword,
      forceRelay: widget.forceRelay,
      connToken: widget.connToken,
    );

    _terminalModel = TerminalModel(_ffi, widget.terminalId);
    if (_canHandleTerminalClipboardWriteRequest) {
      _terminalModel.onClipboardWriteBlocked =
          _handleTerminalClipboardWriteBlocked;
      _terminalModel.onClipboardWriteSucceeded =
          _handleTerminalClipboardWriteSucceeded;
    }
    debugPrint(
        '[TerminalPage] Terminal model created for terminal ${widget.terminalId}');

    _terminalModel.onResizeExternal = (w, h, pw, ph) {
      _cellHeight = ph * 1.0;
    };

    _ffi.registerTerminalModel(widget.terminalId, _terminalModel);

    _terminalModel.onClosed = () {
      if (mounted) {
        closeConnection(id: widget.id);
      }
    };

    _showTerminalExtraKeys = !isWebDesktop &&
        mainGetLocalBoolOptionSync(kOptionEnableShowTerminalExtraKeys);
    _terminalModel.isCtrlLocked = () => _ctrlLocked;
    _terminalModel.clearCtrlLock = () {
      if (_ctrlLocked) setState(() => _ctrlLocked = false);
    };
    _terminalModel.isAltLocked = () => _altLocked;
    _terminalModel.clearAltLock = () {
      if (_altLocked) setState(() => _altLocked = false);
    };
    _row3Expanded =
        bind.mainGetLocalOption(key: kOptionShowTerminalCtrlKeys) == 'Y';
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _ffi.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);

      if (_showTerminalExtraKeys) {
        _updateKeyboardHeight();
      }
    });
    _ffi.ffiModel.updateEventListener(_ffi.sessionId, widget.id);
  }

  void _handleTerminalClipboardWriteBlocked(String clipboardText) {
    if (!mounted) return;
    final option = bind.mainGetLocalOption(
      key: kOptionAllowTerminalClipboardWrite,
    );
    final request = _terminalClipboardNotice.recordBlocked(
      source: widget.terminalId,
      text: clipboardText,
      option: option,
      canWrite: (_) => _canWriteTerminalClipboard,
    );
    if (request != null) _showTerminalClipboardNotice(request);
  }

  void _showTerminalClipboardNotice(
    TerminalClipboardNoticeRequest<int> request,
  ) {
    final controller = ScaffoldMessenger.of(context).showMaterialBanner(
      MaterialBanner(
        leading: const Icon(Icons.content_copy_outlined),
        content: Text(translate(kTerminalClipboardNoticeMessageKey)),
        actions: [
          AnimatedBuilder(
            animation: _terminalClipboardNotice,
            builder: (_, __) => TextButton(
              onPressed: _terminalClipboardNotice.canClaimAction
                  ? _handleTerminalClipboardNegativeAction
                  : null,
              child: Text(translate(request.negativeActionKey)),
            ),
          ),
          AnimatedBuilder(
            animation: _terminalClipboardNotice,
            builder: (_, __) => TextButton(
              onPressed: _terminalClipboardNotice.canClaimAction
                  ? _handleTerminalClipboardPositiveAction
                  : null,
              child: Text(translate(request.actionKey)),
            ),
          ),
        ],
      ),
    );
    _terminalClipboardNoticeController = controller;
    unawaited(controller.closed.then<void>((_) {
      if (identical(_terminalClipboardNoticeController, controller)) {
        _terminalClipboardNoticeController = null;
        _terminalClipboardNotice.noticeClosed();
      }
    }));
  }

  void _handleTerminalClipboardNegativeAction() {
    final request = _terminalClipboardNotice.claimCurrentAction();
    if (request == null) return;
    if (request.persistAllowed) {
      unawaited(_declineTerminalClipboardWrite());
    } else {
      _closeTerminalClipboardNotice();
    }
  }

  void _handleTerminalClipboardPositiveAction() {
    final request = _terminalClipboardNotice.claimCurrentAction();
    if (request == null) return;
    unawaited(_completeTerminalClipboardWrite(request));
  }

  bool get _canWriteTerminalClipboard =>
      _canHandleTerminalClipboardWriteRequest &&
      !_ffi.closed &&
      _ffi.ffiModel.permissions['clipboard'] != false;

  void _handleTerminalClipboardWriteSucceeded(String _) {
    _closeTerminalClipboardNotice();
  }

  Future<void> _declineTerminalClipboardWrite() async {
    try {
      await bind.mainSetLocalOption(
        key: kOptionAllowTerminalClipboardWrite,
        value: kTerminalClipboardWriteDenied,
      );
    } catch (error) {
      debugPrint(
          '[TerminalPage] Failed to save terminal clipboard permission: $error');
      return;
    } finally {
      _terminalClipboardNotice.releaseAction();
    }
    _closeTerminalClipboardNotice();
  }

  Future<void> _completeTerminalClipboardWrite(
    TerminalClipboardNoticeRequest<int> request,
  ) async {
    var completed = false;
    try {
      completed = await completeTerminalClipboardWrite(
        clipboardText: request.text,
        canWrite: () => _canWriteTerminalClipboard,
        writeClipboard: writeTerminalClipboard,
        persistAllowed: request.persistAllowed
            ? () => bind.mainSetLocalOption(
                  key: kOptionAllowTerminalClipboardWrite,
                  value: kTerminalClipboardWriteAllowed,
                )
            : null,
      );
    } catch (error) {
      debugPrint(
          '[TerminalPage] Failed to complete terminal clipboard write: $error');
    } finally {
      _terminalClipboardNotice.releaseAction();
    }
    if (!completed) return;
    _closeTerminalClipboardNotice();
  }

  void _closeTerminalClipboardNotice() {
    if (!_terminalClipboardNotice.beginClose()) return;
    final controller = _terminalClipboardNoticeController;
    if (controller == null) {
      debugPrint('[TerminalPage] Clipboard notice controller is missing');
      _terminalClipboardNotice.noticeClosed();
      return;
    }
    controller.close();
  }

  @override
  void dispose() {
    _ffi.unregisterTerminalModel(widget.terminalId);
    _terminalModel.dispose();
    _keyboardDebounce?.cancel();
    _terminalClipboardNotice.clear();
    _terminalClipboardNoticeController?.close();
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
    TerminalConnectionManager.releaseConnection(widget.id);
  }

  @override
  void didChangeMetrics() {
    super.didChangeMetrics();

    _keyboardDebounce?.cancel();
    _keyboardDebounce = Timer(const Duration(milliseconds: 20), () {
      final bottomInset = MediaQuery.of(context).viewInsets.bottom;
      setState(() {
        _sysKeyboardHeight = bottomInset;
      });
    });
  }

  void _updateKeyboardHeight() {
    if (_keyboardKey.currentContext != null) {
      final renderBox = _keyboardKey.currentContext!.findRenderObject() as RenderBox;
      _keyboardHeight = renderBox.size.height;
    }
  }

  EdgeInsets _calculatePadding(double heightPx) {
    if (_cellHeight == null) {
      return const EdgeInsets.symmetric(horizontal: 5.0, vertical: 2.0);
    }
    final realHeight = heightPx - _sysKeyboardHeight - _keyboardHeight;
    final rows = (realHeight / _cellHeight!).floor();
    final extraSpace = realHeight - rows * _cellHeight!;
    final topBottom = max(0.0, extraSpace / 2.0);
    return EdgeInsets.only(left: 5.0, right: 5.0, top: topBottom, bottom: topBottom + _sysKeyboardHeight + _keyboardHeight);
  }

  Future<void> _pasteClipboardText() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    final text = data?.text;
    if (text == null || !mounted) return;

    await _terminalModel.pasteText(text);
    if (mounted) {
      _terminalModel.terminalController.clearSelection();
    }
  }

  KeyEventResult _handleTerminalKeyEvent(FocusNode _, KeyEvent event) {
    final hardwareKeyboard = HardwareKeyboard.instance;
    final shouldPaste = shouldHandleTerminalPasteShortcut(
      platform: defaultTargetPlatform,
      logicalKey: event.logicalKey,
      isKeyDown: event is KeyDownEvent,
      isKeyRepeat: event is KeyRepeatEvent,
      controlPressed: hardwareKeyboard.isControlPressed,
      metaPressed: hardwareKeyboard.isMetaPressed,
      altPressed: hardwareKeyboard.isAltPressed,
      shiftPressed: hardwareKeyboard.isShiftPressed,
      modifierLockActive: _ctrlLocked || _altLocked,
    );
    if (!shouldPaste) return KeyEventResult.ignored;

    unawaited(_pasteClipboardText());
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return WillPopScope(
      onWillPop: () async {
        clientClose(sessionId, _ffi);
        return false;
      },
      child: buildBody(),
    );
  }

  Widget buildBody() {
    final scaffold = Scaffold(
      resizeToAvoidBottomInset: false,
      backgroundColor: Theme.of(context).scaffoldBackgroundColor,
      body: Stack(
        children: [
          Positioned.fill(
            child: SafeArea(
              top: true,
              child: LayoutBuilder(
                builder: (context, constraints) {
                  final heightPx = constraints.maxHeight;
                  return _buildTerminalViewForPlatform(
                    reportMouseInput: isWebDesktop || isAndroid,
                    reportTouchInput: isIOS,
                    terminal: _terminalModel.terminal,
                    controller: _terminalModel.terminalController,
                    textStyle: _getTerminalStyle(),
                    deleteDetection: isIOS,
                    shortcuts: platformTerminalShortcuts(),
                    onKeyEvent: terminalCopyHandler(
                      _terminalModel.terminal,
                      _terminalModel.terminalController,
                      fallback: _handleTerminalKeyEvent,
                    ),
                    padding: _calculatePadding(heightPx),
                    onSecondaryTapDown: (details, offset) async {
                      final selection = _terminalModel.terminalController.selection;
                      if (selection != null) {
                        final text = _terminalModel.terminal.buffer.getText(selection);
                        _terminalModel.terminalController.clearSelection();
                        await Clipboard.setData(ClipboardData(text: text));
                      } else {
                        await _pasteClipboardText();
                      }
                    },
                  );
                },
              ),
            ),
          ),
          if (_showTerminalExtraKeys) _buildFloatingKeyboard(),
          if (isIOS) _buildCloseButton(),
        ],
      ),
    );

    if (isIOS) {
      return LayoutBuilder(
        builder: (context, constraints) {
          final screenWidth = constraints.maxWidth;
          final edgeThreshold = (screenWidth * 0.1).clamp(20.0, 80.0);
          final swipeThreshold = (screenWidth * 0.25).clamp(80.0, 300.0);

          return RawGestureDetector(
            behavior: HitTestBehavior.translucent,
            gestures: <Type, GestureRecognizerFactory>{
              HorizontalDragGestureRecognizer: GestureRecognizerFactoryWithHandlers<HorizontalDragGestureRecognizer>(
                () => HorizontalDragGestureRecognizer(
                  debugOwner: this,
                  supportedDevices: kTouchBasedDeviceKinds,
                ),
                (HorizontalDragGestureRecognizer instance) {
                  instance
                    ..onDown = (details) {
                      _swipeStartX = details.localPosition.dx;
                      _swipeCurrentX = details.localPosition.dx;
                    }
                    ..onUpdate = (details) {
                      _swipeCurrentX = details.localPosition.dx;
                    }
                    ..onEnd = (details) {
                      if (_swipeStartX < edgeThreshold && (_swipeCurrentX - _swipeStartX) > swipeThreshold) {
                        clientClose(sessionId, _ffi);
                      }
                      _swipeStartX = 0;
                      _swipeCurrentX = 0;
                    }
                    ..onCancel = () {
                      _swipeStartX = 0;
                      _swipeCurrentX = 0;
                    };
                },
              ),
            },
            child: scaffold,
          );
        },
      );
    }

    return scaffold;
  }

  Widget _buildCloseButton() {
    return Positioned(
      top: 0,
      right: 0,
      child: SafeArea(
        minimum: const EdgeInsets.only(
          top: 16,
          right: 16,
        ),
        child: Semantics(
          button: true,
          label: translate('Close'),
          child: Container(
            width: 44,
            height: 44,
            decoration: BoxDecoration(
              color: Colors.black.withOpacity(0.5),
              shape: BoxShape.circle,
            ),
            child: Material(
              color: Colors.transparent,
              shape: const CircleBorder(),
              clipBehavior: Clip.antiAlias,
              child: InkWell(
                customBorder: const CircleBorder(),
                onTap: () {
                  clientClose(sessionId, _ffi);
                },
                child: Tooltip(
                  message: translate('Close'),
                  child: const Icon(
                    Icons.chevron_left,
                    color: Colors.white,
                    size: 28,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildFloatingKeyboard() {
    return AnimatedPositioned(
      duration: const Duration(milliseconds: 200),
      left: 0,
      right: 0,
      bottom: _sysKeyboardHeight,
      child: Container(
        key: _keyboardKey,
        color: Theme.of(context).scaffoldBackgroundColor,
        padding: EdgeInsets.zero,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: _buildKeyboardKeyButtons(terminalKeyboardRow1Keys),
            ),
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                ..._buildKeyboardKeyButtons(terminalKeyboardRow2Keys),
                const SizedBox(width: terminalKeyboardKeySpacing),
                _buildCollapseButton(),
              ],
            ),
            if (_row3Expanded)
              Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  ..._buildKeyboardKeyButtons(terminalKeyboardRow3Keys),
                  for (var i = 0;
                      i < terminalKeyboardRow3TrailingPlaceholderCount;
                      i++) ...[
                    const SizedBox(width: terminalKeyboardKeySpacing),
                    const SizedBox(width: terminalKeyboardKeyWidth),
                  ],
                ],
              ),
          ],
        ),
      ),
    );
  }

  Widget _buildCtrlKeyButton() {
    return _buildModifierToggleButton(
      text: 'Ctrl',
      semanticsLabel: 'Ctrl',
      isLocked: _ctrlLocked,
      onPressed: () => setState(() => _ctrlLocked = !_ctrlLocked),
    );
  }

  Widget _buildAltKeyButton() {
    return _buildModifierToggleButton(
      text: 'Alt',
      semanticsLabel: 'Alt',
      isLocked: _altLocked,
      onPressed: () => setState(() => _altLocked = !_altLocked),
    );
  }

  void _toggleRow3Expanded() {
    final willExpand = !_row3Expanded;
    final shouldClearModifiers = shouldClearTerminalModifiersWhenRow3Collapses(
      wasExpanded: _row3Expanded,
      willExpand: willExpand,
      ctrlLocked: _ctrlLocked,
      altLocked: _altLocked,
    );
    setState(() {
      _row3Expanded = willExpand;
      if (shouldClearModifiers) {
        _ctrlLocked = false;
        _altLocked = false;
      }
    });
    mainSetLocalBoolOption(kOptionShowTerminalCtrlKeys, willExpand);

    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_showTerminalExtraKeys) return;
      setState(() {
        _updateKeyboardHeight();
      });
    });
  }

  Widget _buildCollapseButton() {
    return Semantics(
      label: translate('Show terminal extra keys'),
      toggled: _row3Expanded,
      child: ElevatedButton(
        onPressed: _toggleRow3Expanded,
        child: Text(_row3Expanded ? '∧' : '∨'),
        style: ElevatedButton.styleFrom(
          minimumSize: const Size(terminalKeyboardKeyWidth, 32),
          padding: EdgeInsets.zero,
          textStyle: const TextStyle(fontSize: 12),
          backgroundColor:
              Theme.of(context).colorScheme.surfaceContainerHighest,
          foregroundColor: Theme.of(context).colorScheme.onSurfaceVariant,
        ),
      ),
    );
  }

  List<Widget> _buildKeyboardKeyButtons(List<String> labels) {
    return [
      for (var i = 0; i < labels.length; i++) ...[
        _buildKeyButton(labels[i]),
        if (i < labels.length - 1)
          const SizedBox(width: terminalKeyboardKeySpacing),
      ],
    ];
  }

  Widget _buildModifierToggleButton({
    required String text,
    required String semanticsLabel,
    required bool isLocked,
    required VoidCallback onPressed,
  }) {
    return Semantics(
      label: semanticsLabel,
      toggled: isLocked,
      child: ElevatedButton(
        onPressed: onPressed,
        child: Text(text),
        style: ElevatedButton.styleFrom(
          minimumSize: const Size(terminalKeyboardKeyWidth, 32),
          padding: EdgeInsets.zero,
          textStyle: const TextStyle(fontSize: 12),
          backgroundColor: isLocked
              ? Colors.blue
              : Theme.of(context).colorScheme.surfaceContainerHighest,
          foregroundColor: isLocked
              ? Colors.white
              : Theme.of(context).colorScheme.onSurfaceVariant,
        ),
      ),
    );
  }

  Widget _buildKeyButton(String label) {
    if (label == 'Ctrl') return _buildCtrlKeyButton();
    if (label == 'Alt') return _buildAltKeyButton();

    return ElevatedButton(
      onPressed: () {
        _sendKeyToTerminal(label);
      },
      child: Text(label),
      style: ElevatedButton.styleFrom(
        minimumSize: const Size(terminalKeyboardKeyWidth, 32),
        padding: EdgeInsets.zero,
        textStyle: const TextStyle(fontSize: 12),
        backgroundColor:
            Theme.of(context).colorScheme.surfaceContainerHighest,
        foregroundColor: Theme.of(context).colorScheme.onSurfaceVariant,
      ),
    );
  }

  void _sendKeyToTerminal(String key) {
    String send;

    switch (key) {
      case 'Esc':
        send = '\x1B';
        break;
      case 'Tab':
        send = '\t';
        break;
      case 'Ctrl+C':
        send = '\x03';
        break;

      case '↑':
        send = '\x1B[A';
        break;
      case '↓':
        send = '\x1B[B';
        break;
      case '→':
        send = '\x1B[C';
        break;
      case '←':
        send = '\x1B[D';
        break;

      case 'Home':
        send = '\x1B[H';
        break;
      case 'End':
        send = '\x1B[F';
        break;
      case 'PgUp':
        send = '\x1B[5~';
        break;
      case 'PgDn':
        send = '\x1B[6~';
        break;

      default:
        send = key;
        break;
    }

    _terminalModel.sendVirtualKey(send);
  }

  TerminalStyle _getTerminalStyle() {
    return isWeb
        ? TerminalStyle(
            fontFamily: _robotoMonoFontFamily,
            fontSize: 14,
          )
        : const TerminalStyle();
  }

  @override
  bool get wantKeepAlive => true;
}
