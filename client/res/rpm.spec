Name:       supportdesk
Version:    1.5.1
Release:    0
Summary:    RPM package
License:    GPL-3.0
URL:        https://rustdesk.com
Vendor:     SupportDesk
Requires:   gtk3 libxcb libXfixes alsa-lib libva2 gstreamer1-plugins-base
Recommends: libayatana-appindicator-gtk3 libxdo


%description
The best open-source remote desktop client software, written in Rust.

%prep

%build

%global __python %{__python3}

%install
mkdir -p %{buildroot}/usr/bin/
mkdir -p %{buildroot}/usr/share/supportdesk/
mkdir -p %{buildroot}/usr/share/supportdesk/files/
mkdir -p %{buildroot}/usr/share/icons/hicolor/256x256/apps/
mkdir -p %{buildroot}/usr/share/icons/hicolor/scalable/apps/
install -m 755 $HBB/target/release/supportdesk %{buildroot}/usr/bin/supportdesk
install $HBB/libsciter-gtk.so %{buildroot}/usr/share/supportdesk/libsciter-gtk.so
install $HBB/res/supportdesk.service %{buildroot}/usr/share/supportdesk/files/
install $HBB/res/128x128@2x.png %{buildroot}/usr/share/icons/hicolor/256x256/apps/supportdesk.png
install $HBB/res/scalable.svg %{buildroot}/usr/share/icons/hicolor/scalable/apps/supportdesk.svg
install $HBB/res/supportdesk.desktop %{buildroot}/usr/share/supportdesk/files/
install $HBB/res/supportdesk-link.desktop %{buildroot}/usr/share/supportdesk/files/

%files
/usr/bin/supportdesk
/usr/share/supportdesk/libsciter-gtk.so
/usr/share/supportdesk/files/supportdesk.service
/usr/share/icons/hicolor/256x256/apps/supportdesk.png
/usr/share/icons/hicolor/scalable/apps/supportdesk.svg
/usr/share/supportdesk/files/supportdesk.desktop
/usr/share/supportdesk/files/supportdesk-link.desktop
/usr/share/supportdesk/files/__pycache__/*

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
    rm /usr/share/applications/supportdesk.desktop || true
    rm /usr/share/applications/supportdesk-link.desktop || true
    update-desktop-database
  ;;
  1)
  ;;
esac
