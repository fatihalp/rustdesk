<?php

namespace App\Services\Installation\Steps;

use App\Services\Installation\InstallationSettings;
use App\Services\Installation\InstallationStep;
use App\Services\Installation\ShellRunner;
use RuntimeException;

class RelayServerStep implements InstallationStep
{
    private const SERVICE = 'relay-server';

    private const SERVICE_USER = 'relay';

    private const BINARY = '/usr/local/bin/relay-server';

    private const ENVIRONMENT_FILE = '/etc/relay-server.env';

    private const UNIT_FILE = '/etc/systemd/system/relay-server.service';

    private const DATA_DIRECTORY = '/var/lib/relay-server';

    private const PUBLIC_KEY_FILE = self::DATA_DIRECTORY.'/id_ed25519.pub';

    private const PUBLIC_KEY_WAIT_ATTEMPTS = 40;

    public function __construct(private readonly ShellRunner $shell) {}

    public function title(): string
    {
        return 'Building and starting the relay server';
    }

    public function shouldRun(InstallationSettings $settings): bool
    {
        return true;
    }

    public function run(InstallationSettings $settings): void
    {
        $source = base_path('RelayServer');
        $this->shell->run([SystemPackagesStep::cargoBinary(), 'build', '--release', '--locked'], path: $source);
        $this->shell->run(['install', '-m', '0755', "{$source}/target/release/relay-server", self::BINARY]);

        if (! $this->shell->succeeds(['id', self::SERVICE_USER])) {
            $this->shell->run(['useradd', '--system', '--no-create-home', '--shell', '/usr/sbin/nologin', self::SERVICE_USER]);
        }

        $this->writeEnvironment($settings);
        file_put_contents(self::UNIT_FILE, file_get_contents(resource_path('installer/relay-server.service.stub')));

        $this->shell->run(['systemctl', 'daemon-reload']);
        $this->shell->run(['systemctl', 'enable', self::SERVICE]);
        $this->shell->run(['systemctl', 'restart', self::SERVICE]);
        $this->waitForPublicKey();
    }

    public function publicKey(): string
    {
        return trim((string) file_get_contents(self::PUBLIC_KEY_FILE));
    }

    private function writeEnvironment(InstallationSettings $settings): void
    {
        $lines = [
            'RELAY_HOST' => $settings->domain,
            'DATA_DIR' => self::DATA_DIRECTORY,
            'CONTROL_SECRET' => $settings->controlSecret,
            'ENCRYPTED_ONLY' => 'N',
            'ALWAYS_USE_RELAY' => 'N',
        ];

        file_put_contents(self::ENVIRONMENT_FILE, collect($lines)->map(fn ($value, $key) => "{$key}={$value}")->implode("\n")."\n");
        chmod(self::ENVIRONMENT_FILE, 0640);
        chgrp(self::ENVIRONMENT_FILE, self::SERVICE_USER);
    }

    private function waitForPublicKey(): void
    {
        for ($attempt = 0; $attempt < self::PUBLIC_KEY_WAIT_ATTEMPTS; $attempt++) {
            clearstatcache();
            if (is_file(self::PUBLIC_KEY_FILE) && $this->publicKey() !== '') {
                return;
            }
            usleep(250_000);
        }

        throw new RuntimeException('The relay server did not create its key pair. Check it with: journalctl -u '.self::SERVICE);
    }
}
