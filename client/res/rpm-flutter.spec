Name:       supportdesk
Version:    1.5.1
Release:    0
Summary:    RPM package
License:    GPL-3.0
URL:        https://rustdesk.com
Vendor:     SupportDesk
Requires:   gtk3 libxcb libXfixes alsa-lib libva gstreamer1-plugins-base
Recommends: libayatana-appindicator-gtk3 libxdo
Provides:   libdesktop_drop_plugin.so()(64bit), libdesktop_multi_window_plugin.so()(64bit), libfile_selector_linux_plugin.so()(64bit), libflutter_custom_cursor_plugin.so()(64bit), libflutter_linux_gtk.so()(64bit), libscreen_retriever_plugin.so()(64bit), libtray_manager_plugin.so()(64bit), liburl_launcher_linux_plugin.so()(64bit), libwindow_manager_plugin.so()(64bit), libwindow_size_plugin.so()(64bit), libtexture_rgba_renderer_plugin.so()(64bit)


%description
The best open-source remote desktop client software, written in Rust.

%prep

%build


%install

mkdir -p "%{buildroot}/usr/share/supportdesk" && cp -r ${HBB}/flutter/build/linux/x64/release/bundle/* -t "%{buildroot}/usr/share/supportdesk"
mkdir -p "%{buildroot}/usr/bin"
install -Dm 644 $HBB/res/supportdesk.service -t "%{buildroot}/usr/share/supportdesk/files"
install -Dm 644 $HBB/res/supportdesk.desktop -t "%{buildroot}/usr/share/supportdesk/files"
install -Dm 644 $HBB/res/supportdesk-link.desktop -t "%{buildroot}/usr/share/supportdesk/files"
install -Dm 644 $HBB/res/128x128@2x.png "%{buildroot}/usr/share/icons/hicolor/256x256/apps/supportdesk.png"
install -Dm 644 $HBB/res/scalable.svg "%{buildroot}/usr/share/icons/hicolor/scalable/apps/supportdesk.svg"

%files
/usr/share/supportdesk/*
/usr/share/supportdesk/files/supportdesk.service
/usr/share/icons/hicolor/256x256/apps/supportdesk.png
/usr/share/icons/hicolor/scalable/apps/supportdesk.svg
/usr/share/supportdesk/files/supportdesk.desktop
/usr/share/supportdesk/files/supportdesk-link.desktop

%changelog

%pre
case "$1" in
  1)
  ;;
  2)
    systemctl stop supportdesk || true
  ;;
esac

%post
cp /usr/share/supportdesk/files/supportdesk.service /etc/systemd/system/supportdesk.service
cp /usr/share/supportdesk/files/supportdesk.desktop /usr/share/applications/
cp /usr/share/supportdesk/files/supportdesk-link.desktop /usr/share/applications/
ln -sf /usr/share/supportdesk/supportdesk /usr/bin/supportdesk
systemctl daemon-reload
systemctl enable supportdesk
systemctl start supportdesk
update-desktop-database

%preun
case "$1" in
  0)
    systemctl stop supportdesk || true
    systemctl disable supportdesk || true
    rm /etc/systemd/system/supportdesk.service || true
  ;;
  1)
  ;;
esac

%postun
case "$1" in
  0)
    rm /usr/bin/supportdesk || true
    rmdir /usr/lib/supportdesk || true
    rmdir /usr/local/supportdesk || true
    rmdir /usr/share/supportdesk || true
    rm /usr/share/applications/supportdesk.desktop || true
    rm /usr/share/applications/supportdesk-link.desktop || true
    update-desktop-database
  ;;
  1)
    rmdir /usr/lib/supportdesk || true
    rmdir /usr/local/supportdesk || true
  ;;
esac
