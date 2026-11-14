<?php

namespace App\Services\Installation\Steps;

use App\Services\Installation\InstallationSettings;
use App\Services\Installation\InstallationStep;
use App\Services\Installation\ShellRunner;
use Throwable;

class FirewallStep implements InstallationStep
{
    private const DEFAULT_SSH_PORT = '22';

    private const PUBLIC_RULES = ['80/tcp', '443/tcp', '21115:21119/tcp', '21116/udp'];

    public function __construct(private readonly ShellRunner $shell) {}

    public function title(): string
    {
        return 'Opening web, SSH and relay ports in the firewall';
    }

    public function shouldRun(InstallationSettings $settings): bool
    {
        return true;
    }

    public function run(InstallationSettings $settings): void
    {
        foreach ($this->sshPorts() as $port) {
            $this->shell->run(['ufw', 'allow', "{$port}/tcp"]);
        }

        foreach (self::PUBLIC_RULES as $rule) {
            $this->shell->run(['ufw', 'allow', $rule]);
        }

        $this->shell->run(['ufw', '--force', 'enable']);
    }

    private function sshPorts(): array
    {
        try {
            preg_match_all('/^port (\d+)$/m', $this->shell->run(['sshd', '-T'], path: '/'), $matches);
        } catch (Throwable) {
            return [self::DEFAULT_SSH_PORT];
        }

        return $matches[1] ?: [self::DEFAULT_SSH_PORT];
    }
}
