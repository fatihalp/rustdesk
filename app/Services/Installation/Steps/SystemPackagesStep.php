<?php

namespace App\Services\Installation\Steps;

use App\Services\Installation\InstallationSettings;
use App\Services\Installation\InstallationStep;
use App\Services\Installation\ShellRunner;

class SystemPackagesStep implements InstallationStep
{
    private const PHP_EXTENSIONS = ['fpm', 'cli', 'pgsql', 'sqlite3', 'mbstring', 'xml', 'curl', 'zip', 'bcmath', 'intl', 'gd'];

    private const PACKAGES = [
        'nginx', 'postgresql', 'nodejs', 'npm', 'git', 'curl', 'unzip', 'build-essential', 'pkg-config',
        'ufw', 'certbot', 'python3-certbot-nginx',
    ];

    public function __construct(private readonly ShellRunner $shell) {}

    public static function cargoBinary(): string
    {
        return (getenv('HOME') ?: '/root').'/.cargo/bin/cargo';
    }

    public function title(): string
    {
        return 'Installing system packages and the Rust toolchain';
    }

    public function shouldRun(InstallationSettings $settings): bool
    {
        return true;
    }

    public function run(InstallationSettings $settings): void
    {
        $environment = ['DEBIAN_FRONTEND' => 'noninteractive'];
        $phpPackages = array_map(fn (string $extension) => "php{$settings->phpVersion()}-{$extension}", self::PHP_EXTENSIONS);

        $this->shell->run(['apt-get', 'update'], $environment);
        $this->shell->run(['apt-get', 'install', '-y', ...self::PACKAGES, ...$phpPackages], $environment);

        if (! is_file(self::cargoBinary())) {
            $this->shell->run(['bash', '-c', 'curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal']);
        }
    }
}
