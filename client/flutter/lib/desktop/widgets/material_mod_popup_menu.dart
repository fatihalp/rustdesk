
import 'package:flutter/foundation.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/widgets/menu_button.dart';


const Duration _kMenuDuration = Duration(milliseconds: 0);
const double _kMenuCloseIntervalEnd = 2.0 / 3.0;
const double _kMenuHorizontalPadding = 16.0;
const double _kMenuDividerHeight = 16.0;
const double _kMenuMinWidth = 2.0 * _kMenuWidthStep;
const double _kMenuMaxWidth = double.infinity;
const double _kMenuVerticalPadding = 8.0;
const double _kMenuWidthStep = 0.0;
const double _kMenuScreenPadding = 0.0;
const double _kDefaultIconSize = 24.0;

enum PopupMenuPosition {
  over,

  under,

  overSide,

  underSide,
}

abstract class PopupMenuEntry<T> extends StatefulWidget {
  const PopupMenuEntry({Key? key}) : super(key: key);

  double get height;

  bool represents(T? value);
}

class PopupMenuDivider extends PopupMenuEntry<Never> {
  const PopupMenuDivider({Key? key, this.height = _kMenuDividerHeight})
      : super(key: key);

  @override
  final double height;

  @override
  bool represents(void value) => false;

  @override
  State<PopupMenuDivider> createState() => _PopupMenuDividerState();
}

class _PopupMenuDividerState extends State<PopupMenuDivider> {
  @override
  Widget build(BuildContext context) => Divider(height: widget.height);
}

class _MenuItem extends SingleChildRenderObjectWidget {
  const _MenuItem({
    Key? key,
    required this.onLayout,
    required Widget? child,
  }) : super(key: key, child: child);

  final ValueChanged<Size> onLayout;

  @override
  RenderObject createRenderObject(BuildContext context) {
    return _RenderMenuItem(onLayout);
  }

  @override
  void updateRenderObject(
      BuildContext context, covariant _RenderMenuItem renderObject) {
    renderObject.onLayout = onLayout;
  }
}

class _RenderMenuItem extends RenderShiftedBox {
  _RenderMenuItem(this.onLayout, [RenderBox? child]) : super(child);

  ValueChanged<Size> onLayout;

  @override
  Size computeDryLayout(BoxConstraints constraints) {
    if (child == null) {
      return Size.zero;
    }
    return child!.getDryLayout(constraints);
  }

  @override
  void performLayout() {
    if (child == null) {
      size = Size.zero;
    } else {
      child!.layout(constraints, parentUsesSize: true);
      size = constraints.constrain(child!.size);
      final BoxParentData childParentData = child!.parentData! as BoxParentData;
      childParentData.offset = Offset.zero;
    }
    onLayout(size);
  }
}

class PopupMenuItem<T> extends PopupMenuEntry<T> {
  const PopupMenuItem({
    Key? key,
    this.value,
    this.onTap,
    this.enabled = true,
    this.height = kMinInteractiveDimension,
    this.padding,
    this.textStyle,
    this.mouseCursor,
    required this.child,
  }) : super(key: key);

  final T? value;

  final VoidCallback? onTap;

  final bool enabled;

  @override
  final double height;

  final EdgeInsets? padding;

  final TextStyle? textStyle;

  final MouseCursor? mouseCursor;

  final Widget? child;

  @override
  bool represents(T? value) => value == this.value;

  @override
  PopupMenuItemState<T, PopupMenuItem<T>> createState() =>
      PopupMenuItemState<T, PopupMenuItem<T>>();
}

class PopupMenuItemState<T, W extends PopupMenuItem<T>> extends State<W> {
  @protected
  Widget? buildChild() => widget.child;

  @protected
  void handleTap() {
    widget.onTap?.call();
    if (Navigator.canPop(context)) {
      Navigator.pop<T>(context, widget.value);
    }
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final PopupMenuThemeData popupMenuTheme = PopupMenuTheme.of(context);
    TextStyle style = widget.textStyle ??
        popupMenuTheme.textStyle ??
        theme.textTheme.titleMedium!;

    if (!widget.enabled) style = style.copyWith(color: theme.disabledColor);

    Widget item = AnimatedDefaultTextStyle(
      style: style,
      duration: kThemeChangeDuration,
      child: Container(
        alignment: AlignmentDirectional.centerStart,
        constraints: BoxConstraints(minHeight: widget.height),
        padding: widget.padding ??
            const EdgeInsets.symmetric(horizontal: _kMenuHorizontalPadding),
        child: buildChild(),
      ),
    );

    if (!widget.enabled) {
      final bool isDark = theme.brightness == Brightness.dark;
      item = IconTheme.merge(
        data: IconThemeData(opacity: isDark ? 0.5 : 0.38),
        child: item,
      );
    }

    return MergeSemantics(
      child: Semantics(
        enabled: widget.enabled,
        button: true,
        child: TextButton(
          onPressed: widget.enabled ? handleTap : null,
          child: item,
        ),
      ),
    );
  }
}

class CheckedPopupMenuItem<T> extends PopupMenuItem<T> {
  const CheckedPopupMenuItem({
    Key? key,
    T? value,
    this.checked = false,
    bool enabled = true,
    EdgeInsets? padding,
    double height = kMinInteractiveDimension,
    Widget? child,
  }) : super(
          key: key,
          value: value,
          enabled: enabled,
          padding: padding,
          height: height,
          child: child,
        );

  final bool checked;

  @override
  Widget? get child => super.child;

  @override
  PopupMenuItemState<T, CheckedPopupMenuItem<T>> createState() =>
      _CheckedPopupMenuItemState<T>();
}

class _CheckedPopupMenuItemState<T>
    extends PopupMenuItemState<T, CheckedPopupMenuItem<T>>
    with SingleTickerProviderStateMixin {
  static const Duration _fadeDuration = Duration(milliseconds: 150);
  late AnimationController _controller;
  Animation<double> get _opacity => _controller.view;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(duration: _fadeDuration, vsync: this)
      ..value = widget.checked ? 1.0 : 0.0
      ..addListener(() => setState(() { }));
  }

  @override
  void handleTap() {
    if (widget.checked) {
      _controller.reverse();
    } else {
      _controller.forward();
    }
    super.handleTap();
  }

  @override
  Widget buildChild() {
    return ListTile(
      enabled: widget.enabled,
      leading: FadeTransition(
        opacity: _opacity,
        child: Icon(_controller.isDismissed ? null : Icons.done),
      ),
      title: widget.child,
    );
  }
}

class _PopupMenu<T> extends StatelessWidget {
  const _PopupMenu({
    Key? key,
    required this.route,
    required this.semanticLabel,
    this.constraints,
  }) : super(key: key);

  final _PopupMenuRoute<T> route;
  final String? semanticLabel;
  final BoxConstraints? constraints;

  @override
  Widget build(BuildContext context) {
    final double unit = 1.0 /
        (route.items.length +
            1.5);
    final List<Widget> children = <Widget>[];
    final PopupMenuThemeData popupMenuTheme = PopupMenuTheme.of(context);

    for (int i = 0; i < route.items.length; i += 1) {
      final double start = (i + 1) * unit;
      final double end = (start + 1.5 * unit).clamp(0.0, 1.0);
      final CurvedAnimation opacity = CurvedAnimation(
        parent: route.animation!,
        curve: Interval(start, end),
      );
      Widget item = route.items[i];
      if (route.initialValue != null &&
          route.items[i].represents(route.initialValue)) {
        item = Container(
          color: Theme.of(context).highlightColor,
          child: item,
        );
      }
      children.add(
        _MenuItem(
          onLayout: (Size size) {
            route.itemSizes[i] = size;
          },
          child: FadeTransition(
            opacity: opacity,
            child: item,
          ),
        ),
      );
    }

    final CurveTween opacity =
        CurveTween(curve: const Interval(0.0, 1.0 / 3.0));
    final CurveTween width = CurveTween(curve: Interval(0.0, unit));
    final CurveTween height =
        CurveTween(curve: Interval(0.0, unit * route.items.length));

    final Widget child = ConstrainedBox(
      constraints: constraints ??
          const BoxConstraints(
            minWidth: _kMenuMinWidth,
            maxWidth: _kMenuMaxWidth,
          ),
      child: IntrinsicWidth(
        stepWidth: _kMenuWidthStep,
        child: Semantics(
          scopesRoute: true,
          namesRoute: true,
          explicitChildNodes: true,
          label: semanticLabel,
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(
              vertical: _kMenuVerticalPadding,
            ),
            controller: ScrollController(),
            child: ListBody(children: children),
          ),
        ),
      ),
    );

    return AnimatedBuilder(
      animation: route.animation!,
      builder: (BuildContext context, Widget? child) {
        return FadeTransition(
          opacity: opacity.animate(route.animation!),
          child: Material(
            shape: route.shape ?? popupMenuTheme.shape,
            color: route.color ?? popupMenuTheme.color,
            type: MaterialType.card,
            elevation: route.elevation ?? popupMenuTheme.elevation ?? 8.0,
            child: Align(
              alignment: AlignmentDirectional.topEnd,
              widthFactor: width.evaluate(route.animation!),
              heightFactor: height.evaluate(route.animation!),
              child: child,
            ),
          ),
        );
      },
      child: child,
    );
  }
}

class _PopupMenuRouteLayout extends SingleChildLayoutDelegate {
  _PopupMenuRouteLayout(
    this.position,
    this.itemSizes,
    this.selectedItemIndex,
    this.textDirection,
    this.padding,
    this.avoidBounds,
  );

  final RelativeRect position;

  List<Size?> itemSizes;

  final int? selectedItemIndex;

  final TextDirection textDirection;

  EdgeInsets padding;

  final Set<Rect> avoidBounds;


  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) {
    return BoxConstraints.loose(constraints.biggest).deflate(
      const EdgeInsets.all(_kMenuScreenPadding) + padding,
    );
  }

  @override
  Offset getPositionForChild(Size size, Size childSize) {

    final double buttonHeight = size.height - position.top - position.bottom;
    double y = position.top;
    if (selectedItemIndex != null) {
      double selectedItemOffset = _kMenuVerticalPadding;
      for (int index = 0; index < selectedItemIndex!; index += 1) {
        selectedItemOffset += itemSizes[index]!.height;
      }
      selectedItemOffset += itemSizes[selectedItemIndex!]!.height / 2;
      y = y + buttonHeight / 2.0 - selectedItemOffset;
    }

    double x;
    switch (textDirection) {
      case TextDirection.rtl:
        x = size.width - position.right - childSize.width;
        break;
      case TextDirection.ltr:
        x = position.left;
        break;
    }
    final Offset wantedPosition = Offset(x, y);
    final Offset originCenter = position.toRect(Offset.zero & size).center;
    final Iterable<Rect> subScreens =
        DisplayFeatureSubScreen.subScreensInBounds(
            Offset.zero & size, avoidBounds);
    final Rect subScreen = _closestScreen(subScreens, originCenter);
    return _fitInsideScreen(subScreen, childSize, wantedPosition);
  }

  Rect _closestScreen(Iterable<Rect> screens, Offset point) {
    Rect closest = screens.first;
    for (final Rect screen in screens) {
      if ((screen.center - point).distance <
          (closest.center - point).distance) {
        closest = screen;
      }
    }
    return closest;
  }

  Offset _fitInsideScreen(Rect screen, Size childSize, Offset wantedPosition) {
    double x = wantedPosition.dx;
    double y = wantedPosition.dy;
    if (x < screen.left + _kMenuScreenPadding + padding.left) {
      x = screen.left + _kMenuScreenPadding + padding.left;
    } else if (x + childSize.width >
        screen.right - _kMenuScreenPadding - padding.right) {
      x = screen.right - childSize.width - _kMenuScreenPadding - padding.right;
    }
    if (y < screen.top + _kMenuScreenPadding + padding.top) {
      y = _kMenuScreenPadding + padding.top;
    } else if (y + childSize.height >
        screen.bottom - _kMenuScreenPadding - padding.bottom) {
      y = screen.bottom -
          childSize.height -
          _kMenuScreenPadding -
          padding.bottom;
    }

    return Offset(x, y);
  }

  @override
  bool shouldRelayout(_PopupMenuRouteLayout oldDelegate) {
    assert(itemSizes.length == oldDelegate.itemSizes.length);

    return position != oldDelegate.position ||
        selectedItemIndex != oldDelegate.selectedItemIndex ||
        textDirection != oldDelegate.textDirection ||
        !listEquals(itemSizes, oldDelegate.itemSizes) ||
        padding != oldDelegate.padding ||
        !setEquals(avoidBounds, oldDelegate.avoidBounds);
  }
}

class _PopupMenuRoute<T> extends PopupRoute<T> {
  _PopupMenuRoute({
    required this.position,
    required this.items,
    this.menuWrapper,
    this.initialValue,
    this.elevation,
    required this.barrierLabel,
    this.semanticLabel,
    this.shape,
    this.color,
    required this.capturedThemes,
    this.constraints,
  }) : itemSizes = List<Size?>.filled(items.length, null);

  final RelativeRect position;
  final List<PopupMenuEntry<T>> items;
  final MenuWrapper? menuWrapper;
  final List<Size?> itemSizes;
  final T? initialValue;
  final double? elevation;
  final String? semanticLabel;
  final ShapeBorder? shape;
  final Color? color;
  final CapturedThemes capturedThemes;
  final BoxConstraints? constraints;

  @override
  Animation<double> createAnimation() {
    return CurvedAnimation(
      parent: super.createAnimation(),
      curve: Curves.linear,
      reverseCurve: const Interval(0.0, _kMenuCloseIntervalEnd),
    );
  }

  @override
  Duration get transitionDuration => _kMenuDuration;

  @override
  bool get barrierDismissible => true;

  @override
  Color? get barrierColor => null;

  @override
  final String barrierLabel;

  @override
  Widget buildPage(BuildContext context, Animation<double> animation,
      Animation<double> secondaryAnimation) {
    int? selectedItemIndex;
    if (initialValue != null) {
      for (int index = 0;
          selectedItemIndex == null && index < items.length;
          index += 1) {
        if (items[index].represents(initialValue)) selectedItemIndex = index;
      }
    }

    Widget menu = _PopupMenu<T>(
      route: this,
      semanticLabel: semanticLabel,
      constraints: constraints,
    );
    if (this.menuWrapper != null) {
      menu = this.menuWrapper!(menu);
    }
    final MediaQueryData mediaQuery = MediaQuery.of(context);
    return MediaQuery.removePadding(
      context: context,
      removeTop: true,
      removeBottom: true,
      removeLeft: true,
      removeRight: true,
      child: Builder(
        builder: (BuildContext context) {
          return CustomSingleChildLayout(
            delegate: _PopupMenuRouteLayout(
              position,
              itemSizes,
              selectedItemIndex,
              Directionality.of(context),
              mediaQuery.padding,
              _avoidBounds(mediaQuery),
            ),
            child: capturedThemes.wrap(menu),
          );
        },
      ),
    );
  }

  Set<Rect> _avoidBounds(MediaQueryData mediaQuery) {
    return DisplayFeatureSubScreen.avoidBounds(mediaQuery).toSet();
  }
}

class PopupMenu<T> extends StatelessWidget {
  PopupMenu({
    Key? key,
    required this.items,
    this.initialValue,
    this.semanticLabel,
    this.constraints,
  })  : itemSizes = List<Size?>.filled(items.length, null),
        super(key: key);

  final List<PopupMenuEntry<T>> items;
  final List<Size?> itemSizes;
  final T? initialValue;
  final String? semanticLabel;
  final BoxConstraints? constraints;

  Widget _buildMenu(BuildContext context) {
    final List<Widget> children = <Widget>[];
    for (int i = 0; i < items.length; i += 1) {
      Widget item = items[i];
      if (initialValue != null && items[i].represents(initialValue)) {
        item = Container(
          color: Theme.of(context).highlightColor,
          child: item,
        );
      }
      children.add(
        _MenuItem(
          onLayout: (Size size) {
            itemSizes[i] = size;
          },
          child: item,
        ),
      );
    }

    final child = ConstrainedBox(
      constraints: constraints ??
          const BoxConstraints(
            minWidth: _kMenuMinWidth,
            maxWidth: _kMenuMaxWidth,
          ),
      child: IntrinsicWidth(
        stepWidth: _kMenuWidthStep,
        child: Semantics(
          scopesRoute: true,
          namesRoute: true,
          explicitChildNodes: true,
          label: semanticLabel,
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(
              vertical: _kMenuVerticalPadding,
            ),
            controller: ScrollController(),
            child: ListBody(children: children),
          ),
        ),
      ),
    );

    final PopupMenuThemeData popupMenuTheme = PopupMenuTheme.of(context);
    return Material(
      shape: popupMenuTheme.shape,
      color: popupMenuTheme.color,
      type: MaterialType.card,
      elevation: popupMenuTheme.elevation ?? 8.0,
      child: child,
    );
  }

  @override
  Widget build(BuildContext context) {
    int? selectedItemIndex;
    if (initialValue != null) {
      for (int index = 0;
          selectedItemIndex == null && index < items.length;
          index += 1) {
        if (items[index].represents(initialValue)) selectedItemIndex = index;
      }
    }

    return MediaQuery.removePadding(
      context: context,
      removeTop: true,
      removeBottom: true,
      removeLeft: true,
      removeRight: true,
      child: Builder(
        builder: (BuildContext context) {
          return InheritedTheme.capture(from: context, to: context)
              .wrap(_buildMenu(context));
        },
      ),
    );
  }
}

Future<T?> showMenu<T>({
  required BuildContext context,
  required RelativeRect position,
  required List<PopupMenuEntry<T>> items,
  MenuWrapper? menuWrapper,
  T? initialValue,
  double? elevation,
  String? semanticLabel,
  ShapeBorder? shape,
  Color? color,
  bool useRootNavigator = false,
  BoxConstraints? constraints,
}) {
  assert(items.isNotEmpty);
  assert(debugCheckHasMaterialLocalizations(context));

  switch (Theme.of(context).platform) {
    case TargetPlatform.iOS:
    case TargetPlatform.macOS:
      break;
    case TargetPlatform.android:
    case TargetPlatform.fuchsia:
    case TargetPlatform.linux:
    case TargetPlatform.windows:
      semanticLabel ??= MaterialLocalizations.of(context).popupMenuLabel;
  }

  final NavigatorState navigator =
      Navigator.of(context, rootNavigator: useRootNavigator);
  return navigator.push(_PopupMenuRoute<T>(
    position: position,
    items: items,
    menuWrapper: menuWrapper,
    initialValue: initialValue,
    elevation: elevation,
    semanticLabel: semanticLabel,
    barrierLabel: MaterialLocalizations.of(context).modalBarrierDismissLabel,
    shape: shape,
    color: color,
    capturedThemes:
        InheritedTheme.capture(from: context, to: navigator.context),
    constraints: constraints,
  ));
}

typedef PopupMenuItemSelected<T> = void Function(T value);

typedef PopupMenuCanceled = void Function();

typedef PopupMenuItemBuilder<T> = List<PopupMenuEntry<T>> Function(
    BuildContext context);

typedef MenuWrapper = Widget Function(Widget child);

class PopupMenuButton<T> extends StatefulWidget {
  const PopupMenuButton({
    Key? key,
    required this.itemBuilder,
    this.menuWrapper,
    this.initialValue,
    this.onHover,
    this.onSelected,
    this.onCanceled,
    this.tooltip,
    this.elevation,
    this.padding = const EdgeInsets.all(8.0),
    this.child,
    this.splashRadius,
    this.icon,
    this.iconSize,
    this.offset = Offset.zero,
    this.enabled = true,
    this.shape,
    this.color,
    this.enableFeedback,
    this.constraints,
    this.position = PopupMenuPosition.over,
  })  : assert(
          !(child != null && icon != null),
          'You can only pass [child] or [icon], not both.',
        ),
        super(key: key);

  final PopupMenuItemBuilder<T> itemBuilder;

  final MenuWrapper? menuWrapper;

  final T? initialValue;

  final ValueChanged<bool>? onHover;

  final PopupMenuItemSelected<T>? onSelected;

  final PopupMenuCanceled? onCanceled;

  final String? tooltip;

  final double? elevation;

  final EdgeInsetsGeometry padding;

  final double? splashRadius;

  final Widget? child;

  final Widget? icon;

  final Offset offset;

  final bool enabled;

  final ShapeBorder? shape;

  final Color? color;

  final bool? enableFeedback;

  final double? iconSize;

  final BoxConstraints? constraints;

  final PopupMenuPosition position;

  @override
  PopupMenuButtonState<T> createState() => PopupMenuButtonState<T>();
}

class PopupMenuButtonState<T> extends State<PopupMenuButton<T>> {
  void showButtonMenu() {
    final PopupMenuThemeData popupMenuTheme = PopupMenuTheme.of(context);
    final RenderBox button = context.findRenderObject()! as RenderBox;
    final RenderBox overlay =
        Navigator.of(context).overlay!.context.findRenderObject()! as RenderBox;
    final Offset offset;
    switch (widget.position) {
      case PopupMenuPosition.over:
        offset = widget.offset;
        break;
      case PopupMenuPosition.under:
        offset =
            Offset(0.0, button.size.height - (widget.padding.vertical / 2)) +
                widget.offset;
        break;
      case PopupMenuPosition.overSide:
        offset =
            Offset(button.size.width - (widget.padding.horizontal / 2), 0.0) +
                widget.offset;
        break;
      case PopupMenuPosition.underSide:
        offset = Offset(button.size.width - (widget.padding.horizontal / 2),
                button.size.height - (widget.padding.vertical / 2)) +
            widget.offset;
        break;
    }
    final RelativeRect position = RelativeRect.fromRect(
      Rect.fromPoints(
        button.localToGlobal(offset, ancestor: overlay),
        button.localToGlobal(button.size.bottomRight(Offset.zero) + offset,
            ancestor: overlay),
      ),
      Offset.zero & overlay.size,
    );
    final List<PopupMenuEntry<T>> items = widget.itemBuilder(context);
    if (items.isNotEmpty) {
      showMenu<T?>(
        context: context,
        elevation: widget.elevation ?? popupMenuTheme.elevation,
        items: items,
        menuWrapper: widget.menuWrapper,
        initialValue: widget.initialValue,
        position: position,
        shape: widget.shape ?? popupMenuTheme.shape,
        color: widget.color ?? popupMenuTheme.color,
        constraints: widget.constraints,
      ).then<void>((T? newValue) {
        if (!mounted) return null;
        if (newValue == null) {
          widget.onCanceled?.call();
          return null;
        }
        widget.onSelected?.call(newValue);
      });
    }
  }

  bool get _canRequestFocus {
    final NavigationMode mode = MediaQuery.maybeOf(context)?.navigationMode ??
        NavigationMode.traditional;
    switch (mode) {
      case NavigationMode.traditional:
        return widget.enabled;
      case NavigationMode.directional:
        return true;
    }
  }

  @override
  Widget build(BuildContext context) {
    final bool enableFeedback = widget.enableFeedback ??
        PopupMenuTheme.of(context).enableFeedback ??
        true;

    assert(debugCheckHasMaterialLocalizations(context));

    if (widget.child != null) {
      return Tooltip(
        message:
            widget.tooltip ?? MaterialLocalizations.of(context).showMenuTooltip,
        child: InkWell(
          onTap: widget.enabled ? showButtonMenu : null,
          onHover: widget.onHover,
          canRequestFocus: _canRequestFocus,
          enableFeedback: enableFeedback,
          child: widget.child,
        ),
      );
    }

    return MenuButton(
      child: widget.icon ?? Icon(Icons.adaptive.more),
      tooltip:
          widget.tooltip ?? MaterialLocalizations.of(context).showMenuTooltip,
      onPressed: widget.enabled ? showButtonMenu : null,
      enableFeedback: enableFeedback,
      color: MyTheme.button,
      hoverColor: MyTheme.accent,
    );
  }
}

class _EffectiveMouseCursor extends MaterialStateMouseCursor {
  const _EffectiveMouseCursor(this.widgetCursor, this.themeCursor);

  final MouseCursor? widgetCursor;
  final MaterialStateProperty<MouseCursor?>? themeCursor;

  @override
  MouseCursor resolve(Set<MaterialState> states) {
    return MaterialStateProperty.resolveAs<MouseCursor?>(
            widgetCursor, states) ??
        themeCursor?.resolve(states) ??
        MaterialStateMouseCursor.clickable.resolve(states);
  }

  @override
  String get debugDescription => 'MaterialStateMouseCursor(PopupMenuItemState)';
}
