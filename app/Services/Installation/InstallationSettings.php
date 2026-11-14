<?php

namespace App\Services\Installation;

final readonly class InstallationSettings
{
    public const DATABASE_NAME = 'supportdesk';

    public const DATABASE_USER = 'supportdesk';

    public function __construct(
        public string $domain,
        public ?string $certificateEmail,
        public string $adminEmail,
        public string $adminPassword,
        public string $databasePassword,
        public string $controlSecret,
        public string $buildSecret,
    ) {}

    public function usesHttps(): bool
    {
        return $this->certificateEmail !== null;
    }

    public function appUrl(): string
    {
        return ($this->usesHttps() ? 'https' : 'http').'://'.$this->domain;
    }

    public function relayEventsUrl(): string
    {
        return ($this->usesHttps() ? 'wss' : 'ws').'://'.$this->domain.'/relay/ws';
    }

    public function phpVersion(): string
    {
        return PHP_MAJOR_VERSION.'.'.PHP_MINOR_VERSION;
    }

    public function phpFpmService(): string
    {
        return "php{$this->phpVersion()}-fpm";
    }
}
