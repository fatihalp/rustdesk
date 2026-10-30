
#include "wayland_shortcuts_inhibit.h"

#if defined(GDK_WINDOWING_WAYLAND) && defined(HAS_KEYBOARD_SHORTCUTS_INHIBIT)

#include <cstring>
#include <gdk/gdkwayland.h>
#include <wayland-client.h>
#include "keyboard-shortcuts-inhibit-unstable-v1-client-protocol.h"

typedef struct {
  struct zwp_keyboard_shortcuts_inhibit_manager_v1* manager;
  struct zwp_keyboard_shortcuts_inhibitor_v1* inhibitor;
} ShortcutsInhibitData;

static void shortcuts_inhibit_data_free(gpointer data) {
  ShortcutsInhibitData* inhibit_data = static_cast<ShortcutsInhibitData*>(data);
  if (inhibit_data->inhibitor != NULL) {
    zwp_keyboard_shortcuts_inhibitor_v1_destroy(inhibit_data->inhibitor);
  }
  if (inhibit_data->manager != NULL) {
    zwp_keyboard_shortcuts_inhibit_manager_v1_destroy(inhibit_data->manager);
  }
  g_free(inhibit_data);
}

static void registry_handle_global(void* data, struct wl_registry* registry,
                                   uint32_t name, const char* interface,
                                   uint32_t ) {
  ShortcutsInhibitData* inhibit_data = static_cast<ShortcutsInhibitData*>(data);
  if (strcmp(interface,
             zwp_keyboard_shortcuts_inhibit_manager_v1_interface.name) == 0) {
    inhibit_data->manager =
        static_cast<zwp_keyboard_shortcuts_inhibit_manager_v1*>(wl_registry_bind(
            registry, name, &zwp_keyboard_shortcuts_inhibit_manager_v1_interface,
            1));
  }
}

static void registry_handle_global_remove(void* , struct wl_registry* ,
                                          uint32_t ) {
}

static const struct wl_registry_listener registry_listener = {
    registry_handle_global,
    registry_handle_global_remove,
};

static void inhibitor_active(void* ,
                             struct zwp_keyboard_shortcuts_inhibitor_v1* ) {
}

static void inhibitor_inactive(void* ,
                               struct zwp_keyboard_shortcuts_inhibitor_v1* ) {
}

static const struct zwp_keyboard_shortcuts_inhibitor_v1_listener inhibitor_listener = {
    inhibitor_active,
    inhibitor_inactive,
};

static void uninhibit_keyboard_shortcuts(GtkWindow* window);

static void inhibit_keyboard_shortcuts(GtkWindow* window) {
  GdkDisplay* display = gtk_widget_get_display(GTK_WIDGET(window));
  if (!GDK_IS_WAYLAND_DISPLAY(display)) {
    return;
  }

  if (g_object_get_data(G_OBJECT(window), "shortcuts-inhibit-data") != NULL) {
    return;
  }

  ShortcutsInhibitData* inhibit_data = g_new0(ShortcutsInhibitData, 1);

  struct wl_display* wl_display = gdk_wayland_display_get_wl_display(display);
  if (wl_display == NULL) {
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  struct wl_registry* registry = wl_display_get_registry(wl_display);
  if (registry == NULL) {
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  wl_registry_add_listener(registry, &registry_listener, inhibit_data);
  wl_display_roundtrip(wl_display);

  if (inhibit_data->manager == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  GdkWindow* gdk_window = gtk_widget_get_window(GTK_WIDGET(window));
  if (gdk_window == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  struct wl_surface* surface = gdk_wayland_window_get_wl_surface(gdk_window);
  if (surface == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  GdkSeat* gdk_seat = gdk_display_get_default_seat(display);
  if (gdk_seat == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  struct wl_seat* seat = gdk_wayland_seat_get_wl_seat(gdk_seat);
  if (seat == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  inhibit_data->inhibitor =
      zwp_keyboard_shortcuts_inhibit_manager_v1_inhibit_shortcuts(
          inhibit_data->manager, surface, seat);

  if (inhibit_data->inhibitor == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  zwp_keyboard_shortcuts_inhibitor_v1_add_listener(
      inhibit_data->inhibitor, &inhibitor_listener, window);

  wl_display_roundtrip(wl_display);
  wl_registry_destroy(registry);

  g_object_set_data_full(G_OBJECT(window), "shortcuts-inhibit-data",
                         inhibit_data, shortcuts_inhibit_data_free);
}

static void uninhibit_keyboard_shortcuts(GtkWindow* window) {
  ShortcutsInhibitData* inhibit_data = static_cast<ShortcutsInhibitData*>(
      g_object_get_data(G_OBJECT(window), "shortcuts-inhibit-data"));

  if (inhibit_data == NULL) {
    return;
  }

  g_object_set_data(G_OBJECT(window), "shortcuts-inhibit-data", NULL);
}

static gboolean on_window_focus_in(GtkWidget* widget, GdkEventFocus* , gpointer ) {
  if (GTK_IS_WINDOW(widget)) {
    inhibit_keyboard_shortcuts(GTK_WINDOW(widget));
  }
  return FALSE;
}

static gboolean on_window_focus_out(GtkWidget* widget, GdkEventFocus* , gpointer ) {
  if (GTK_IS_WINDOW(widget)) {
    uninhibit_keyboard_shortcuts(GTK_WINDOW(widget));
  }
  return FALSE;
}

static const char* const kFocusHandlersConnectedKey = "shortcuts-inhibit-focus-handlers-connected";
static const char* const kRealizeHandlerConnectedKey = "shortcuts-inhibit-realize-handler-connected";

static void on_window_realize(GtkWidget* widget, gpointer ) {
  if (GTK_IS_WINDOW(widget)) {
    if (g_object_get_data(G_OBJECT(widget), kFocusHandlersConnectedKey) != NULL) {
      return;
    }

    g_signal_connect(widget, "focus-in-event",
                     G_CALLBACK(on_window_focus_in), NULL);
    g_signal_connect(widget, "focus-out-event",
                     G_CALLBACK(on_window_focus_out), NULL);

    g_object_set_data(G_OBJECT(widget), kFocusHandlersConnectedKey, GINT_TO_POINTER(1));

    if (gtk_window_has_toplevel_focus(GTK_WINDOW(widget))) {
      inhibit_keyboard_shortcuts(GTK_WINDOW(widget));
    }
  }
}

void wayland_shortcuts_inhibit_init_for_subwindow(void* view) {
  GtkWidget* widget = GTK_WIDGET(view);
  GtkWidget* toplevel = gtk_widget_get_toplevel(widget);

  if (toplevel != NULL && GTK_IS_WINDOW(toplevel)) {
    if (g_object_get_data(G_OBJECT(toplevel), kFocusHandlersConnectedKey) != NULL ||
        g_object_get_data(G_OBJECT(toplevel), kRealizeHandlerConnectedKey) != NULL) {
      return;
    }

    if (gtk_widget_get_realized(toplevel)) {
      on_window_realize(toplevel, NULL);
    } else {
      g_object_set_data(G_OBJECT(toplevel), kRealizeHandlerConnectedKey, GINT_TO_POINTER(1));
      g_signal_connect(toplevel, "realize",
                       G_CALLBACK(on_window_realize), NULL);
    }
  }
}

#endif
