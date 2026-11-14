<?php

namespace App\Services\Installation\Steps;

use App\Services\Installation\InstallationSettings;
use App\Services\Installation\InstallationStep;
use App\Services\Installation\ShellRunner;

class CertificateStep implements InstallationStep
{
    public function __construct(private readonly ShellRunner $shell) {}

    public function title(): string
    {
        return "Requesting a Let's Encrypt certificate and redirecting HTTP to HTTPS";
    }

    public function shouldRun(InstallationSettings $settings): bool
    {
        return $settings->usesHttps();
    }

    public function run(InstallationSettings $settings): void
    {
        $this->shell->run([
            'certbot', '--nginx', '--non-interactive', '--agree-tos', '--keep-until-expiring', '--redirect',
            '-m', $settings->certificateEmail,
            '-d', $settings->domain,
        ]);
    }
}
