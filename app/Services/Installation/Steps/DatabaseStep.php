<?php

namespace App\Services\Installation\Steps;

use App\Services\Installation\InstallationSettings;
use App\Services\Installation\InstallationStep;
use App\Services\Installation\ShellRunner;

class DatabaseStep implements InstallationStep
{
    public function __construct(private readonly ShellRunner $shell) {}

    public function title(): string
    {
        return 'Creating the PostgreSQL database and user';
    }

    public function shouldRun(InstallationSettings $settings): bool
    {
        return true;
    }

    public function run(InstallationSettings $settings): void
    {
        $this->shell->run(['systemctl', 'enable', '--now', 'postgresql']);

        $user = InstallationSettings::DATABASE_USER;
        $database = InstallationSettings::DATABASE_NAME;
        $password = str_replace("'", "''", $settings->databasePassword);

        $roleExists = $this->query("SELECT 1 FROM pg_roles WHERE rolname = '{$user}'") === '1';
        $this->query(($roleExists ? 'ALTER' : 'CREATE')." ROLE \"{$user}\" WITH LOGIN PASSWORD '{$password}'");

        if ($this->query("SELECT 1 FROM pg_database WHERE datname = '{$database}'") !== '1') {
            $this->query("CREATE DATABASE \"{$database}\" OWNER \"{$user}\"");
        }
    }

    private function query(string $sql): string
    {
        return $this->shell->run(['runuser', '-u', 'postgres', '--', 'psql', '-v', 'ON_ERROR_STOP=1', '-tAc', $sql], path: '/tmp');
    }
}
