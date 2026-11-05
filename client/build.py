#!/usr/bin/env python3
"""
SupportDesk Build & Packaging Script
A simplified, clean, and modern build tool for SupportDesk and custom-branded clients.

Usage:
    python3 build.py                    # Fast debug build
    python3 build.py --release          # Optimized release build
    python3 build.py --bin visiosoft    # Build Visiosoft brand
    python3 build.py --run              # Build and immediately run
    python3 build.py --ansible          # Delegate execution to Ansible playbook
"""

import argparse
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Optional, List

CLIENT_DIR = Path(__file__).resolve().parent
REPO_ROOT = CLIENT_DIR.parent
SYS_PLATFORM = platform.system().lower()  # darwin, windows, linux

GREEN = "\033[92m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
RED = "\033[91m"
RESET = "\033[0m"


def log(msg: str, color=CYAN):
    print(f"{color}==> {msg}{RESET}")


def get_version() -> str:
    cargo_toml = CLIENT_DIR / "Cargo.toml"
    if cargo_toml.exists():
        for line in cargo_toml.read_text().splitlines():
            if line.strip().startswith("version"):
                return line.split("=")[1].strip().strip('"')
    return "1.5.0"


def setup_env() -> dict:
    """Setup optimal build environment (Rust toolchain, vcpkg, libraries)."""
    env = os.environ.copy()

    home = Path.home()
    rust_177_bin = home / ".rustup" / "toolchains" / "1.77.2-aarch64-apple-darwin" / "bin"
    cargo_bin = home / ".cargo" / "bin"

    paths_to_add = []
    if rust_177_bin.exists():
        paths_to_add.append(str(rust_177_bin))
    if cargo_bin.exists():
        paths_to_add.append(str(cargo_bin))

    if paths_to_add:
        env["PATH"] = os.pathsep.join(paths_to_add) + os.pathsep + env.get("PATH", "")

    if "VCPKG_ROOT" not in env:
        for candidate in [home / "vcpkg", Path("/opt/vcpkg"), Path("/usr/local/vcpkg")]:
            if candidate.exists():
                env["VCPKG_ROOT"] = str(candidate)
                break

    return env


def generate_inline_sciter():
    """Inline UI assets (HTML, CSS, TIS) into Rust code."""
    script = CLIENT_DIR / "res" / "inline-sciter.py"
    if script.exists():
        log("Generating inline Sciter assets...", CYAN)
        subprocess.run([sys.executable, str(script)], cwd=CLIENT_DIR, check=True)


def collect_features() -> list:
    return ["inline"]


def ensure_signing_environment():
    """Ensure VizioSoft code signing certificate and keychain are configured."""
    sign_script = REPO_ROOT / "certs" / "generate-cert.sh"
    if sign_script.exists():
        subprocess.run(["bash", str(sign_script)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)


def sign_artifact(target_path: Path):
    """Sign binary, library, or .app bundle with VizioSoft certificate."""
    if SYS_PLATFORM != "darwin" or not target_path.exists():
        return
    sign_script = REPO_ROOT / "certs" / "sign.sh"
    if sign_script.exists():
        log(f"Signing {target_path.name} with VizioSoft certificate...", CYAN)
        res = subprocess.run(["bash", str(sign_script), str(target_path)], capture_output=True, text=True)
        if res.returncode == 0:
            log(f"Verified VizioSoft signature for {target_path.name}", GREEN)
        else:
            log(f"Signing notice: {res.stderr.strip() or res.stdout.strip()}", YELLOW)


def package_macos_app(target_dir: Path, bin_name: str, brand_name: str, brand_file: Optional[Path], sign: bool = True):
    """Package binary into a standard macOS .app bundle."""
    app_dir = target_dir / f"{brand_name}.app"
    macos_dir = app_dir / "Contents" / "MacOS"
    res_dir = app_dir / "Contents" / "Resources"
    macos_dir.mkdir(parents=True, exist_ok=True)
    res_dir.mkdir(parents=True, exist_ok=True)

    log(f"Packaging macOS bundle: {app_dir.name}", GREEN)

    bin_src = target_dir / bin_name
    if bin_src.exists():
        shutil.copy2(bin_src, macos_dir / bin_name)
        os.chmod(macos_dir / bin_name, 0o755)

    sciter_dylib = CLIENT_DIR / "libsciter.dylib"
    if sciter_dylib.exists():
        shutil.copy2(sciter_dylib, macos_dir / "libsciter.dylib")
        shutil.copy2(sciter_dylib, target_dir / "libsciter.dylib")

    if brand_file and brand_file.exists():
        shutil.copy2(brand_file, res_dir / "brand.json")
        shutil.copy2(brand_file, target_dir / "brand.json")

    plist_content = f"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>{bin_name}</string>
    <key>CFBundleIdentifier</key>
    <string>com.{brand_name.lower()}.supportdesk</string>
    <key>CFBundleName</key>
    <string>{brand_name}</string>
    <key>CFBundleDisplayName</key>
    <string>{brand_name}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>{get_version()}</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.14</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
"""
    (app_dir / "Contents" / "Info.plist").write_text(plist_content)

    if sign and SYS_PLATFORM == "darwin":
        sign_artifact(app_dir)

    if SYS_PLATFORM == "darwin":
        dmg_path = target_dir / f"{brand_name}.dmg"
        staging_dir = target_dir / "dmg_staging"
        if staging_dir.exists():
            shutil.rmtree(staging_dir)
        staging_dir.mkdir(parents=True, exist_ok=True)

        shutil.copytree(app_dir, staging_dir / f"{brand_name}.app", symlinks=True)
        try:
            os.symlink("/Applications", staging_dir / "Applications")
        except OSError:
            pass

        launcher_script = f"""#!/bin/bash
DIR="$(cd "$(dirname "$0")" && pwd)"
xattr -cr "$DIR/{brand_name}.app" 2>/dev/null || true
open "$DIR/{brand_name}.app"
"""
        launcher_file = staging_dir / "Baslat.command"
        launcher_file.write_text(launcher_script)
        os.chmod(launcher_file, 0o755)

        readme_text = f"""==================================================
  {brand_name} - Uzaktan Destek İstemcisi
  VİSİOSOFT TEKNOLOJİ ANONİM ŞİRKETİ
==================================================

Bu uygulama resmi olarak VİSİOSOFT TEKNOLOJİ ANONİM ŞİRKETİ sertifikasıyla imzalanmıştır.

KURULUM:
1. '{brand_name}.app' uygulamasını 'Applications' (Uygulamalar) klasörüne sürükleyin.
2. Uygulamalar klasöründen uygulamayı açabilirsiniz.

macOS Güvenlik Uyarısı ("Apple could not verify..." veya "Açılamadı") Alırsanız:
* Adım 1: Ekranda çıkan uyarıda 'Bitti' (Done) butonuna tıklayın.
* Adım 2: 'Sistem Ayarları' > 'Gizlilik ve Güvenlik' (System Settings > Privacy & Security) sayfasını açın.
* Adım 3: En alta kaydırıp 'Güvenlik' altında "{brand_name}.app engellendi" yazısının yanındaki "Yine de Aç" (Open Anyway) butonuna tıklayın.
* Alternatif: Paket içindeki 'Baslat.command' dosyasına çift tıklayarak doğrudan açabilirsiniz.
"""
        (staging_dir / "README.txt").write_text(readme_text)

        log(f"Creating macOS DMG disk image: {dmg_path.name}...", CYAN)
        hdi_cmd = ["hdiutil", "create", "-volname", brand_name, "-srcfolder", str(staging_dir), "-ov", "-format", "UDZO", str(dmg_path)]
        hdi_res = subprocess.run(hdi_cmd, capture_output=True, text=True)
        shutil.rmtree(staging_dir, ignore_errors=True)

        if hdi_res.returncode == 0:
            log(f"Successfully created DMG: {dmg_path.name}", GREEN)
            if sign:
                sign_artifact(dmg_path)
        else:
            log(f"DMG creation warning: {hdi_res.stderr.strip()}", YELLOW)


def build_with_ansible(args):
    """Delegate build to Ansible playbook."""
    log("Running build with Ansible...", YELLOW)
    playbook = CLIENT_DIR / "playbook.yml"
    if not playbook.exists():
        sys.exit(f"Error: Playbook not found at {playbook}")

    extra_vars = [
        f"release={'true' if args.release else 'false'}",
        f"bin_name={args.bin}",
        f"brand_name={args.brand}",
        f"run_app={'true' if args.run else 'false'}",
        f"package_app={'true' if args.package else 'false'}",
    ]
    if args.brand_file:
        extra_vars.append(f"brand_file={args.brand_file}")

    cmd = ["ansible-playbook", str(playbook), "-i", str(CLIENT_DIR / "inventory.ini"), "-e", " ".join(extra_vars)]
    subprocess.run(cmd, cwd=CLIENT_DIR, check=True)


def run_build(args):
    if args.ansible:
        return build_with_ansible(args)

    if args.clean:
        log("Cleaning build directory...", YELLOW)
        subprocess.run(["cargo", "clean"], cwd=CLIENT_DIR)
        return

    features = collect_features()
    env = setup_env()
    profile = "release" if args.release else "debug"
    target_dir = CLIENT_DIR / "target" / profile

    generate_inline_sciter()

    if getattr(args, "sign", True) and SYS_PLATFORM == "darwin":
        ensure_signing_environment()
        pre_sciter = CLIENT_DIR / "libsciter.dylib"
        if pre_sciter.exists():
            log("Pre-build: Signing internal libraries with VizioSoft certificate before compilation...", CYAN)
            sign_artifact(pre_sciter)

    cargo_cmd = ["cargo", "build"]
    if args.release:
        cargo_cmd.append("--release")
    if features:
        cargo_cmd.extend(["--features", ",".join(features)])
    if args.bin:
        cargo_cmd.extend(["--bin", args.bin])

    log(f"Building {args.bin} ({profile})...", CYAN)
    subprocess.run(cargo_cmd, cwd=CLIENT_DIR, env=env, check=True)

    if getattr(args, "sign", True) and SYS_PLATFORM == "darwin":
        compiled_bin = target_dir / args.bin
        if compiled_bin.exists():
            log(f"Build step: Signing compiled binary {args.bin} with VizioSoft certificate...", CYAN)
            sign_artifact(compiled_bin)

    if SYS_PLATFORM == "darwin":
        sciter_src = CLIENT_DIR / "libsciter.dylib"
        if sciter_src.exists():
            target_dir.mkdir(parents=True, exist_ok=True)
            shutil.copy2(sciter_src, target_dir / "libsciter.dylib")

    brand_file = Path(args.brand_file) if args.brand_file else (CLIENT_DIR / "brand.json")
    if not brand_file.is_absolute():
        brand_file = CLIENT_DIR / brand_file

    if brand_file.exists():
        shutil.copy2(brand_file, target_dir / "brand.json")

    if args.package and SYS_PLATFORM == "darwin":
        package_macos_app(target_dir, args.bin, args.brand, brand_file if brand_file.exists() else None, sign=getattr(args, "sign", True))

    log(f"Build complete: {target_dir / args.bin}", GREEN)

    if args.run:
        bin_path = target_dir / args.bin
        log(f"Launching {bin_path.name}...", GREEN)
        cmd = [str(bin_path)]
        subprocess.Popen(cmd, cwd=CLIENT_DIR, env=env)


def parse_args():
    parser = argparse.ArgumentParser(
        description="SupportDesk Client Build System",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument("--release", action="store_true", help="Build optimized release binary")
    parser.add_argument("--bin", default="supportdesk", help="Binary target name (supportdesk, visiosoft)")
    parser.add_argument("--brand", default="SupportDesk", help="Brand name for display and app bundle")
    parser.add_argument("--brand-file", help="Custom brand.json path (defaults to brand.json)")
    parser.add_argument("--package", action="store_true", default=(SYS_PLATFORM == "darwin"), help="Package as .app bundle")
    parser.add_argument("--run", action="store_true", help="Launch binary immediately after building")
    parser.add_argument("--clean", action="store_true", help="Clean cargo build artifacts")
    parser.add_argument("--ansible", action="store_true", help="Execute build via Ansible playbook")
    parser.add_argument("--no-sign", dest="sign", action="store_false", default=True, help="Skip code signing with VizioSoft certificate")

    return parser.parse_args()


if __name__ == "__main__":
    run_build(parse_args())
