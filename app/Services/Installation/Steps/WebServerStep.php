<?php

namespace App\Services\Installation\Steps;

use App\Services\Installation\InstallationSettings;
use App\Services\Installation\InstallationStep;
use App\Services\Installation\ShellRunner;

class WebServerStep implements InstallationStep
{
    private const SITE_FILE = '/etc/nginx/sites-available/supportdesk';

    private const ENABLED_SITE_LINK = '/etc/nginx/sites-enabled/supportdesk';

    private const DEFAULT_SITE_LINK = '/etc/nginx/sites-enabled/default';

    public function __construct(private readonly ShellRunner $shell) {}

    public function title(): string
    {
        return 'Configuring nginx and PHP-FPM';
    }

    public function shouldRun(InstallationSettings $settings): bool
    {
        return true;
    }

    public function run(InstallationSettings $settings): void
    {
        file_put_contents(self::SITE_FILE, strtr(file_get_contents(resource_path('installer/nginx.conf.stub')), [
            '{{ domain }}' => $settings->domain,
            '{{ root }}' => base_path(),
            '{{ php_fpm_service }}' => $settings->phpFpmService(),
        ]));

        if (! is_link(self::ENABLED_SITE_LINK)) {
            symlink(self::SITE_FILE, self::ENABLED_SITE_LINK);
        }
        if (is_link(self::DEFAULT_SITE_LINK)) {
            unlink(self::DEFAULT_SITE_LINK);
        }

        $this->shell->run(['nginx', '-t']);
        $this->shell->run(['systemctl', 'enable', '--now', $settings->phpFpmService()]);
        $this->shell->run(['systemctl', 'restart', $settings->phpFpmService()]);
        $this->shell->run(['systemctl', 'enable', '--now', 'nginx']);
        $this->shell->run(['systemctl', 'reload', 'nginx']);
    }
}
