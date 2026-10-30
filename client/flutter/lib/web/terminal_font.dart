import 'dart:html' as html;
import 'dart:js' as js;
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

bool _loadRequested = false;

Future<void> loadLocalTerminalFontIfNeeded() async {
  if (_loadRequested || js.context['supportdeskLocalFonts'] != true) {
    return;
  }
  _loadRequested = true;
  try {
    final req = await html.HttpRequest.request(
      'fonts/RobotoMono-Regular.ttf',
      responseType: 'arraybuffer',
    );
    final data = ByteData.view(req.response as ByteBuffer);
    final loader = FontLoader('RobotoMono_regular')
      ..addFont(Future.value(data));
    await loader.load();
  } catch (e) {
    _loadRequested = false;
    debugPrint('Failed to load bundled Roboto Mono: $e');
  }
}
