class RelativeMouseDelta {
  final int x;
  final int y;

  const RelativeMouseDelta(this.x, this.y);
}

class RelativeMouseAccumulator {
  double _fracX = 0.0;
  double _fracY = 0.0;

  RelativeMouseDelta? add(
    double dx,
    double dy, {
    required int maxDelta,
  }) {
    assert(maxDelta >= 0, 'maxDelta must be non-negative');

    _fracX += dx;
    _fracY += dy;

    int intX = _fracX.truncate();
    int intY = _fracY.truncate();

    if (intX == 0 && intY == 0) {
      return null;
    }

    if (maxDelta > 0) {
      intX = intX.clamp(-maxDelta, maxDelta);
      intY = intY.clamp(-maxDelta, maxDelta);
    }

    _fracX -= intX;
    _fracY -= intY;

    return RelativeMouseDelta(intX, intY);
  }

  void reset() {
    _fracX = 0.0;
    _fracY = 0.0;
  }
}
