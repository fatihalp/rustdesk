#!/usr/bin/env bash
cd $(dirname $(dirname $(which flutter)))
git apply ~/supportdesk/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff
cd -
flutter build ipa --release
