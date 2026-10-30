#!/bin/bash





prepare_flutter() {
	VERSION="${1}"
	FLUTTER_DIR="${2}"

	if [ ! -f "${FLUTTER_DIR}/bin/flutter" ]; then
		git clone https://github.com/flutter/flutter "${FLUTTER_DIR}"
	fi

	pushd "${FLUTTER_DIR}"

	git restore .
	git checkout "${VERSION}"


	if dpkg --compare-versions "${VERSION}" ge "3.24.4"; then
		git apply "${ROOTDIR}/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff"
	fi

	flutter config --no-analytics

	popd
}


set -x


ROOTDIR="${PWD}"


VERNAME="${1}"
VERCODE="${2}"
ANDROID_ABI="${3}"
BUILDSTEP="${4}"

if [ -z "${VERNAME}" ] || [ -z "${VERCODE}" ] || [ -z "${ANDROID_ABI}" ] ||
	[ -z "${BUILDSTEP}" ]; then
	echo "ERROR: Command-line arguments are all required to be non-empty!" >&2
	exit 1
fi


case "${ANDROID_ABI}" in
arm64-v8a)
	FLUTTER_TARGET=android-arm64
	NDK_TARGET=aarch64-linux-android
	RUST_TARGET=aarch64-linux-android
	SUPPORTDESK_FEATURES='flutter,hwcodec'
	;;
armeabi-v7a)
	FLUTTER_TARGET=android-arm
	NDK_TARGET=arm-linux-androideabi
	RUST_TARGET=armv7-linux-androideabi
	SUPPORTDESK_FEATURES='flutter,hwcodec'
	;;
x86_64)
	FLUTTER_TARGET=android-x64
	NDK_TARGET=x86_64-linux-android
	RUST_TARGET=x86_64-linux-android
	SUPPORTDESK_FEATURES='flutter'
	;;
x86)
	FLUTTER_TARGET=android-x86
	NDK_TARGET=i686-linux-android
	RUST_TARGET=i686-linux-android
	SUPPORTDESK_FEATURES='flutter'
	;;
*)
	echo "ERROR: Unknown Android ABI '${ANDROID_ABI}'!" >&2
	exit 1
	;;
esac


if [ ! -d "${ANDROID_SDK_ROOT}" ] || ! command -v sdkmanager 1>/dev/null; then
	echo "ERROR: Can not find Android SDK!" >&2
	exit 1
fi


export PATH="${PATH}:${HOME}/flutter/bin:${HOME}/depot_tools"

export VCPKG_ROOT="${HOME}/vcpkg"



case "${BUILDSTEP}" in
prebuild)


	CARGO_NDK_VERSION="$(yq -r \
		.env.CARGO_NDK_VERSION \
		.github/workflows/flutter-build.yml)"


	FLUTTER_VERSION="$(yq -r \
		.env.ANDROID_FLUTTER_VERSION \
		.github/workflows/flutter-build.yml)"

	if [ -z "${FLUTTER_VERSION}" ]; then
		FLUTTER_VERSION="$(yq -r \
			.env.FLUTTER_VERSION \
			.github/workflows/flutter-build.yml)"
	fi


	CARGO_EXPAND_VERSION="$(yq -r \
		.env.CARGO_EXPAND_VERSION \
		.github/workflows/bridge.yml)"

	FLUTTER_BRIDGE_VERSION="$(yq -r \
		'.jobs.generate_bridge.strategy.matrix.job[] | select(.["artifact-name"] == "bridge-artifact") | .["flutter-version"]' \
		.github/workflows/bridge.yml)"

	FLUTTER_RUST_BRIDGE_VERSION="$(yq -r \
		.env.FLUTTER_RUST_BRIDGE_VERSION \
		.github/workflows/bridge.yml)"

	NDK_VERSION="$(yq -r \
		.env.NDK_VERSION \
		.github/workflows/flutter-build.yml)"

	RUST_VERSION="$(yq -r \
		.env.RUST_VERSION \
		.github/workflows/flutter-build.yml)"

	VCPKG_COMMIT_ID="$(yq -r \
		.env.VCPKG_COMMIT_ID \
		.github/workflows/flutter-build.yml)"

	if [ -z "${CARGO_NDK_VERSION}" ] || [ -z "${FLUTTER_VERSION}" ] ||
		[ -z "${FLUTTER_BRIDGE_VERSION}" ] ||
		[ -z "${FLUTTER_RUST_BRIDGE_VERSION}" ] ||
		[ -z "${NDK_VERSION}" ] || [ -z "${RUST_VERSION}" ] ||
		[ -z "${VCPKG_COMMIT_ID}" ]; then
		echo "ERROR: Can not identify all required versions!" >&2
		exit 1
	fi

	NDK_VERSION="$(curl https://gitlab.com/fdroid/android-sdk-transparency-log/-/raw/master/signed/checksums.json |
		jq -r ".\"https://dl.google.com/android/repository/android-ndk-${NDK_VERSION}-linux.zip\"[0].\"source.properties\"" |
		sed -n -E 's/.*Pkg.Revision = ([0-9.]+).*/\1/p')"

	if [ -z "${NDK_VERSION}" ]; then
		echo "ERROR: Can not map Android NDK codename to revision!" >&2
		exit 1
	fi

	export ANDROID_NDK_HOME="${ANDROID_SDK_ROOT}/ndk/${NDK_VERSION}"
	export ANDROID_NDK_ROOT="${ANDROID_SDK_ROOT}/ndk/${NDK_VERSION}"


	set -e


	if [ ! -d "${ANDROID_NDK_ROOT}" ]; then
		sdkmanager --install "ndk;${NDK_VERSION}"
	fi


	if [ ! -f "${HOME}/rustup/rustup-init.sh" ]; then
		pushd "${HOME}"

		git clone --depth 1 https://github.com/rust-lang/rustup

		popd
	fi

	pushd "${HOME}/rustup"
	bash rustup-init.sh -y \
		--target "${RUST_TARGET}" \
		--default-toolchain "${RUST_VERSION}"
	popd

	if ! command -v cargo 1>/dev/null 2>&1; then
		. "${HOME}/.cargo/env"
	fi


	cargo install \
		cargo-ndk \
		--version "${CARGO_NDK_VERSION}" \
		--locked


	cargo install \
		cargo-expand \
		--version "${CARGO_EXPAND_VERSION}" \
		--locked
	cargo install flutter_rust_bridge_codegen \
		--version "${FLUTTER_RUST_BRIDGE_VERSION}" \
		--features "uuid" \
		--locked


	if [ ! -d "${VCPKG_ROOT}" ]; then
		pushd "${HOME}"

		git clone \
			https://github.com/Microsoft/vcpkg.git
		git clone \
			https://github.com/Microsoft/vcpkg-tool.git

		pushd vcpkg-tool

		mkdir build

		pushd build

		cmake \
			-DCMAKE_BUILD_TYPE=Release \
			-G 'Ninja' \
			-DVCPKG_DEVELOPMENT_WARNINGS=OFF \
			..

		cmake --build .

		popd

		popd

		pushd vcpkg

		git reset --hard "${VCPKG_COMMIT_ID}"

		cp -a ../vcpkg-tool/build/vcpkg vcpkg


		touch "vcpkg.disable-metrics"

		popd

		popd
	fi


	if [ "${ANDROID_ABI}" = "x86" ]; then
		if [ ! -d "${HOME}/depot_tools" ]; then
			pushd "${HOME}"

			git clone \
				--depth 1 \
				https://chromium.googlesource.com/chromium/tools/depot_tools.git

			popd
		fi
	fi


	git apply res/fdroid/patches/*.patch



	BRIDGE_LLVM_PATH="$(find /usr/lib/ -name libclang.so | head -n1)"

	if [ -z "${BRIDGE_LLVM_PATH}" ]; then
		echo 'ERROR: Can not find libclang.so for bridge generator!' >&2
		exit 1
	fi

	BRIDGE_LLVM_PATH="$(dirname "${BRIDGE_LLVM_PATH}")"
	BRIDGE_LLVM_PATH="$(dirname "${BRIDGE_LLVM_PATH}")"



	prepare_flutter "$FLUTTER_BRIDGE_VERSION" "${HOME}/flutter"


	git add .


	sed \
		-i \
		-e 's/extended_text: 14.0.0/extended_text: 13.0.0/g' \
		flutter/pubspec.yaml


	pushd flutter

	flutter clean
	flutter packages pub get --enforce-lockfile

	popd


	flutter_rust_bridge_codegen \
		--rust-input ./src/flutter_ffi.rs \
		--dart-output ./flutter/lib/generated_bridge.dart \
		--llvm-path "${BRIDGE_LLVM_PATH}"


	git add -f ./flutter/lib/generated_bridge.* ./src/bridge_generated.*


	git checkout '*'
	git clean -dffx
	git reset

	unset BRIDGE_LLVM_PATH


	prepare_flutter "${FLUTTER_VERSION}" "${HOME}/flutter"


	sed \
		-i \
		-e '/gms/d' \
		flutter/android/build.gradle \
		flutter/android/app/build.gradle


	sed \
		-i \
		-e 's/org.gradle.jvmargs=-Xmx1024M/org.gradle.jvmargs=-Xmx2g/g' \
		flutter/android/gradle.properties


	sed \
		-i \
		-e '/firebase_analytics/d' \
		flutter/pubspec.yaml

	sed \
		-i \
		-e '/  firebase/,/    version/d' \
		flutter/pubspec.lock

	sed \
		-i \
		-e '/firebase/Id' \
		flutter/lib/main.dart

	;;
build)

	set -e



	FLUTTER_VERSION="$(yq -r \
		.env.ANDROID_FLUTTER_VERSION \
		.github/workflows/flutter-build.yml)"

	if [ -z "${FLUTTER_VERSION}" ]; then
		FLUTTER_VERSION="$(yq -r \
			.env.FLUTTER_VERSION \
			.github/workflows/flutter-build.yml)"
	fi

	NDK_VERSION="$(yq -r \
		.env.NDK_VERSION \
		.github/workflows/flutter-build.yml)"

	NDK_VERSION="$(curl https://gitlab.com/fdroid/android-sdk-transparency-log/-/raw/master/signed/checksums.json |
		jq -r ".\"https://dl.google.com/android/repository/android-ndk-${NDK_VERSION}-linux.zip\"[0].\"source.properties\"" |
		sed -n -E 's/.*Pkg.Revision = ([0-9.]+).*/\1/p')"

	if [ -z "${NDK_VERSION}" ]; then
		echo "ERROR: Can not map Android NDK codename to revision!" >&2
		exit 1
	fi

	export ANDROID_NDK_HOME="${ANDROID_SDK_ROOT}/ndk/${NDK_VERSION}"
	export ANDROID_NDK_ROOT="${ANDROID_SDK_ROOT}/ndk/${NDK_VERSION}"

	if ! command -v cargo 1>/dev/null 2>&1; then
		. "${HOME}/.cargo/env"
	fi


	pushd flutter

	flutter clean
	flutter packages pub get

	popd


	bash flutter/build_android_deps.sh "${ANDROID_ABI}"


	cargo ndk \
		--platform 21 \
		--target "${RUST_TARGET}" \
		--bindgen \
		build \
		--locked \
		--release \
		--features "${SUPPORTDESK_FEATURES}"

	mkdir -p "flutter/android/app/src/main/jniLibs/${ANDROID_ABI}"

	cp "target/${RUST_TARGET}/release/liblibsupportdesk.so" \
		"flutter/android/app/src/main/jniLibs/${ANDROID_ABI}/libsupportdesk.so"

	cp "${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/${NDK_TARGET}/libc++_shared.so" \
		"flutter/android/app/src/main/jniLibs/${ANDROID_ABI}/"

	"${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip" \
		"flutter/android/app/src/main/jniLibs/${ANDROID_ABI}"/*


	if [ "${ANDROID_ABI}" = "x86" ]; then
		pushd flutter-sdk

		echo "## Sync flutter engine sources"
		echo "### We need fakeroot because chromium base image is unpacked with weird uid/gid ownership"

		sed -i "s/FLUTTER_VERSION_PLACEHOLDER/${FLUTTER_VERSION}/" .gclient

		export FAKEROOTDONTTRYCHOWN=1

		fakeroot gclient sync

		unset FAKEROOTDONTTRYCHOWN

		pushd src

		echo "## Patch away Google Play dependencies"

		rm \
			flutter/shell/platform/android/io/flutter/app/FlutterPlayStoreSplitApplication.java \
			flutter/shell/platform/android/io/flutter/embedding/engine/deferredcomponents/PlayStoreDeferredComponentManager.java flutter/shell/platform/android/io/flutter/embedding/android/FlutterPlayStoreSplitApplication.java

		sed \
			-i \
			-e '/PlayStore/d' \
			flutter/tools/android_lint/project.xml \
			flutter/shell/platform/android/BUILD.gn

		sed \
			-i \
			-e '/com.google.android.play/d' \
			flutter/tools/androidx/files.json

		echo "## Configure android engine build"

		flutter/tools/gn \
			--android --android-cpu x86 --runtime-mode=jit_release \
			--no-goma --no-enable-unittests

		echo "## Perform android engine build"

		ninja -C out/android_jit_release_x86

		echo "## Configure host engine build"

		flutter/tools/gn \
			--android-cpu x86 --runtime-mode=jit_release \
			--no-goma --no-enable-unittests

		echo "## Perform android engine build"

		ninja -C out/host_jit_release_x86

		echo "## Rename host engine"

		mv out/host_jit_release_x86 out/host_jit_release

		echo "## Mimic jit_release engine to debug to use with flutter build apk"

		pushd out/android_jit_release_x86

		sed \
			-e 's/jit_release/debug/' \
			flutter_embedding_jit_release.maven-metadata.xml \
			1>flutter_embedding_debug.maven-metadata.xml

		sed \
			-e 's/jit_release/debug/' \
			flutter_embedding_jit_release.pom \
			1>flutter_embedding_debug.pom

		sed \
			-e 's/jit_release/debug/' \
			x86_jit_release.maven-metadata.xml \
			1>x86_debug.maven-metadata.xml

		sed \
			-e 's/jit_release/debug/' \
			x86_jit_release.pom \
			1>x86_debug.pom

		cp -a \
			flutter_embedding_jit_release-sources.jar \
			flutter_embedding_debug-sources.jar

		cp -a \
			flutter_embedding_jit_release.jar \
			flutter_embedding_debug.jar

		cp -a \
			x86_jit_release.jar \
			x86_debug.jar

		popd

		popd

		popd

		echo "# Clean up intermediate engine files and show free space"

		rm -rf \
			flutter-sdk/src/out/android_jit_release_x86/obj \
			flutter-sdk/src/out/host_jit_release/obj

		mv flutter-sdk/src/out flutter-out

		rm -rf flutter-sdk

		mkdir -p flutter-sdk/src/

		mv flutter-out flutter-sdk/src/out
	fi


	pushd flutter

	if [ "${ANDROID_ABI}" = "x86" ]; then
		flutter build apk \
			--local-engine-src-path="$(readlink -mf "../flutter-sdk/src")" \
			--local-engine=android_jit_release_x86 \
			--debug \
			--build-number="${VERCODE}" \
			--build-name="${VERNAME}" \
			--target-platform "${FLUTTER_TARGET}"
	else
		flutter build apk \
			--release \
			--build-number="${VERCODE}" \
			--build-name="${VERNAME}" \
			--target-platform "${FLUTTER_TARGET}"
	fi

	popd

	rm -rf flutter-sdk


	rm -f /home/vagrant/.gitconfig

	;;
*)
	echo "ERROR: Unknown build step '${BUILDSTEP}'!" >&2
	exit 1
	;;
esac


echo "All done!"
