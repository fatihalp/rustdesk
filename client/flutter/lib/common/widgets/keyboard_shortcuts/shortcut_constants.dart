const kShortcutActionSendCtrlAltDel = 'send_ctrl_alt_del';
const kShortcutActionToggleFullscreen = 'toggle_fullscreen';
const kShortcutActionSwitchDisplayNext = 'switch_display_next';
const kShortcutActionSwitchDisplayPrev = 'switch_display_prev';
const kShortcutActionSwitchDisplayAll = 'switch_display_all';
const kShortcutActionScreenshot = 'screenshot';
const kShortcutActionInsertLock = 'insert_lock';
const kShortcutActionRefresh = 'refresh';
const kShortcutActionToggleBlockInput = 'toggle_block_input';
const kShortcutActionToggleRecording = 'toggle_recording';
const kShortcutActionSwitchSides = 'switch_sides';
const kShortcutActionCloseTab = 'close_tab';
const kShortcutActionToggleToolbar = 'toggle_toolbar';
const kShortcutActionRestartRemote = 'restart_remote';
const kShortcutActionResetCanvas = 'reset_canvas';
const kShortcutActionSwitchTabNext = 'switch_tab_next';
const kShortcutActionSwitchTabPrev = 'switch_tab_prev';

const kShortcutActionsRunOnKeyUp = {
  kShortcutActionCloseTab,
  kShortcutActionSwitchTabNext,
  kShortcutActionSwitchTabPrev,
  kShortcutActionRestartRemote,
  kShortcutActionSwitchSides,
  kShortcutActionToggleChat,
};
const kShortcutActionToggleMute = 'toggle_mute';
const kShortcutActionPinToolbar = 'pin_toolbar';
const kShortcutActionViewModeOriginal = 'view_mode_original';
const kShortcutActionViewModeAdaptive = 'view_mode_adaptive';
const kShortcutActionToggleChat = 'toggle_chat';
const kShortcutActionToggleQualityMonitor = 'toggle_quality_monitor';
const kShortcutActionToggleShowRemoteCursor = 'toggle_show_remote_cursor';
const kShortcutActionToggleShowMyCursor = 'toggle_show_my_cursor';
const kShortcutActionToggleDisableClipboard = 'toggle_disable_clipboard';
const kShortcutActionPrivacyMode1 = 'privacy_mode_1';
const kShortcutActionPrivacyMode2 = 'privacy_mode_2';
const kShortcutActionKeyboardModeMap = 'keyboard_mode_map';
const kShortcutActionKeyboardModeTranslate = 'keyboard_mode_translate';
const kShortcutActionKeyboardModeLegacy = 'keyboard_mode_legacy';
const kShortcutActionCodecAuto = 'codec_auto';
const kShortcutActionCodecVp8 = 'codec_vp8';
const kShortcutActionCodecVp9 = 'codec_vp9';
const kShortcutActionCodecAv1 = 'codec_av1';
const kShortcutActionCodecH264 = 'codec_h264';
const kShortcutActionCodecH265 = 'codec_h265';
const kShortcutActionPlugOutAllVirtualDisplays =
    'plug_out_all_virtual_displays';
const kShortcutActionToggleRelativeMouseMode = 'toggle_relative_mouse_mode';
const kShortcutActionToggleFollowRemoteCursor = 'toggle_follow_remote_cursor';
const kShortcutActionToggleFollowRemoteWindow = 'toggle_follow_remote_window';
const kShortcutActionToggleZoomCursor = 'toggle_zoom_cursor';
const kShortcutActionToggleReverseMouseWheel = 'toggle_reverse_mouse_wheel';
const kShortcutActionToggleSwapLeftRightMouse = 'toggle_swap_left_right_mouse';
const kShortcutActionToggleLockAfterSessionEnd = 'toggle_lock_after_session_end';
const kShortcutActionToggleTrueColor = 'toggle_true_color';
const kShortcutActionToggleSwapCtrlCmd = 'toggle_swap_ctrl_cmd';
const kShortcutActionToggleEnableFileCopyPaste = 'toggle_enable_file_copy_paste';
const kShortcutActionViewModeCustom = 'view_mode_custom';
const kShortcutActionImageQualityBest = 'image_quality_best';
const kShortcutActionImageQualityBalanced = 'image_quality_balanced';
const kShortcutActionImageQualityLow = 'image_quality_low';
const kShortcutActionSendClipboardKeystrokes = 'send_clipboard_keystrokes';
const kShortcutActionToggleVoiceCall = 'toggle_voice_call';
const kShortcutActionToggleViewOnly = 'toggle_view_only';

const kShortcutLocalConfigKey = 'keyboard-shortcuts';
const kShortcutEventName = 'shortcut_triggered';

final List<Map<String, Object>> kDefaultShortcutBindings = [
  for (final entry in <List<Object>>[
    [kShortcutActionSendCtrlAltDel,         'delete'],
    [kShortcutActionToggleFullscreen,       'enter'],
    [kShortcutActionSwitchDisplayNext,      'arrow_right'],
    [kShortcutActionSwitchDisplayPrev,      'arrow_left'],
    [kShortcutActionScreenshot,             'p'],
    [kShortcutActionToggleShowRemoteCursor, 'm'],
    [kShortcutActionToggleMute,             's'],
    [kShortcutActionToggleBlockInput,       'i'],
    [kShortcutActionToggleChat,             'c'],
  ])
    {
      'action': entry[0],
      'mods': const ['primary', 'alt', 'shift'],
      'key': entry[1],
    },
];
