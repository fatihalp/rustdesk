<?php

namespace App\Services\Installation\Steps;

use App\Services\Installation\EnvironmentFile;
use App\Services\Installation\InstallationSettings;
use App\Services\Installation\InstallationStep;
use App\Services\Installation\ShellRunner;
use Database\Seeders\DefaultAdminSeeder;

class ApplicationStep implements InstallationStep
{
    private const WEB_USER = 'www-data';

    public function __construct(
        private readonly ShellRunner $shell,
        private readonly EnvironmentFile $environment,
        private readonly RelayServerStep $relay,
    ) {}

    public function title(): string
    {
        return 'Configuring the panel, migrating the database and building the portal';
    }

    public function shouldRun(InstallationSettings $settings): bool
    {
        return true;
    }

    public function run(InstallationSettings $settings): void
    {
        $this->environment->set([
            'APP_ENV' => 'production',
            'APP_DEBUG' => 'false',
            'APP_URL' => $settings->appUrl(),
            'DB_CONNECTION' => 'pgsql',
            'DB_HOST' => '127.0.0.1',
            'DB_PORT' => '5432',
            'DB_DATABASE' => InstallationSettings::DATABASE_NAME,
            'DB_USERNAME' => InstallationSettings::DATABASE_USER,
            'DB_PASSWORD' => $settings->databasePassword,
            'SESSION_SECURE_COOKIE' => $settings->usesHttps() ? 'true' : 'false',
            'PANEL_API_URL' => $settings->appUrl(),
            'PANEL_RENDEZVOUS_SERVER' => $settings->domain,
            'PANEL_RELAY_SERVER' => $settings->domain,
            'PANEL_RELAY_KEY' => $this->relay->publicKey(),
            'PANEL_RELAY_CONTROL_URL' => 'http://127.0.0.1:21120',
            'PANEL_RELAY_CONTROL_SECRET' => $settings->controlSecret,
            'PANEL_RELAY_WS_URL' => $settings->relayEventsUrl(),
            'PANEL_BUILD_SECRET' => $settings->buildSecret,
            'PANEL_ADMIN_EMAIL' => $settings->adminEmail,
            'PANEL_ADMIN_PASSWORD' => '',
        ]);

        if (blank($this->environment->get('APP_KEY'))) {
            $this->artisan(['key:generate', '--force']);
        }

        $this->artisan(['migrate', '--force']);
        $this->artisan(['db:seed', '--class='.DefaultAdminSeeder::class, '--force'], [
            'PANEL_ADMIN_PASSWORD' => $settings->adminPassword,
        ]);

        $this->shell->run(['npm', 'install', '--no-audit', '--no-fund']);
        $this->shell->run(['npm', 'run', 'build']);
        $this->artisan(['optimize']);

        $this->grantWebServerAccess();
    }

    private function artisan(array $arguments, array $environment = []): void
    {
        $staleValues = array_fill_keys($this->environment->keys(), false);

        $this->shell->run([PHP_BINARY, 'artisan', ...$arguments, '--no-interaction'], [...$staleValues, ...$environment]);
    }

    private function grantWebServerAccess(): void
    {
        $this->shell->run(['chown', '-R', self::WEB_USER.':'.self::WEB_USER, storage_path(), base_path('bootstrap/cache')]);
        $this->shell->run(['chown', 'root:'.self::WEB_USER, $this->environment->path()]);
        $this->shell->run(['chmod', '0640', $this->environment->path()]);
    }
}
