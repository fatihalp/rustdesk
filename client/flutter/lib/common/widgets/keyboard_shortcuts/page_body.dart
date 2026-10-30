
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../../../common.dart';
import '../../../consts.dart';
import '../../../models/platform_model.dart';
import '../../../models/shortcut_model.dart';
import 'display.dart';
import 'recording_dialog.dart';
import 'shortcut_actions.dart';
import 'shortcut_utils.dart';

class KeyboardShortcutsPageBody extends StatefulWidget {
  final bool compact;
  final String? editButtonHint;
  final Widget? headerBanner;

  final bool showMasterToggles;

  const KeyboardShortcutsPageBody({
    Key? key,
    this.compact = true,
    this.editButtonHint,
    this.headerBanner,
    this.showMasterToggles = true,
  }) : super(key: key);

  @override
  State<KeyboardShortcutsPageBody> createState() =>
      KeyboardShortcutsPageBodyState();
}

class KeyboardShortcutsPageBodyState extends State<KeyboardShortcutsPageBody> {

  Map<String, dynamic> _readJson() {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    if (raw.isEmpty) return {'enabled': false, 'bindings': <dynamic>[]};
    try {
      final parsed = jsonDecode(raw) as Map<String, dynamic>;
      parsed['bindings'] ??= <dynamic>[];
      parsed['enabled'] ??= false;
      return parsed;
    } catch (_) {
      return {'enabled': false, 'bindings': <dynamic>[]};
    }
  }

  Future<void> _writeJson(Map<String, dynamic> json) async {
    await bind.mainSetLocalOption(
        key: kShortcutLocalConfigKey, value: jsonEncode(json));
    bind.mainReloadKeyboardShortcuts();
    if (mounted) setState(() {});
  }

  Future<void> _setBinding(
    String actionId, {
    Map<String, dynamic>? binding,
    String? clearActionId,
  }) async {
    final json = _readJson();
    final list = shortcutBindingMapsFrom(json['bindings']);
    list.removeWhere((b) {
      final a = b['action'];
      return a == actionId || (clearActionId != null && a == clearActionId);
    });
    if (binding != null) {
      list.add(binding);
    }
    json['bindings'] = list;
    await _writeJson(json);
  }

  Future<void> _setEnabled(bool v) async {
    await ShortcutModel.setEnabled(v);
    if (mounted) setState(() {});
  }

  Future<void> _setPassThrough(bool v) async {
    await ShortcutModel.setPassThrough(v);
    if (mounted) setState(() {});
  }

  Future<void> _resetToDefaults() async {
    final json = _readJson();
    json['bindings'] = filterDefaultBindingsForPlatform(
      jsonDecode(bind.mainGetDefaultKeyboardShortcuts()) as List,
      ShortcutModel.currentPlatformCapabilities(),
    );
    await _writeJson(json);
  }

  String _labelFor(String actionId) {
    for (final entry in allActionEntries(kKeyboardShortcutActionGroups)) {
      if (entry.id == actionId) return translate(entry.labelKey);
    }
    return actionId;
  }

  List<KeyboardShortcutActionGroup> _groupsForCurrentPlatform() {
    return filterKeyboardShortcutActionGroupsForPlatform(
      ShortcutModel.currentPlatformCapabilities(),
    );
  }


  Future<void> _onEdit(KeyboardShortcutActionEntry entry) async {
    final json = _readJson();
    final bindings = shortcutBindingMapsFrom(json['bindings']);
    final result = await showRecordingDialog(
      context: context,
      actionId: entry.id,
      actionLabel: translate(entry.labelKey),
      existingBindings: bindings,
      actionLabelLookup: _labelFor,
    );
    if (result == null) return;
    await _setBinding(
      entry.id,
      binding: result.binding,
      clearActionId: result.clearActionId,
    );
  }

  Future<void> _onClear(KeyboardShortcutActionEntry entry) async {
    await _setBinding(entry.id, binding: null);
  }

  Future<void> resetToDefaultsWithConfirm() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(translate('Reset to defaults')),
        content: Text(translate('shortcut-reset-confirm-tip')),
        actions: [
          dialogButton('Cancel',
              onPressed: () => Navigator.of(ctx).pop(false), isOutline: true),
          dialogButton('OK', onPressed: () => Navigator.of(ctx).pop(true)),
        ],
      ),
    );
    if (confirmed == true) {
      await _resetToDefaults();
    }
  }


  @override
  Widget build(BuildContext context) {
    final enabled = ShortcutModel.isEnabled();
    final theme = Theme.of(context);

    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        if (widget.headerBanner != null) ...[
          widget.headerBanner!,
          const SizedBox(height: 12),
        ],
        if (widget.showMasterToggles) ...[
          _toggleRow(
            enabled,
            'Enable keyboard shortcuts in remote session',
            (v) => _setEnabled(v),
          ),
          if (enabled)
            _toggleRow(
              ShortcutModel.isPassThrough(),
              'Pass-through to remote',
              (v) => _setPassThrough(v),
            ),
        ],
        const SizedBox(height: 8),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: Text(
            translate('shortcut-page-description'),
            style: TextStyle(color: theme.hintColor),
          ),
        ),
        const SizedBox(height: 16),
        if (enabled)
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              for (final group in _groupsForCurrentPlatform())
                _buildGroup(context, group),
            ],
          ),
      ],
    );
  }

  Widget _toggleRow(
      bool value, String labelKey, Future<void> Function(bool) onChanged,
      {String? tooltipKey}) {
    return Row(
      children: [
        Checkbox(
          value: value,
          onChanged: (v) async {
            if (v == null) return;
            await onChanged(v);
          },
        ),
        const SizedBox(width: 4),
        Expanded(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: () => onChanged(!value),
            child: Text(translate(labelKey)),
          ),
        ),
        if (tooltipKey != null) InfoTooltipIcon(tipKey: tooltipKey),
      ],
    );
  }

  static const double _kIndentStep = 16.0;

  Widget _buildGroup(BuildContext context, KeyboardShortcutActionGroup group) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const SizedBox(height: 12),
        _buildHeading(context, group.titleKey, isSub: false),
        const SizedBox(height: 4),
        for (final child in group.children)
          switch (child) {
            KeyboardShortcutActionEntry() => Padding(
                padding: const EdgeInsets.only(left: _kIndentStep),
                child: _buildEntryRow(context, child),
              ),
            KeyboardShortcutActionSubgroup() =>
              _buildSubgroup(context, child),
          },
      ],
    );
  }

  Widget _buildSubgroup(
      BuildContext context, KeyboardShortcutActionSubgroup subgroup) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const SizedBox(height: 8),
        _buildHeading(context, subgroup.titleKey, isSub: true),
        const SizedBox(height: 4),
        for (final entry in subgroup.entries)
          Padding(
            padding: const EdgeInsets.only(left: _kIndentStep * 2),
            child: _buildEntryRow(context, entry),
          ),
      ],
    );
  }

  Widget _buildHeading(BuildContext context, String titleKey,
      {required bool isSub}) {
    final indent = isSub ? _kIndentStep : 0.0;
    return Padding(
      padding: EdgeInsets.only(left: 8 + indent, right: 8),
      child: Row(
        children: [
          Text(
            translate(titleKey),
            style: TextStyle(
              fontWeight: isSub ? FontWeight.w500 : FontWeight.w600,
              color: isSub
                  ? Theme.of(context).hintColor
                  : Theme.of(context).colorScheme.primary,
            ),
          ),
          const SizedBox(width: 8),
          Expanded(child: Divider(thickness: isSub ? 0.5 : 1)),
        ],
      ),
    );
  }

  Widget _buildEntryRow(
      BuildContext context, KeyboardShortcutActionEntry entry) {
    return widget.compact
        ? _buildCompactRow(context, entry)
        : _buildTouchRow(context, entry);
  }

  Widget _buildCompactRow(
      BuildContext context, KeyboardShortcutActionEntry entry) {
    final shortcut = ShortcutDisplay.formatFor(entry.id, requireEnabled: false);
    final hasBinding = shortcut != null;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
      child: Row(
        children: [
          Expanded(
            flex: 5,
            child: Text(translate(entry.labelKey)),
          ),
          Expanded(
            flex: 4,
            child: Text(
              shortcut ?? '—',
              style: TextStyle(
                fontFamily: defaultTargetPlatform == TargetPlatform.windows
                    ? 'Consolas'
                    : 'monospace',
                color: hasBinding ? null : Theme.of(context).hintColor,
              ),
            ),
          ),
          IconButton(
            tooltip: widget.editButtonHint ?? translate('Edit'),
            onPressed: () => _onEdit(entry),
            icon: const Icon(Icons.edit_outlined, size: 18),
          ),
          SizedBox(
            width: 40,
            child: hasBinding
                ? IconButton(
                    tooltip: translate('Clear'),
                    onPressed: () => _onClear(entry),
                    icon: const Icon(Icons.close, size: 18),
                  )
                : const SizedBox.shrink(),
          ),
        ],
      ),
    );
  }

  Widget _buildTouchRow(
      BuildContext context, KeyboardShortcutActionEntry entry) {
    final shortcut = ShortcutDisplay.formatFor(entry.id, requireEnabled: false);
    final hasBinding = shortcut != null;
    return ListTile(
      dense: false,
      contentPadding: const EdgeInsets.symmetric(horizontal: 8),
      title: Text(translate(entry.labelKey)),
      subtitle: Text(
        shortcut ?? '—',
        style: TextStyle(
          fontFamily: defaultTargetPlatform == TargetPlatform.windows
              ? 'Consolas'
              : 'monospace',
          color: hasBinding ? null : Theme.of(context).hintColor,
        ),
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            tooltip: widget.editButtonHint ?? translate('Edit'),
            onPressed: () => _onEdit(entry),
            icon: const Icon(Icons.edit_outlined),
          ),
          if (hasBinding)
            IconButton(
              tooltip: translate('Clear'),
              onPressed: () => _onClear(entry),
              icon: const Icon(Icons.close),
            )
          else
            const SizedBox(width: 48),
        ],
      ),
    );
  }
}

class InfoTooltipIcon extends StatelessWidget {
  final String tipKey;
  const InfoTooltipIcon({Key? key, required this.tipKey}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: translate(tipKey),
      triggerMode: TooltipTriggerMode.tap,
      preferBelow: false,
      waitDuration: const Duration(milliseconds: 250),
      showDuration: const Duration(seconds: 6),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 6),
        child: Icon(
          Icons.help_outline,
          size: 16,
          color: Theme.of(context).hintColor,
        ),
      ),
    );
  }
}
