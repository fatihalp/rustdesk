import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/common.dart';

enum SystemWindowTheme { light, dark }

class RdPlatformChannel {
  RdPlatformChannel._();

  static final RdPlatformChannel _windowUtil = RdPlatformChannel._();

  static RdPlatformChannel get instance => _windowUtil;

  final MethodChannel _hostMethodChannel =
      MethodChannel("org.supportdesk.supportdesk/host");

  Future<bool> bumpMouse({required int dx, required int dy}) async {

    bool? result = await _hostMethodChannel
      .invokeMethod("bumpMouse", {"dx": dx, "dy": dy});

    return result ?? false;
  }

  Future<void> changeSystemWindowTheme(SystemWindowTheme theme) {
    assert(isMacOS);
    if (kDebugMode) {
      print(
          "[Window ${kWindowId ?? 'Main'}] change system window theme to ${theme.name}");
    }
    return _hostMethodChannel
        .invokeMethod("setWindowTheme", {"themeName": theme.name});
  }

  Future<void> terminate() {
    assert(isMacOS);
    return _hostMethodChannel.invokeMethod("terminate");
  }
}
