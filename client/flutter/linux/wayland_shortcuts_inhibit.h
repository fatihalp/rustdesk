
#ifndef WAYLAND_SHORTCUTS_INHIBIT_H_
#define WAYLAND_SHORTCUTS_INHIBIT_H_

#include <gtk/gtk.h>

#if defined(GDK_WINDOWING_WAYLAND) && defined(HAS_KEYBOARD_SHORTCUTS_INHIBIT)

void wayland_shortcuts_inhibit_init_for_subwindow(void* view);

#endif

#endif
