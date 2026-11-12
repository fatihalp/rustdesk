<?php

namespace App\Services;

use App\Models\ClientApp;
use App\Models\ClientBuild;
use Illuminate\Http\File as HttpFile;
use Illuminate\Support\Facades\File;
use Illuminate\Support\Facades\Process;
use Illuminate\Support\Str;
use RuntimeException;
use ZipArchive;

class ClientPackagingService
{
    public function __construct(private readonly BuildArtifactStore $artifacts) {}

    public function package(ClientBuild $build): ClientBuild
    {
        $app = $build->clientApp;
        $workDir = storage_path('app/tmp_builds/'.Str::uuid());
        File::makeDirectory($workDir, 0755, true);

        try {
            $brandJson = json_encode($app->manifest(), JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES);
            [$artifact, $signed] = match ($build->platform) {
                'macos' => $this->packageMacos($app, $brandJson, $workDir),
                'windows' => [$this->packageWindows($app, $brandJson, $workDir), false],
                default => throw new RuntimeException("Unsupported platform: {$build->platform}"),
            };

            return $this->artifacts->store($build, new HttpFile($artifact), basename($artifact), [
                'signed' => $signed,
                'version' => config('panel.client.version'),
            ]);
        } finally {
            File::deleteDirectory($workDir);
        }
    }

    private function packageMacos(ClientApp $app, string $brandJson, string $workDir): array
    {
        if (PHP_OS_FAMILY !== 'Darwin') {
            throw new RuntimeException('macOS packages can only be built on a macOS host.');
        }

        $name = $this->bundleName($app);
        $binary = config('panel.client.binary_dir').'/'.config('panel.client.binary_name');
        if (! is_file($binary)) {
            $debugBinary = base_path('client/target/debug/'.config('panel.client.binary_name'));
            if (is_file($debugBinary)) {
                $binary = $debugBinary;
            }
        }
        $binary = $this->requireFile($binary);
        $root = $workDir.'/dmg';
        $bundle = "{$root}/{$name}.app";

        File::makeDirectory("{$bundle}/Contents/MacOS", 0755, true);
        File::makeDirectory("{$bundle}/Contents/Resources", 0755, true);
        File::put("{$bundle}/Contents/Info.plist", $this->infoPlist($app, $name));
        File::put("{$bundle}/Contents/Resources/brand.json", $brandJson);
        File::copy($binary, "{$bundle}/Contents/MacOS/{$name}");
        chmod("{$bundle}/Contents/MacOS/{$name}", 0755);

        $sciter = config('panel.client.sciter_library');
        if (is_file($sciter)) {
            File::copy($sciter, "{$bundle}/Contents/MacOS/libsciter.dylib");
        }

        $signed = $this->sign($bundle);

        symlink('/Applications', "{$root}/Applications");
        File::put("{$root}/README.txt", $this->readme($app, $name));

        $dmg = "{$workDir}/{$name}-macOS.dmg";
        $this->run(['hdiutil', 'create', '-volname', $app->app_name, '-srcfolder', $root, '-ov', '-format', 'UDZO', $dmg]);
        $signed = $this->sign($dmg) && $signed;

        return [$dmg, $signed];
    }

    private function packageWindows(ClientApp $app, string $brandJson, string $workDir): string
    {
        $name = $this->bundleName($app);
        $binary = config('panel.client.binary_dir').'/'.config('panel.client.binary_name').'.exe';
        if (! is_file($binary)) {
            $debugBinary = base_path('client/target/debug/'.config('panel.client.binary_name').'.exe');
            if (is_file($debugBinary)) {
                $binary = $debugBinary;
            }
        }
        $binary = $this->requireFile($binary);
        $zipPath = "{$workDir}/{$name}-Windows.zip";

        $zip = new ZipArchive;
        if ($zip->open($zipPath, ZipArchive::CREATE | ZipArchive::OVERWRITE) !== true) {
            throw new RuntimeException('Failed to create the Windows package.');
        }
        $zip->addFile($binary, "{$name}.exe");
        $zip->addFromString('brand.json', $brandJson);
        $zip->addFromString('README.txt', $this->readme($app, $name));
        $zip->close();

        return $zipPath;
    }

    private function sign(string $path): bool
    {
        $script = config('panel.client.sign_script');
        $command = is_file($script) ? [$script, $path] : ['codesign', '--force', '--deep', '--sign', '-', $path];

        return Process::timeout(600)->run($command)->successful() && is_file($script);
    }

    private function run(array $command): void
    {
        $result = Process::timeout(600)->run($command);
        if (! $result->successful()) {
            throw new RuntimeException(trim($result->errorOutput() ?: $result->output()));
        }
    }

    private function requireFile(string $path): string
    {
        if (! is_file($path)) {
            throw new RuntimeException("Client binary not found at {$path}. Build the client first.");
        }

        return $path;
    }

    private function bundleName(ClientApp $app): string
    {
        return preg_replace('/[^A-Za-z0-9_-]/', '', $app->app_name) ?: 'Client';
    }

    private function infoPlist(ClientApp $app, string $name): string
    {
        $displayName = htmlspecialchars($app->app_name, ENT_XML1);
        $version = htmlspecialchars((string) config('panel.client.version'), ENT_XML1);
        $identifier = 'com.supportdesk.'.preg_replace('/[^a-z0-9-]/', '', $app->slug);

        return <<<XML
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>{$name}</string>
    <key>CFBundleIdentifier</key>
    <string>{$identifier}</string>
    <key>CFBundleName</key>
    <string>{$displayName}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>{$version}</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.14</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
XML;
    }

    private function readme(ClientApp $app, string $name): string
    {
        return <<<TXT
{$app->app_name} - Remote Support Client

macOS: drag {$name}.app into Applications and open it.
If macOS blocks the app, open System Settings > Privacy & Security and click "Open Anyway".

Windows: extract the archive and run {$name}.exe.
TXT;
    }
}
