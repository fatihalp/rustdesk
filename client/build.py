#!/usr/bin/env python3

import os
import glob
import contextlib
import pathlib
import platform
import zipfile
import urllib.request
import shutil
import hashlib
import re
import subprocess
import argparse
import sys
from pathlib import Path

REPO_ROOT = os.path.dirname(os.path.abspath(__file__))

windows = platform.platform().startswith('Windows')
osx = platform.platform().startswith(
    'Darwin') or platform.platform().startswith("macOS")
hbb_name = 'supportdesk' + ('.exe' if windows else '')
exe_path = 'target/release/' + hbb_name
if windows:
    win_arch = 'arm64' if platform.machine().lower() in ('arm64', 'aarch64') else 'x64'
    flutter_build_dir = f'build/windows/{win_arch}/runner/Release/'
elif osx:
    flutter_build_dir = 'build/macos/Build/Products/Release/'
else:
    flutter_build_dir = 'build/linux/x64/release/bundle/'
flutter_build_dir_2 = f'flutter/{flutter_build_dir}'
skip_cargo = False


def get_deb_arch() -> str:
    custom_arch = os.environ.get("DEB_ARCH")
    if custom_arch is None:
        return "amd64"
    return custom_arch

def get_deb_extra_depends() -> str:
    custom_arch = os.environ.get("DEB_ARCH")
    if custom_arch == "armhf":
        return ", libatomic1"
    return ""

def system2(cmd):
    exit_code = os.system(cmd)
    if exit_code != 0:
        sys.stderr.write(f"Error occurred when executing: `{cmd}`. Exiting.\n")
        sys.exit(-1)


def get_version():
    with open("Cargo.toml", encoding="utf-8") as fh:
        for line in fh:
            if line.startswith("version"):
                return line.replace("version", "").replace("=", "").replace('"', '').strip()
    return ''


def parse_rc_features(feature):
    available_features = {}
    apply_features = {}
    if not feature:
        feature = []

    def platform_check(platforms):
        if windows:
            return 'windows' in platforms
        elif osx:
            return 'osx' in platforms
        else:
            return 'linux' in platforms

    def get_all_features():
        features = []
        for (feat, feat_info) in available_features.items():
            if platform_check(feat_info['platform']):
                features.append(feat)
        return features

    if isinstance(feature, str) and feature.upper() == 'ALL':
        return get_all_features()
    elif isinstance(feature, list):
        if windows:
            pass
        for feat in feature:
            if isinstance(feat, str) and feat.upper() == 'ALL':
                return get_all_features()
            if feat in available_features:
                if platform_check(available_features[feat]['platform']):
                    apply_features[feat] = available_features[feat]
            else:
                print(f'Unrecognized feature {feat}')
        return apply_features
    else:
        raise Exception(f'Unsupported features param {feature}')


def make_parser():
    parser = argparse.ArgumentParser(description='Build script.')
    parser.add_argument(
        '-f',
        '--feature',
        dest='feature',
        metavar='N',
        type=str,
        nargs='+',
        default='',
        help='Integrate features, windows only.'
             'Available: [Not used for now]. Special value is "ALL" and empty "". Default is empty.')
    parser.add_argument('--flutter', action='store_true',
                        help='Build flutter package', default=False)
    parser.add_argument(
        '--hwcodec',
        action='store_true',
        help='Enable feature hwcodec' + (
            '' if windows or osx else ', need libva-dev.')
    )
    parser.add_argument(
        '--vram',
        action='store_true',
        help='Enable feature vram, only available on windows now.'
    )
    parser.add_argument(
        '--portable',
        action='store_true',
        help='Build windows portable'
    )
    parser.add_argument(
        '--unix-file-copy-paste',
        action='store_true',
        help='Build with unix file copy paste feature'
    )
    parser.add_argument(
        '--drm',
        action='store_true',
        help='Linux only: build the DRM/KMS capture backend (bundles libdrmtap.so, '
             'dlopen-ed in-process by the root service). Off by default.'
    )
    parser.add_argument(
        '--print-features',
        action='store_true',
        help='Print the cargo feature list these flags select, and exit without building. For a '
             'caller that runs its own cargo line and then packages with --skip-cargo: it can ask '
             'for the list rather than repeat it, so the two cannot drift.'
    )
    parser.add_argument(
        '--skip-cargo',
        action='store_true',
        help='Skip cargo build process, only flutter version + Linux supported currently'
    )
    if windows:
        parser.add_argument(
            '--skip-portable-pack',
            action='store_true',
            help='Skip packing, only flutter version + Windows supported'
        )
    parser.add_argument(
        "--package",
        type=str
    )
    if osx:
        parser.add_argument(
            '--screencapturekit',
            action='store_true',
            help='Enable feature screencapturekit'
        )
    return parser


def generate_build_script_for_docker():
    with open("/tmp/build.sh", "w") as f:
        f.write('''
            #!/bin/bash
            # environment
            export CPATH="$(clang -v 2>&1 | grep "Selected GCC installation: " | cut -d' ' -f4-)/include"
            # flutter
            pushd /opt
            wget https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_3.0.5-stable.tar.xz
            tar -xvf flutter_linux_3.0.5-stable.tar.xz
            export PATH=`pwd`/flutter/bin:$PATH
            popd
            # flutter_rust_bridge
            dart pub global activate ffigen --version 5.0.1
            pushd /tmp && git clone https://github.com/SoLongAndThanksForAllThePizza/flutter_rust_bridge --depth=1 && popd
            pushd /tmp/flutter_rust_bridge/frb_codegen && cargo install --path . --locked && popd
            pushd flutter && flutter pub get && popd
            ~/.cargo/bin/flutter_rust_bridge_codegen --rust-input ./src/flutter_ffi.rs --dart-output ./flutter/lib/generated_bridge.dart
            # install vcpkg
            pushd /opt
            export VCPKG_ROOT=`pwd`/vcpkg
            git clone https://github.com/microsoft/vcpkg
            vcpkg/bootstrap-vcpkg.sh
            popd
            $VCPKG_ROOT/vcpkg install --x-install-root="$VCPKG_ROOT/installed"
            # build supportdesk
            ./build.py --flutter --hwcodec
        ''')
    system2("chmod +x /tmp/build.sh")
    system2("bash /tmp/build.sh")


def download_extract_features(features, res_dir):
    import re

    proxy = ''

    def req(url):
        if not proxy:
            return url
        else:
            r = urllib.request.Request(url)
            r.set_proxy(proxy, 'http')
            r.set_proxy(proxy, 'https')
            return r

    for (feat, feat_info) in features.items():
        includes = feat_info['include'] if 'include' in feat_info and feat_info['include'] else []
        includes = [re.compile(p) for p in includes]
        excludes = feat_info['exclude'] if 'exclude' in feat_info and feat_info['exclude'] else []
        excludes = [re.compile(p) for p in excludes]

        print(f'{feat} download begin')
        download_filename = feat_info['zip_url'].split('/')[-1]
        checksum_md5_response = urllib.request.urlopen(
            req(feat_info['checksum_url']))
        for line in checksum_md5_response.read().decode('utf-8').splitlines():
            if line.split()[1] == download_filename:
                checksum_md5 = line.split()[0]
                filename, _headers = urllib.request.urlretrieve(feat_info['zip_url'],
                                                                download_filename)
                md5 = hashlib.md5(open(filename, 'rb').read()).hexdigest()
                if checksum_md5 != md5:
                    raise Exception(f'{feat} download failed')
                print(f'{feat} download end. extract bein')
                zip_file = zipfile.ZipFile(filename)
                zip_list = zip_file.namelist()
                for f in zip_list:
                    file_exclude = False
                    for p in excludes:
                        if p.match(f) is not None:
                            file_exclude = True
                            break
                    if file_exclude:
                        continue

                    file_include = False if includes else True
                    for p in includes:
                        if p.match(f) is not None:
                            file_include = True
                            break
                    if file_include:
                        print(f'extract file {f}')
                        zip_file.extract(f, res_dir)
                zip_file.close()
                os.remove(download_filename)
                print(f'{feat} extract end')


def external_resources(flutter, args, res_dir):
    features = parse_rc_features(args.feature)
    if not features:
        return

    print(f'Build with features {list(features.keys())}')
    if os.path.isdir(res_dir) and not os.path.islink(res_dir):
        shutil.rmtree(res_dir)
    elif os.path.exists(res_dir):
        raise Exception(f'Find file {res_dir}, not a directory')
    os.makedirs(res_dir, exist_ok=True)
    download_extract_features(features, res_dir)
    if flutter:
        os.makedirs(flutter_build_dir_2, exist_ok=True)
        for f in pathlib.Path(res_dir).iterdir():
            print(f'{f}')
            if f.is_file():
                shutil.copy2(f, flutter_build_dir_2)
            else:
                shutil.copytree(f, f'{flutter_build_dir_2}{f.stem}')


def linux_packaging_branch():
    """Which packaging path `main()` will take on THIS host.

    MUST mirror the elif chain in main() (pacman / yum / zypper / else), and exists so `--drm` can
    refuse a branch that is not drm-aware instead of silently producing a stock-named package with
    the capture backend compiled in. Only the final `deb` branch reaches `build_flutter_deb`, which
    is what bundles libdrmtap, renames the package, adds Conflicts/Provides and asserts the staged
    binary really is a drm build.
    """
    if os.path.isfile('/usr/bin/pacman'):
        return 'pacman'
    if os.path.isfile('/usr/bin/yum'):
        return 'yum'
    if os.path.isfile('/usr/bin/zypper'):
        return 'zypper'
    return 'deb'


def get_features(args):
    features = ['inline'] if not args.flutter else []
    if args.hwcodec:
        features.append('hwcodec')
    if args.vram:
        features.append('vram')
    if args.flutter:
        features.append('flutter')
    if args.unix_file_copy_paste:
        features.append('unix-file-copy-paste')
    if args.drm:
        if windows or osx:
            raise Exception('--drm is Linux only')
        branch = linux_packaging_branch()
        if branch != 'deb':
            raise Exception(
                f'--drm is only supported on the deb packaging path; this host would package via '
                f'{branch}, which cannot bundle libdrmtap or name the package distinctly')
        features.append('drm')
        features.append('drm-wake')
    if osx:
        if args.screencapturekit:
            features.append('screencapturekit')
    print("features:", features)
    return features


def generate_control_file(version):
    control_file_path = "../res/DEBIAN/control"
    system2('/bin/rm -rf %s' % control_file_path)

    content = """Package: supportdesk
Section: net
Priority: optional
Version: %s
Architecture: %s
Maintainer: SupportDesk
Homepage: https://rustdesk.com
Depends: libgtk-3-0t64 | libgtk-3-0, libxcb-randr0, libxdo3 | libxdo4, libxfixes3, libxcb-shape0, libxcb-xfixes0, libasound2t64 | libasound2, libsystemd0, curl, libva2, libva-drm2, libva-x11-2, libgstreamer-plugins-base1.0-0, gstreamer1.0-pipewire%s
Recommends: libayatana-appindicator3-1
Description: A remote control software.

""" % (version, get_deb_arch(), get_deb_extra_depends())
    file = open(control_file_path, "w")
    file.write(content)
    file.close()


def ffi_bindgen_function_refactor():
    system2(
        'sed -i "s/ffi.NativeFunction<ffi.Bool Function(DartPort/ffi.NativeFunction<ffi.Uint8 Function(DartPort/g" flutter/lib/generated_bridge.dart')


LIBDRMTAP_REPO_PINNED = 'https://github.com/rustdesk-org/libdrmtap'
LIBDRMTAP_SHA_PINNED = '5936a7a88b94aa43516a3dd0eb509d2383f31e2d'
LIBDRMTAP_REPO = os.environ.get('DRMTAP_REPO', LIBDRMTAP_REPO_PINNED)
LIBDRMTAP_SHA = os.environ.get('DRMTAP_SHA', LIBDRMTAP_SHA_PINNED)
DRMTAP_UNPINNED_OK = os.environ.get('DRMTAP_ALLOW_UNPINNED') == '1'


def _prebuilt_dir_is_the_pinned_checkout(prebuilt_dir):
    src = os.path.join(REPO_ROOT, 'third_party', 'libdrmtap')
    try:
        inside = os.path.commonpath([os.path.abspath(prebuilt_dir), src]) == src
    except ValueError:
        return False
    if not inside or not os.path.isdir(os.path.join(src, '.git')):
        return False
    try:
        head = subprocess.check_output(['git', '-C', src, 'rev-parse', 'HEAD']).decode().strip()
    except (subprocess.SubprocessError, OSError):
        return False
    return head == LIBDRMTAP_SHA


def _validate_libdrmtap_pin():
    prebuilt = os.environ.get('DRMTAP_PREBUILT_DIR') or None
    if prebuilt and _prebuilt_dir_is_the_pinned_checkout(prebuilt):
        prebuilt = None
    overridden = [
        name
        for name, value, pinned in (
            ('DRMTAP_REPO', LIBDRMTAP_REPO, LIBDRMTAP_REPO_PINNED),
            ('DRMTAP_SHA', LIBDRMTAP_SHA, LIBDRMTAP_SHA_PINNED),
            ('DRMTAP_PREBUILT_DIR', prebuilt, None),
        )
        if value != pinned
    ]
    if overridden and not DRMTAP_UNPINNED_OK:
        raise Exception(
            f'{", ".join(overridden)} would build libdrmtap from something other than the pinned '
            f'{LIBDRMTAP_REPO_PINNED} at {LIBDRMTAP_SHA_PINNED}. That is supported for local work and '
            'cross-builds, but it has to be deliberate: set DRMTAP_ALLOW_UNPINNED=1 as well.')
    if overridden:
        print(f'WARNING: libdrmtap is NOT the pinned build ({", ".join(overridden)} set)')
    if not re.fullmatch(r'[0-9a-f]{40}', LIBDRMTAP_SHA):
        raise Exception(
            f'DRMTAP_SHA must be a full 40-character commit sha, got {LIBDRMTAP_SHA!r}')
    if not re.fullmatch(r'(https://|git@)[A-Za-z0-9._~:/@-]+', LIBDRMTAP_REPO):
        raise Exception(f'DRMTAP_REPO does not look like a git remote url: {LIBDRMTAP_REPO!r}')


def _single_real_so(paths, where):
    real = sorted(p for p in paths if os.path.isfile(p) and not os.path.islink(p))
    if len(real) != 1:
        raise Exception(
            f'expected exactly one real libdrmtap.so.0.* in {where}, found {len(real)}: {real}')
    return real[0]


def build_libdrmtap_so():
    _validate_libdrmtap_pin()
    prebuilt_dir = os.environ.get('DRMTAP_PREBUILT_DIR')
    if prebuilt_dir:
        prebuilt = glob.glob(os.path.join(prebuilt_dir, 'libdrmtap.so.0.*'))
        so = _single_real_so(prebuilt, f'DRMTAP_PREBUILT_DIR={prebuilt_dir}')
        _assert_so_has_egl(so)
        _assert_so_has_no_helper(so)
        return so
    src = os.path.join(REPO_ROOT, 'third_party', 'libdrmtap')
    if not os.path.exists(os.path.join(src, 'meson.build')):
        if os.path.isdir(src):
            shutil.rmtree(src)
        os.makedirs(src, exist_ok=True)
        system2(f'git -C "{src}" init -q')
        system2(f'git -C "{src}" remote add origin {LIBDRMTAP_REPO}')
        system2(f'git -C "{src}" fetch --depth 1 origin {LIBDRMTAP_SHA}')
        system2(f'git -C "{src}" checkout -q FETCH_HEAD')
    if os.path.isdir(os.path.join(src, '.git')):
        got_sha = subprocess.check_output(
            ['git', '-C', src, 'rev-parse', 'HEAD']).decode().strip()
        if got_sha != LIBDRMTAP_SHA:
            shutil.rmtree(src, ignore_errors=True)
            raise Exception(
                f'libdrmtap at {src} is {got_sha}, expected {LIBDRMTAP_SHA} '
                f'(stale checkout from a different pin; removed, re-run to re-fetch)')
    build_dir = os.path.join(src, 'build-pkg')
    if not os.path.exists(os.path.join(build_dir, 'build.ninja')):
        system2(f'meson setup "{build_dir}" "{src}" --buildtype=release -Dhelper=disabled')
    else:
        system2(f'meson configure "{build_dir}" -Dhelper=disabled')
    system2(f'meson compile -C "{build_dir}" drmtap:shared_library')
    sos = glob.glob(os.path.join(build_dir, 'libdrmtap.so.0.*'))
    so = _single_real_so(sos, f'the libdrmtap meson build dir {build_dir}')
    _assert_so_has_egl(so)
    _assert_so_has_no_helper(so)
    return so


def _assert_so_has_no_helper(so_path):
    with open(so_path, 'rb') as f:
        blob = f.read()
    found = [m.decode() for m in (b'socketpair', b'/usr/libexec/drmtap-helper') if m in blob]
    if found:
        raise Exception(
            f'{so_path} still carries the privileged-helper fallback ({", ".join(found)}). '
            f'It must be built with -Dhelper=disabled: supportdesk captures as root and never uses '
            f'the helper, so this is a fork/exec path in the privileged process that nothing '
            f'reaches. Delete third_party/libdrmtap/build-pkg and rebuild.')


def _assert_so_has_egl(so_path):
    try:
        with open(so_path, 'rb') as f:
            blob = f.read()
    except OSError as err:
        raise Exception(f'cannot read the built libdrmtap at {so_path}: {err}') from err
    missing = [m for m in (b'libEGL.so.1', b'eglCreateImageKHR') if m not in blob]
    if missing:
        raise Exception(
            f'{so_path} looks like a CPU-only libdrmtap stub (missing '
            f'{", ".join(m.decode() for m in missing)}): the EGL detile path the split capture '
            'depends on is not in it, and DRM capture would silently fall back to PipeWire. '
            'Install the EGL development packages and rebuild (Debian/Ubuntu: libegl-dev '
            'libgles2-mesa-dev; Arch: mesa libglvnd).')


DRM_PACKAGE_NAME = 'supportdesk-unattended-wayland'


def assert_so_satisfies_the_runtime_abi_gate(so_path):
    """The .so we are about to ship must be one the RUNTIME will actually accept.

    `abi_accepted` in libs/scrap/src/common/drmtap_dl.rs is the only place the pinned library's
    version is ever validated, and it runs at dlopen time on the USER's machine. Nothing in the
    build or in CI compared the two, so the pin and the gate could drift apart and every existing
    assertion would still pass: the EGL check does not look at the version, the CI symbol contract
    does not call drmtap_version(), and the deb-contents regex matches any `libdrmtap.so.0.X.Y`.
    A green pipeline could therefore produce a deb in which DRM capture can never start, and the
    only symptom on the host is one log line before it falls back to the portal.

    So parse the gate out of the Rust and apply it here, to the object being staged. This is the
    same rule, not a copy of the numbers: if someone bumps the constants, this reads the new ones.
    """
    m = re.search(r'libdrmtap\.so\.(\d+)\.(\d+)\.(\d+)', os.path.basename(so_path))
    if not m:
        print(f'[drm] cannot read a version out of {so_path}; skipping the ABI-gate cross-check')
        return
    so_ver = tuple(int(g) for g in m.groups())
    gate_path = os.path.join(REPO_ROOT, 'libs', 'scrap', 'src', 'common', 'drmtap_dl.rs')
    with open(gate_path) as f:
        gate_src = f.read()

    def _const(name):
        mm = re.search(rf'const {name}: c_int = (\d+);', gate_src)
        return int(mm.group(1)) if mm else None

    major, minor = _const('DRMTAP_ABI_MAJOR'), _const('DRMTAP_ABI_MINOR')
    mm = re.search(r'const DRMTAP_MIN_MINOR_PATCH: \(c_int, c_int\) = \((\d+), (\d+)\);', gate_src)
    floor = (int(mm.group(1)), int(mm.group(2))) if mm else None
    if major is None or minor is None or floor is None:
        raise Exception(
            'could not parse the libdrmtap ABI gate out of drmtap_dl.rs (DRMTAP_ABI_MAJOR / '
            'DRMTAP_ABI_MINOR / DRMTAP_MIN_MINOR_PATCH). The gate moved and this check did not; '
            'fix the check rather than removing it, or the pin and the gate can drift silently.')
    accepted = so_ver[0] == major and so_ver[1] == minor and (so_ver[1], so_ver[2]) >= floor
    if not accepted:
        raise Exception(
            f'the libdrmtap being packaged is {so_ver[0]}.{so_ver[1]}.{so_ver[2]}, which the '
            f'runtime loader would REFUSE: drmtap_dl.rs accepts exactly major {major}, minor '
            f'{minor}, patch >= {floor[1]}. Shipping it produces a deb whose DRM capture can never '
            'start. Move the build pin and the gate together, or fix whichever one is wrong.')
    print(f'[drm] libdrmtap {so_ver[0]}.{so_ver[1]}.{so_ver[2]} satisfies the runtime ABI gate '
          f'(major {major}, minor {minor}, patch >= {floor[1]})')


def stage_libdrmtap_into_deb(so_path):
    assert_so_satisfies_the_runtime_abi_gate(so_path)
    so_basename = os.path.basename(so_path)
    system2('mkdir -p tmpdeb/usr/lib/supportdesk')
    system2(f'cp "{so_path}" tmpdeb/usr/lib/supportdesk/')
    system2(f'ln -sf "{so_basename}" tmpdeb/usr/lib/supportdesk/libdrmtap.so.0')


def _max_glibc_minor(path):
    best = 0
    with open(path, 'rb') as f:
        tail = b''
        while True:
            chunk = f.read(1 << 20)
            if not chunk:
                return best
            for m in re.finditer(rb'GLIBC_2\.(\d+)', tail + chunk):
                best = max(best, int(m.group(1)))
            tail = chunk[-16:]


def measured_glibc_floor():
    paths = [p for p in glob.glob('tmpdeb/usr/lib/supportdesk/libdrmtap.so.0.*')
             + glob.glob('tmpdeb/usr/share/supportdesk/lib/libsupportdesk.so')
             + glob.glob('tmpdeb/usr/share/supportdesk/supportdesk')
             if os.path.isfile(p) and not os.path.islink(p)]
    minor = max((_max_glibc_minor(p) for p in paths), default=0)
    if not minor:
        raise Exception(
            f'could not measure a GLIBC_2.x floor from any staged object ({paths or "none found"}); '
            'refusing to ship the unattended-wayland variant with an undeclared libc6 floor, which '
            'is what lets it install on a host where libdrmtap can never load')
    return f'2.{minor}'


def retarget_control_to_drm_variant():
    path = '../res/DEBIAN/control'
    floor = measured_glibc_floor()
    print(f'[drm] {DRM_PACKAGE_NAME} libc6 floor measured at {floor}')
    with open(path) as f:
        lines = f.readlines()
    out = []
    for line in lines:
        if line.startswith('Package: supportdesk'):
            out.append(f'Package: {DRM_PACKAGE_NAME}\n')
            out.append('Conflicts: supportdesk\nReplaces: supportdesk\nProvides: supportdesk\n')
        elif line.startswith('Depends:'):
            out.append(line.rstrip('\n') + ', libdrm2 (>= 2.4.101), libegl1, libgles2, '
                       f'libc6 (>= {floor})\n')
        else:
            out.append(line)
    body = ''.join(out)
    if f'Package: {DRM_PACKAGE_NAME}\n' not in body or 'libegl1' not in body:
        raise Exception(f'could not retarget {path} to the drm variant; upstream control layout changed')
    with open(path, 'w') as f:
        f.write(body)


def build_flutter_deb(version, features):
    if not skip_cargo:
        system2(f'cargo build --locked --features {features} --lib --release')
        ffi_bindgen_function_refactor()
    os.chdir('flutter')
    system2('flutter build linux --release')
    system2('mkdir -p tmpdeb/usr/bin/')
    system2('mkdir -p tmpdeb/usr/share/supportdesk')
    system2('mkdir -p tmpdeb/usr/share/supportdesk/files/systemd/')
    system2('mkdir -p tmpdeb/usr/share/icons/hicolor/256x256/apps/')
    system2('mkdir -p tmpdeb/usr/share/icons/hicolor/scalable/apps/')
    system2('mkdir -p tmpdeb/usr/share/applications/')
    system2('mkdir -p tmpdeb/usr/share/polkit-1/actions')
    system2('rm tmpdeb/usr/bin/supportdesk || true')
    system2(
        f'cp -r {flutter_build_dir}/* tmpdeb/usr/share/supportdesk/')
    system2(
        'cp ../res/supportdesk.service tmpdeb/usr/share/supportdesk/files/systemd/')
    system2(
        'cp ../res/128x128@2x.png tmpdeb/usr/share/icons/hicolor/256x256/apps/supportdesk.png')
    system2(
        'cp ../res/scalable.svg tmpdeb/usr/share/icons/hicolor/scalable/apps/supportdesk.svg')
    system2(
        'cp ../res/supportdesk.desktop tmpdeb/usr/share/applications/supportdesk.desktop')
    system2(
        'cp ../res/supportdesk-link.desktop tmpdeb/usr/share/applications/supportdesk-link.desktop')
    system2(
        "echo \"#!/bin/sh\" >> tmpdeb/usr/share/supportdesk/files/polkit && chmod a+x tmpdeb/usr/share/supportdesk/files/polkit")
    ships_so = 'drm' in features.split(',')
    if ships_so:
        assert_staged_binary_is_drm()
        stage_libdrmtap_into_deb(build_libdrmtap_so())

    system2('mkdir -p tmpdeb/DEBIAN')
    generate_control_file(version)
    if ships_so:
        retarget_control_to_drm_variant()
    system2('cp -a ../res/DEBIAN/* tmpdeb/DEBIAN/')
    md5_file_folder("tmpdeb/")
    system2('dpkg-deb -b tmpdeb supportdesk.deb;')

    system2('/bin/rm -rf tmpdeb/')
    system2('/bin/rm -rf ../res/DEBIAN/control')
    os.rename('supportdesk.deb', '../supportdesk-%s.deb' % version)
    if ships_so:
        os.rename('../supportdesk-%s.deb' % version, f'../{DRM_PACKAGE_NAME}-{version}.deb')
    os.chdir("..")


DRMTAP_DLOPEN_MARKER = b'/usr/lib/supportdesk/libdrmtap.so.0'
DRMTAP_WAKE_MARKER = b'enable-drm-display-wake'


def _carries_drmtap_marker(path, marker=DRMTAP_DLOPEN_MARKER):
    with open(path, 'rb') as f:
        tail = b''
        while True:
            chunk = f.read(1 << 20)
            if not chunk:
                return False
            if marker in tail + chunk:
                return True
            tail = chunk[-(len(marker) - 1):]


def assert_staged_binary_is_drm():
    """The staged BINARY must really be a drm build before it is named the unattended-wayland
    variant. That package conflicts with and replaces the stock one, so shipping a stock binary
    under that name produces something that can never capture and cannot be installed alongside
    what it replaced. The marker is the absolute dlopen path from drmtap_dl.rs, present only when
    the feature is compiled in -- assert what was produced, not what was asked for.

    Called from BOTH packaging paths. It used to guard only one of them, and `--skip-cargo` (which
    is how CI packages) reaches the other, where nothing had rebuilt the binary at all.
    """
    binaries = [p for p in glob.glob('tmpdeb/usr/share/supportdesk/lib/libsupportdesk.so')
                + glob.glob('tmpdeb/usr/share/supportdesk/supportdesk') if os.path.isfile(p)]
    if not any(_carries_drmtap_marker(p) for p in binaries):
        raise Exception(
            f'--drm was requested but the staged bundle does not look like a drm build (no '
            f'{DRMTAP_DLOPEN_MARKER.decode()} dlopen path in {binaries or "any staged binary"}); '
            'refusing to package it as the unattended-wayland variant, which conflicts with and '
            'replaces the stock package but could never capture')
    if not any(_carries_drmtap_marker(p, DRMTAP_WAKE_MARKER) for p in binaries):
        raise Exception(
            f'--drm was requested but the staged binary has no {DRMTAP_WAKE_MARKER.decode()} '
            f'marker in {binaries or "any staged binary"}, so it was built without `drm-wake`; '
            'refusing to package it as the unattended-wayland variant, which is named and '
            'documented as the build that can wake an idle-disabled display. If this fired under '
            '--skip-cargo, the cargo line that produced the bundle is missing the feature: '
            '--features ...,drm,drm-wake')


def build_deb_from_folder(version, binary_folder, want_drm=False):
    os.chdir('flutter')
    system2('mkdir -p tmpdeb/usr/bin/')
    system2('mkdir -p tmpdeb/usr/share/supportdesk')
    system2('mkdir -p tmpdeb/usr/share/supportdesk/files/systemd/')
    system2('mkdir -p tmpdeb/usr/share/icons/hicolor/256x256/apps/')
    system2('mkdir -p tmpdeb/usr/share/icons/hicolor/scalable/apps/')
    system2('mkdir -p tmpdeb/usr/share/applications/')
    system2('mkdir -p tmpdeb/usr/share/polkit-1/actions')
    system2('rm tmpdeb/usr/bin/supportdesk || true')
    system2(
        f'cp -r ../{binary_folder}/* tmpdeb/usr/share/supportdesk/')
    system2(
        'cp ../res/supportdesk.service tmpdeb/usr/share/supportdesk/files/systemd/')
    system2(
        'cp ../res/128x128@2x.png tmpdeb/usr/share/icons/hicolor/256x256/apps/supportdesk.png')
    system2(
        'cp ../res/scalable.svg tmpdeb/usr/share/icons/hicolor/scalable/apps/supportdesk.svg')
    system2(
        'cp ../res/supportdesk.desktop tmpdeb/usr/share/applications/supportdesk.desktop')
    system2(
        'cp ../res/supportdesk-link.desktop tmpdeb/usr/share/applications/supportdesk-link.desktop')
    system2(
        "echo \"#!/bin/sh\" >> tmpdeb/usr/share/supportdesk/files/polkit && chmod a+x tmpdeb/usr/share/supportdesk/files/polkit")
    bundled_glob = glob.glob('tmpdeb/usr/share/supportdesk/libdrmtap.so.0.*')
    bundle_carries_so = any(os.path.isfile(p) and not os.path.islink(p) for p in bundled_glob)
    if bundle_carries_so and not want_drm:
        raise Exception(
            'the staged bundle carries libdrmtap.so.0.* but --drm was not passed; refusing '
            'to silently ship the consent-bypass unattended-wayland variant (pass --drm to '
            'build it deliberately)')
    if want_drm:
        assert_staged_binary_is_drm()
        if bundle_carries_so:
            so = _single_real_so(bundled_glob, 'the staged --drm bundle')
            _assert_so_has_egl(so)
            _assert_so_has_no_helper(so)
            stage_libdrmtap_into_deb(so)
            system2(f'rm -f "{so}"')
            system2('rm -f tmpdeb/usr/share/supportdesk/libdrmtap.so tmpdeb/usr/share/supportdesk/libdrmtap.so.0')
        else:
            stage_libdrmtap_into_deb(build_libdrmtap_so())

    system2('mkdir -p tmpdeb/DEBIAN')
    generate_control_file(version)
    if want_drm:
        retarget_control_to_drm_variant()
    system2('cp -a ../res/DEBIAN/* tmpdeb/DEBIAN/')
    md5_file_folder("tmpdeb/")
    system2('dpkg-deb -b tmpdeb supportdesk.deb;')

    system2('/bin/rm -rf tmpdeb/')
    system2('/bin/rm -rf ../res/DEBIAN/control')
    os.rename('supportdesk.deb', '../supportdesk-%s.deb' % version)
    if want_drm:
        os.rename('../supportdesk-%s.deb' % version, f'../{DRM_PACKAGE_NAME}-{version}.deb')
    os.chdir("..")


def build_flutter_dmg(version, features):
    if not skip_cargo:
        system2(
            f'MACOSX_DEPLOYMENT_TARGET=10.14 cargo build --locked --features {features} --release')
    system2(
        "cp target/release/liblibsupportdesk.dylib target/release/libsupportdesk.dylib")
    os.chdir('flutter')
    mac_arch = 'arm64' if platform.machine().lower() in ('arm64', 'aarch64') else 'x86_64'
    system2(
        f'FLUTTER_XCODE_ARCHS={mac_arch} FLUTTER_XCODE_ONLY_ACTIVE_ARCH=YES flutter build macos --release')
    system2('cp -rf ../target/release/service ./build/macos/Build/Products/Release/SupportDesk.app/Contents/MacOS/')
    '''
    system2(
        "create-dmg --volname \"SupportDesk Installer\" --window-pos 200 120 --window-size 800 400 --icon-size 100 --app-drop-link 600 185 --icon SupportDesk.app 200 190 --hide-extension SupportDesk.app supportdesk.dmg ./build/macos/Build/Products/Release/SupportDesk.app")
    os.rename("supportdesk.dmg", f"../supportdesk-{version}.dmg")
    '''
    os.chdir("..")


def build_flutter_arch_manjaro(version, features):
    if not skip_cargo:
        system2(f'cargo build --locked --features {features} --lib --release')
    ffi_bindgen_function_refactor()
    os.chdir('flutter')
    system2('flutter build linux --release')
    system2(f'strip {flutter_build_dir}/lib/libsupportdesk.so')
    os.chdir('../res')
    system2('HBB=`pwd`/.. FLUTTER=1 makepkg -f')


def build_flutter_windows(version, features, skip_portable_pack):
    if not skip_cargo:
        system2(f'cargo build --locked --features {features} --lib --release')
        if not os.path.exists("target/release/libsupportdesk.dll"):
            print("cargo build failed, please check rust source code.")
            exit(-1)
    os.chdir('flutter')
    system2('flutter build windows --release')
    os.chdir('..')
    shutil.copy2('target/release/deps/dylib_virtual_display.dll',
                 flutter_build_dir_2)
    if skip_portable_pack:
        return
    os.chdir('libs/portable')
    system2('pip3 install -r requirements.txt')
    system2(
        f'python3 ./generate.py -f ../../{flutter_build_dir_2} -o . -e ../../{flutter_build_dir_2}/supportdesk.exe')
    os.chdir('../..')
    if os.path.exists('./supportdesk_portable.exe'):
        os.replace('./target/release/supportdesk-portable-packer.exe',
                   './supportdesk_portable.exe')
    else:
        os.rename('./target/release/supportdesk-portable-packer.exe',
                  './supportdesk_portable.exe')
    print(
        f'output location: {os.path.abspath(os.curdir)}/supportdesk_portable.exe')
    os.rename('./supportdesk_portable.exe', f'./supportdesk-{version}-install.exe')
    print(
        f'output location: {os.path.abspath(os.curdir)}/supportdesk-{version}-install.exe')


def main():
    global skip_cargo
    parser = make_parser()
    args = parser.parse_args()

    if args.print_features:
        with contextlib.redirect_stdout(sys.stderr):
            feats = ','.join(get_features(args))
        print(feats)
        return

    if os.path.exists(exe_path):
        os.unlink(exe_path)
    if os.path.isfile('/usr/bin/pacman'):
        system2('git checkout src/ui/common.tis')
    version = get_version()
    features = ','.join(get_features(args))
    flutter = args.flutter
    if not flutter:
        system2('python3 res/inline-sciter.py')
    print(args.skip_cargo)
    if args.skip_cargo:
        skip_cargo = True
    portable = args.portable
    package = args.package
    if package:
        build_deb_from_folder(version, package, args.drm)
        return
    res_dir = 'resources'
    external_resources(flutter, args, res_dir)
    if windows:
        os.chdir('libs/virtual_display/dylib')
        system2('cargo build --locked --release')
        os.chdir('../../..')

        if flutter:
            build_flutter_windows(version, features, args.skip_portable_pack)
            return
        system2('cargo build --locked --release --features ' + features)
        system2('mv target/release/supportdesk.exe target/release/SupportDesk.exe')
        pa = os.environ.get('P')
        if pa:
            system2(
                f'signtool sign /a /v /p {pa} /debug /f .\\cert.pfx /t http://timestamp.digicert.com  '
                'target\\release\\supportdesk.exe')
        else:
            print('Not signed')
        os.makedirs(res_dir, exist_ok=True)
        system2(
            f'cp -rf target/release/SupportDesk.exe {res_dir}')
        os.chdir('libs/portable')
        system2('pip3 install -r requirements.txt')
        system2(
            f'python3 ./generate.py -f ../../{res_dir} -o . -e ../../{res_dir}/supportdesk-{version}-win7-install.exe')
        system2(f'mv ../../{res_dir}/supportdesk-{version}-win7-install.exe ../..')
    elif os.path.isfile('/usr/bin/pacman'):
        system2("sed -i 's/pkgver=.*/pkgver=%s/g' res/PKGBUILD" % version)
        if flutter:
            build_flutter_arch_manjaro(version, features)
        else:
            system2('cargo build --locked --release --features ' + features)
            system2('git checkout src/ui/common.tis')
            system2('strip target/release/supportdesk')
            system2('ln -s res/pacman_install && ln -s res/PKGBUILD')
            system2('HBB=`pwd` makepkg -f')
        system2('mv supportdesk-%s-0-x86_64.pkg.tar.zst supportdesk-%s-manjaro-arch.pkg.tar.zst' % (
            version, version))
    elif os.path.isfile('/usr/bin/yum'):
        system2('cargo build --locked --release --features ' + features)
        system2('strip target/release/supportdesk')
        system2(
            "sed -i 's/Version:    .*/Version:    %s/g' res/rpm.spec" % version)
        system2('HBB=`pwd` rpmbuild -ba res/rpm.spec')
        system2(
            'mv $HOME/rpmbuild/RPMS/x86_64/supportdesk-%s-0.x86_64.rpm ./supportdesk-%s-fedora28-centos8.rpm' % (
                version, version))
    elif os.path.isfile('/usr/bin/zypper'):
        system2('cargo build --locked --release --features ' + features)
        system2('strip target/release/supportdesk')
        system2(
            "sed -i 's/Version:    .*/Version:    %s/g' res/rpm-suse.spec" % version)
        system2('HBB=`pwd` rpmbuild -ba res/rpm-suse.spec')
        system2(
            'mv $HOME/rpmbuild/RPMS/x86_64/supportdesk-%s-0.x86_64.rpm ./supportdesk-%s-suse.rpm' % (
                version, version))
    else:
        if flutter:
            if osx:
                build_flutter_dmg(version, features)
                pass
            else:
                build_flutter_deb(version, features)
        else:
            system2('cargo --locked bundle --release --features ' + features)
            if osx:
                system2(
                    'strip target/release/bundle/osx/SupportDesk.app/Contents/MacOS/supportdesk')
                system2(
                    'cp libsciter.dylib target/release/bundle/osx/SupportDesk.app/Contents/MacOS/')
                system2('/bin/rm -rf *.dmg')
                pa = os.environ.get('P')
                if pa:
                    system2('''
    # buggy: rcodesign sign ... path/*, have to sign one by one
    # install rcodesign via cargo install apple-codesign
    #rcodesign sign --p12-file ~/.p12/supportdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./target/release/bundle/osx/SupportDesk.app/Contents/MacOS/supportdesk
    #rcodesign sign --p12-file ~/.p12/supportdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./target/release/bundle/osx/SupportDesk.app/Contents/MacOS/libsciter.dylib
    #rcodesign sign --p12-file ~/.p12/supportdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./target/release/bundle/osx/SupportDesk.app
    # goto "Keychain Access" -> "My Certificates" for below id which starts with "Developer ID Application:"
    codesign -s "Developer ID Application: {0}" --force --options runtime  ./target/release/bundle/osx/SupportDesk.app/Contents/MacOS/*
    codesign -s "Developer ID Application: {0}" --force --options runtime  ./target/release/bundle/osx/SupportDesk.app
    '''.format(pa))
                system2(
                    'create-dmg "SupportDesk %s.dmg" "target/release/bundle/osx/SupportDesk.app"' % version)
                os.rename('SupportDesk %s.dmg' %
                          version, 'supportdesk-%s.dmg' % version)
                if pa:
                    system2('''
    # https://pyoxidizer.readthedocs.io/en/apple-codesign-0.14.0/apple_codesign.html
    # https://pyoxidizer.readthedocs.io/en/stable/tugger_code_signing.html
    # https://developer.apple.com/developer-id/
    # goto xcode and login with apple id, manager certificates (Developer ID Application and/or Developer ID Installer) online there (only download and double click (install) cer file can not export p12 because no private key)
    #rcodesign sign --p12-file ~/.p12/supportdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./supportdesk-{1}.dmg
    codesign -s "Developer ID Application: {0}" --force --options runtime ./supportdesk-{1}.dmg
    # https://appstoreconnect.apple.com/access/api
    # https://gregoryszorc.com/docs/apple-codesign/stable/apple_codesign_getting_started.html#apple-codesign-app-store-connect-api-key
    # p8 file is generated when you generate api key (can download only once)
    rcodesign notary-submit --api-key-path ../.p12/api-key.json  --staple supportdesk-{1}.dmg
    # verify:  spctl -a -t exec -v /Applications/SupportDesk.app
    '''.format(pa, version))
                else:
                    print('Not signed')
            else:
                system2(
                    'mv target/release/bundle/deb/supportdesk*.deb ./supportdesk.deb')
                system2('dpkg-deb -R supportdesk.deb tmpdeb')
                system2('mkdir -p tmpdeb/usr/share/supportdesk/files/systemd/')
                system2('mkdir -p tmpdeb/usr/share/icons/hicolor/256x256/apps/')
                system2('mkdir -p tmpdeb/usr/share/icons/hicolor/scalable/apps/')
                system2(
                    'cp res/supportdesk.service tmpdeb/usr/share/supportdesk/files/systemd/')
                system2(
                    'cp res/128x128@2x.png tmpdeb/usr/share/icons/hicolor/256x256/apps/supportdesk.png')
                system2(
                    'cp res/scalable.svg tmpdeb/usr/share/icons/hicolor/scalable/apps/supportdesk.svg')
                system2(
                    'cp res/supportdesk.desktop tmpdeb/usr/share/applications/supportdesk.desktop')
                system2(
                    'cp res/supportdesk-link.desktop tmpdeb/usr/share/applications/supportdesk-link.desktop')
                os.system('cp -a DEBIAN/* tmpdeb/DEBIAN/')
                system2('strip tmpdeb/usr/bin/supportdesk')
                system2('mkdir -p tmpdeb/usr/share/supportdesk')
                system2('mv tmpdeb/usr/bin/supportdesk tmpdeb/usr/share/supportdesk/')
                system2('cp libsciter-gtk.so tmpdeb/usr/share/supportdesk/')
                md5_file_folder("tmpdeb/")
                system2('dpkg-deb -b tmpdeb supportdesk.deb; /bin/rm -rf tmpdeb/')
                os.rename('supportdesk.deb', 'supportdesk-%s.deb' % version)


def md5_file(fn):
    md5 = hashlib.md5(open('tmpdeb/' + fn, 'rb').read()).hexdigest()
    system2('echo "%s  /%s" >> tmpdeb/DEBIAN/md5sums' % (md5, fn))

def md5_file_folder(base_dir):
    base_path = Path(base_dir)
    for file in base_path.rglob('*'):
        if file.is_file() and 'DEBIAN' not in file.parts:
            relative_path = file.relative_to(base_path)
            md5_file(str(relative_path))


if __name__ == "__main__":
    main()
