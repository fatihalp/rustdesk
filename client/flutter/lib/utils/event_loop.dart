import 'dart:async';

import 'package:flutter/foundation.dart';

typedef EventCallback<Data> = Future<dynamic> Function(Data data);

abstract class BaseEvent<EventType, Data> {
  EventType type;
  Data data;

  BaseEvent(this.type, this.data);

  @visibleForTesting
  Future<dynamic> consume() async {
    final cb = findCallback(type);
    if (cb == null) {
      return null;
    } else {
      return cb(data);
    }
  }

  EventCallback<Data>? findCallback(EventType type);
}

abstract class BaseEventLoop<EventType, Data> {
  final List<BaseEvent<EventType, Data>> _evts = [];
  Timer? _timer;

  List<BaseEvent<EventType, Data>> get evts => _evts;

  Future<void> onReady() async {
    _timer = Timer.periodic(Duration(milliseconds: 100), _handleTimer);
  }

  Future<void> onPreConsume(BaseEvent<EventType, Data> evt) async {}
  Future<void> onPostConsume(BaseEvent<EventType, Data> evt) async {}
  Future<void> onEventsClear() async {}
  Future<void> onEventsStartConsuming() async {}

  Future<void> _handleTimer(Timer timer) async {
      if (_evts.isEmpty) {
        return;
      }
      timer.cancel();
      _timer = null;
      await onEventsStartConsuming();
      while (_evts.isNotEmpty) {
        final evt = _evts.first;
        _evts.remove(evt);
        await onPreConsume(evt);
        await evt.consume();
        await onPostConsume(evt);
      }
      await onEventsClear();
      _timer = Timer.periodic(Duration(milliseconds: 100), _handleTimer);
  }

  Future<void> close() async {
    _timer?.cancel();
  }

  void pushEvent(BaseEvent<EventType, Data> evt) {
    _evts.add(evt);
  }

  void clear() {
    _evts.clear();
  }
}
