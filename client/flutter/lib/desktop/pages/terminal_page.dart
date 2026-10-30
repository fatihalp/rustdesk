import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/terminal_model.dart';
import 'package:flutter_hbb/models/terminal_mouse_handler.dart';
import 'terminal_connection_manager.dart';

class TerminalPage extends StatefulWidget {
  TerminalPage({
    Key? key,
    required this.id,
    required this.password,
    required this.tabController,
    required this.isSharedPassword,
    required this.terminalId,
    required this.tabKey,
    this.forceRelay,
    this.connToken,
    this.onClipboardWriteBlocked,
    this.onClipboardWriteSucceeded,
  }) : super(key: key);
  final String id;
  final String? password;
  final DesktopTabController tabController;
  final bool? forceRelay;
  final bool? isSharedPassword;
  final String? connToken;
  final ValueChanged<String>? onClipboardWriteBlocked;
  final ValueChanged<String>? onClipboardWriteSucceeded;
  final int terminalId;

  final String tabKey;
  final SimpleWrapper<State<TerminalPage>?> _lastState = SimpleWrapper(null);

  FFI get ffi => (_lastState.value! as _TerminalPageState)._ffi;

  @override
  State<TerminalPage> createState() {
    final state = _TerminalPageState();
    _lastState.value = state;
    return state;
  }
}

class _TerminalPageState extends State<TerminalPage>
    with AutomaticKeepAliveClientMixin {
  static const EdgeInsets _defaultTerminalPadding =
      EdgeInsets.symmetric(horizontal: 5.0, vertical: 2.0);

  late FFI _ffi;
  late TerminalModel _terminalModel;
  double? _cellHeight;
  final FocusNode _terminalFocusNode = FocusNode(canRequestFocus: false);
  StreamSubscription<DesktopTabState>? _tabStateSubscription;

  @override
  void initState() {
    super.initState();

    _tabStateSubscription = widget.tabController.state.listen(_onTabStateChanged);

    _ffi = TerminalConnectionManager.getConnection(
      peerId: widget.id,
      password: widget.password,
      isSharedPassword: widget.isSharedPassword,
      forceRelay: widget.forceRelay,
      connToken: widget.connToken,
    );

    _terminalModel = TerminalModel(_ffi, widget.terminalId);
    _terminalModel.onClipboardWriteBlocked = widget.onClipboardWriteBlocked;
    _terminalModel.onClipboardWriteSucceeded = widget.onClipboardWriteSucceeded;
    debugPrint(
        '[TerminalPage] Terminal model created for terminal ${widget.terminalId}');

    _terminalModel.onResizeExternal = (w, h, pw, ph) {
      _cellHeight = ph * 1.0;

      if (!_terminalFocusNode.canRequestFocus && w > 0 && h > 0) {
        _terminalFocusNode.canRequestFocus = true;
        _requestFocusIfSelected();
      }

      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          setState(() {});
        }
      });
    };

    _ffi.registerTerminalModel(widget.terminalId, _terminalModel);

    _terminalModel.onClosed = () {
      if (mounted) {
        widget.tabController.closeBy(widget.tabKey);
      }
    };

    WidgetsBinding.instance.addPostFrameCallback((_) {
      widget.tabController.onSelected?.call(widget.id);

      final isExistingConnection =
          TerminalConnectionManager.hasConnection(widget.id) &&
              TerminalConnectionManager.getTerminalCount(widget.id) > 1;

      if (!isExistingConnection) {
        _ffi.dialogManager
            .showLoading(translate('Connecting...'), onCancel: closeConnection);
      } else {
        _terminalModel.openTerminal();
      }
    });
  }

  @override
  void dispose() {
    _tabStateSubscription?.cancel();
    _ffi.unregisterTerminalModel(widget.terminalId);
    _terminalModel.dispose();
    _terminalFocusNode.dispose();
    TerminalConnectionManager.releaseConnection(widget.id);
    super.dispose();
  }

  void _onTabStateChanged(DesktopTabState state) {
    if (state.selected >= 0 && state.selected < state.tabs.length) {
      final selectedTab = state.tabs[state.selected];
      if (selectedTab.key == widget.tabKey && mounted) {
        _requestFocusIfSelected();
      }
    }
  }

  void _requestFocusIfSelected() {
    if (!mounted || !_terminalFocusNode.canRequestFocus) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_terminalFocusNode.canRequestFocus || _terminalFocusNode.hasFocus) return;
      final state = widget.tabController.state.value;
      if (state.selected >= 0 && state.selected < state.tabs.length) {
        if (state.tabs[state.selected].key == widget.tabKey) {
          _terminalFocusNode.requestFocus();
        }
      }
    });
  }

  EdgeInsets _calculatePadding(double heightPx) {
    final cellHeight = _cellHeight;
    if (!heightPx.isFinite ||
        heightPx <= 0 ||
        cellHeight == null ||
        !cellHeight.isFinite ||
        cellHeight <= 0) {
      return _defaultTerminalPadding;
    }
    final rows = (heightPx / cellHeight).floor();
    if (rows <= 0) {
      return _defaultTerminalPadding;
    }
    final extraSpace = heightPx - rows * cellHeight;
    if (!extraSpace.isFinite || extraSpace < 0) {
      return _defaultTerminalPadding;
    }
    final topBottom = extraSpace / 2.0;
    return EdgeInsets.symmetric(
      horizontal: _defaultTerminalPadding.horizontal / 2,
      vertical: topBottom,
    );
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Scaffold(
      backgroundColor: Theme.of(context).scaffoldBackgroundColor,
      body: LayoutBuilder(
        builder: (context, constraints) {
          final heightPx = constraints.maxHeight;
          return TerminalMouseInteraction(
            _terminalModel.terminal,
            controller: _terminalModel.terminalController,
            focusNode: _terminalFocusNode,
            backgroundOpacity: 0.7,
            padding: _calculatePadding(heightPx),
            onSecondaryTapDown: (details, offset) async {
              final selection = _terminalModel.terminalController.selection;
              if (selection != null) {
                final text = _terminalModel.terminal.buffer.getText(selection);
                _terminalModel.terminalController.clearSelection();
                await Clipboard.setData(ClipboardData(text: text));
              } else {
                final data = await Clipboard.getData('text/plain');
                final text = data?.text;
                if (text != null) {
                  _terminalModel.terminal.paste(text);
                }
              }
            },
          );
        },
      ),
    );
  }

  @override
  bool get wantKeepAlive => true;
}
