<?php

namespace App\Services\Installation;

use App\Services\Installation\Steps\ApplicationStep;
use App\Services\Installation\Steps\CertificateStep;
use App\Services\Installation\Steps\DatabaseStep;
use App\Services\Installation\Steps\FirewallStep;
use App\Services\Installation\Steps\RelayServerStep;
use App\Services\Installation\Steps\SystemPackagesStep;
use App\Services\Installation\Steps\WebServerStep;

class ServerInstaller
{
    private const SUPPORTED_SYSTEM = ['ID' => 'ubuntu', 'VERSION_ID' => '24.04'];

    private readonly array $steps;

    public function __construct(
        SystemPackagesStep $packages,
        DatabaseStep $database,
        RelayServerStep $relay,
        ApplicationStep $application,
        WebServerStep $webServer,
        FirewallStep $firewall,
        CertificateStep $certificate,
    ) {
        $this->steps = [$packages, $database, $relay, $application, $webServer, $firewall, $certificate];
    }

    public function unsupportedReason(): ?string
    {
        if (! function_exists('posix_geteuid') || posix_geteuid() !== 0) {
            return 'Run this command as root, for example with sudo.';
        }

        $release = is_readable('/etc/os-release') ? parse_ini_file('/etc/os-release') : [];
        foreach (self::SUPPORTED_SYSTEM as $key => $expected) {
            if (($release[$key] ?? null) !== $expected) {
                return 'This installer supports Ubuntu Server 24.04 only.';
            }
        }

        return null;
    }

    public function install(InstallationSettings $settings, callable $onStep): void
    {
        foreach ($this->steps as $step) {
            if ($step->shouldRun($settings)) {
                $onStep($step->title());
                $step->run($settings);
            }
        }
    }
}
