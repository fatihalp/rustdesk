cmake_minimum_required(VERSION 3.10)
project(runner LANGUAGES C CXX)

set(BINARY_NAME "supportdesk")
set(APPLICATION_ID "com.carriez.flutter_hbb")

cmake_policy(SET CMP0063 NEW)

set(CMAKE_INSTALL_RPATH "$ORIGIN/lib")

if(FLUTTER_TARGET_PLATFORM_SYSROOT)
  set(CMAKE_SYSROOT ${FLUTTER_TARGET_PLATFORM_SYSROOT})
  set(CMAKE_FIND_ROOT_PATH ${CMAKE_SYSROOT})
  set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
  set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
  set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
  set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
endif()

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE "Debug" CACHE
    STRING "Flutter build mode" FORCE)
  set_property(CACHE CMAKE_BUILD_TYPE PROPERTY STRINGS
    "Debug" "Profile" "Release")
endif()

function(APPLY_STANDARD_SETTINGS TARGET)
  target_compile_features(${TARGET} PUBLIC cxx_std_14)
  target_compile_options(${TARGET} PRIVATE -Wall -Werror)
  target_compile_options(${TARGET} PRIVATE "$<$<NOT:$<CONFIG:Debug>>:-O3>")
  target_compile_definitions(${TARGET} PRIVATE "$<$<NOT:$<CONFIG:Debug>>:NDEBUG>")
endfunction()

set(FLUTTER_MANAGED_DIR "${CMAKE_CURRENT_SOURCE_DIR}/flutter")
add_subdirectory(${FLUTTER_MANAGED_DIR})

find_package(PkgConfig REQUIRED)
pkg_check_modules(GTK REQUIRED IMPORTED_TARGET gtk+-3.0)

pkg_check_modules(WAYLAND_CLIENT IMPORTED_TARGET wayland-client)
pkg_check_modules(WAYLAND_PROTOCOLS_PKG QUIET wayland-protocols)
pkg_check_modules(WAYLAND_SCANNER_PKG QUIET wayland-scanner)

if(WAYLAND_PROTOCOLS_PKG_FOUND)
  pkg_get_variable(WAYLAND_PROTOCOLS_DIR wayland-protocols pkgdatadir)
endif()
if(WAYLAND_SCANNER_PKG_FOUND)
  pkg_get_variable(WAYLAND_SCANNER wayland-scanner wayland_scanner)
endif()

if(WAYLAND_CLIENT_FOUND AND WAYLAND_PROTOCOLS_DIR AND WAYLAND_SCANNER)
  set(KEYBOARD_SHORTCUTS_INHIBIT_PROTOCOL
    "${WAYLAND_PROTOCOLS_DIR}/unstable/keyboard-shortcuts-inhibit/keyboard-shortcuts-inhibit-unstable-v1.xml")

  if(EXISTS ${KEYBOARD_SHORTCUTS_INHIBIT_PROTOCOL})
    set(WAYLAND_GENERATED_DIR "${CMAKE_CURRENT_BINARY_DIR}/wayland-protocols")
    file(MAKE_DIRECTORY ${WAYLAND_GENERATED_DIR})

    add_custom_command(
      OUTPUT "${WAYLAND_GENERATED_DIR}/keyboard-shortcuts-inhibit-unstable-v1-client-protocol.h"
      COMMAND ${WAYLAND_SCANNER} client-header
        ${KEYBOARD_SHORTCUTS_INHIBIT_PROTOCOL}
        "${WAYLAND_GENERATED_DIR}/keyboard-shortcuts-inhibit-unstable-v1-client-protocol.h"
      DEPENDS ${KEYBOARD_SHORTCUTS_INHIBIT_PROTOCOL}
      VERBATIM
    )

    add_custom_command(
      OUTPUT "${WAYLAND_GENERATED_DIR}/keyboard-shortcuts-inhibit-unstable-v1-protocol.c"
      COMMAND ${WAYLAND_SCANNER} private-code
        ${KEYBOARD_SHORTCUTS_INHIBIT_PROTOCOL}
        "${WAYLAND_GENERATED_DIR}/keyboard-shortcuts-inhibit-unstable-v1-protocol.c"
      DEPENDS ${KEYBOARD_SHORTCUTS_INHIBIT_PROTOCOL}
      VERBATIM
    )

    set(WAYLAND_PROTOCOL_SOURCES
      "${WAYLAND_GENERATED_DIR}/keyboard-shortcuts-inhibit-unstable-v1-client-protocol.h"
      "${WAYLAND_GENERATED_DIR}/keyboard-shortcuts-inhibit-unstable-v1-protocol.c"
    )

    set(HAS_KEYBOARD_SHORTCUTS_INHIBIT TRUE)
  endif()
endif()

add_definitions(-DAPPLICATION_ID="${APPLICATION_ID}")

add_executable(${BINARY_NAME}
  "main.cc"
  "my_application.cc"
  "wayland_shortcuts_inhibit.cc"
  "bump_mouse.cc"
  "bump_mouse_x11.cc"
  "${FLUTTER_MANAGED_DIR}/generated_plugin_registrant.cc"
  ${WAYLAND_PROTOCOL_SOURCES}
)

apply_standard_settings(${BINARY_NAME})

target_link_libraries(${BINARY_NAME} PRIVATE flutter)
target_link_libraries(${BINARY_NAME} PRIVATE PkgConfig::GTK)
target_link_libraries(${BINARY_NAME} PRIVATE ${CMAKE_DL_LIBS})

if(HAS_KEYBOARD_SHORTCUTS_INHIBIT)
  target_compile_definitions(${BINARY_NAME} PRIVATE HAS_KEYBOARD_SHORTCUTS_INHIBIT)
  target_include_directories(${BINARY_NAME} PRIVATE ${WAYLAND_GENERATED_DIR})
  target_link_libraries(${BINARY_NAME} PRIVATE PkgConfig::WAYLAND_CLIENT)
endif()

add_dependencies(${BINARY_NAME} flutter_assemble)

set_target_properties(${BINARY_NAME}
  PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/intermediates_do_not_run"
)

include(flutter/generated_plugins.cmake)


set(BUILD_BUNDLE_DIR "${PROJECT_BINARY_DIR}/bundle")
  set(CMAKE_INSTALL_PREFIX "${BUILD_BUNDLE_DIR}" CACHE PATH "..." FORCE)

install(CODE "
  file(REMOVE_RECURSE \"${BUILD_BUNDLE_DIR}/\")
  " COMPONENT Runtime)

set(INSTALL_BUNDLE_DATA_DIR "${CMAKE_INSTALL_PREFIX}/data")
set(INSTALL_BUNDLE_LIB_DIR "${CMAKE_INSTALL_PREFIX}/lib")

install(TARGETS ${BINARY_NAME} RUNTIME DESTINATION "${CMAKE_INSTALL_PREFIX}"
  COMPONENT Runtime)

install(FILES "${FLUTTER_ICU_DATA_FILE}" DESTINATION "${INSTALL_BUNDLE_DATA_DIR}"
  COMPONENT Runtime)

install(FILES "${FLUTTER_LIBRARY}" DESTINATION "${INSTALL_BUNDLE_LIB_DIR}"
  COMPONENT Runtime)

foreach(bundled_library ${PLUGIN_BUNDLED_LIBRARIES})
  install(FILES "${bundled_library}"
    DESTINATION "${INSTALL_BUNDLE_LIB_DIR}"
    COMPONENT Runtime)
endforeach(bundled_library)

set(SUPPORTDESK_LIB_BUILD_TYPE $<IF:$<CONFIG:Debug>,debug,release>)
string(TOLOWER ${CMAKE_BUILD_TYPE} ${SUPPORTDESK_LIB_BUILD_TYPE})
message(STATUS "supportdesk lib build type: ${SUPPORTDESK_LIB_BUILD_TYPE}")

set(SUPPORTDESK_LIB "../../target/${SUPPORTDESK_LIB_BUILD_TYPE}/liblibsupportdesk.so")
install(FILES "${SUPPORTDESK_LIB}" DESTINATION "${INSTALL_BUNDLE_LIB_DIR}"
COMPONENT Runtime RENAME libsupportdesk.so)

set(FLUTTER_ASSET_DIR_NAME "flutter_assets")
install(CODE "
  file(REMOVE_RECURSE \"${INSTALL_BUNDLE_DATA_DIR}/${FLUTTER_ASSET_DIR_NAME}\")
  " COMPONENT Runtime)
install(DIRECTORY "${PROJECT_BUILD_DIR}/${FLUTTER_ASSET_DIR_NAME}"
  DESTINATION "${INSTALL_BUNDLE_DATA_DIR}" COMPONENT Runtime)

if(NOT CMAKE_BUILD_TYPE MATCHES "Debug")
  install(FILES "${AOT_LIBRARY}" DESTINATION "${INSTALL_BUNDLE_LIB_DIR}"
    COMPONENT Runtime)
endif()
