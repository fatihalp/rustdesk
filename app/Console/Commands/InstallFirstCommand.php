<?php

namespace App\Console\Commands;

use App\Services\Installation\EnvironmentFile;
use App\Services\Installation\InstallationSettings;
use App\Services\Installation\ServerInstaller;
use App\Services\Installation\ShellRunner;
use Illuminate\Console\Command;
use Illuminate\Support\Str;
use InvalidArgumentException;

use function Laravel\Prompts\password;
use function Laravel\Prompts\text;

class InstallFirstCommand extends Command
{
    private const SECRET_LENGTH = 48;

    private const GENERATED_PASSWORD_LENGTH = 20;

    protected $signature = 'install:first
        {--domain= : Domain name or public IP address that clients and browsers use to reach this server}
        {--email= : Email for the Let\'s Encrypt certificate; leave empty to serve over plain HTTP}
        {--admin-email= : Email of the first admin user}
        {--admin-password= : Password of the first admin user; generated when empty}';

    protected $description = 'Install SupportDesk with PostgreSQL, the relay server, nginx and the firewall on a fresh Ubuntu Server 24.04';

    public function handle(ServerInstaller $installer, ShellRunner $shell, EnvironmentFile $environment): int
    {
        if ($reason = $installer->unsupportedReason()) {
            $this->components->error($reason);

            return self::FAILURE;
        }

        $environment->ensureExists(base_path('.env.example'));

        $domain = $this->askDomain();
        $certificateEmail = $this->askCertificateEmail($domain);
        $adminEmail = $this->askAdminEmail($certificateEmail);
        $adminPassword = $this->option('admin-password') ?? ($this->input->isInteractive()
            ? password('Admin password', hint: 'Leave empty to generate one')
            : '');
        $passwordGenerated = $adminPassword === '';

        $settings = new InstallationSettings(
            domain: $domain,
            certificateEmail: $certificateEmail,
            adminEmail: $adminEmail,
            adminPassword: $passwordGenerated ? Str::password(self::GENERATED_PASSWORD_LENGTH, symbols: false) : $adminPassword,
            databasePassword: $this->existingOrRandom($environment, 'DB_PASSWORD', $environment->get('DB_USERNAME') === InstallationSettings::DATABASE_USER),
            controlSecret: $this->existingOrRandom($environment, 'PANEL_RELAY_CONTROL_SECRET'),
            buildSecret: $this->existingOrRandom($environment, 'PANEL_BUILD_SECRET'),
        );

        $shell->streamTo($this->output);
        $installer->install($settings, fn (string $title) => $this->components->info($title));

        $this->printSummary($settings, $environment, $passwordGenerated);

        return self::SUCCESS;
    }

    private function askDomain(): string
    {
        $domain = $this->option('domain') ?? text(
            label: 'Domain name or public IP address of this server',
            placeholder: 'support.example.com',
            required: true,
            validate: fn (string $value) => $this->isHost($value) ? null : 'Enter a valid domain name or IP address.',
        );

        if (! $this->isHost($domain)) {
            throw new InvalidArgumentException("Invalid domain or IP address: {$domain}");
        }

        return strtolower($domain);
    }

    private function askCertificateEmail(string $domain): ?string
    {
        if (filter_var($domain, FILTER_VALIDATE_IP)) {
            return null;
        }

        $email = $this->option('email') ?? ($this->input->isInteractive()
            ? text(
                label: "Email for the Let's Encrypt certificate",
                hint: 'Leave empty to serve the panel over plain HTTP',
                validate: fn (string $value) => $value === '' || filter_var($value, FILTER_VALIDATE_EMAIL) ? null : 'Enter a valid email address.',
            )
            : '');

        return $email === '' ? null : $email;
    }

    private function askAdminEmail(?string $certificateEmail): string
    {
        return $this->option('admin-email') ?? text(
            label: 'Admin email',
            default: $certificateEmail ?? '',
            required: true,
            validate: fn (string $value) => filter_var($value, FILTER_VALIDATE_EMAIL) ? null : 'Enter a valid email address.',
        );
    }

    private function existingOrRandom(EnvironmentFile $environment, string $key, bool $reuse = true): string
    {
        $existing = $environment->get($key);

        return $reuse && filled($existing) ? $existing : Str::random(self::SECRET_LENGTH);
    }

    private function isHost(string $value): bool
    {
        return filter_var($value, FILTER_VALIDATE_IP) !== false
            || (str_contains($value, '.') && filter_var($value, FILTER_VALIDATE_DOMAIN, FILTER_FLAG_HOSTNAME) !== false);
    }

    private function printSummary(InstallationSettings $settings, EnvironmentFile $environment, bool $passwordGenerated): void
    {
        $this->newLine();
        $this->components->info('SupportDesk is installed.');

        $this->components->twoColumnDetail('Admin panel', $settings->appUrl().'/admin');
        $this->components->twoColumnDetail('Customer portal', $settings->appUrl().'/portal');
        $this->components->twoColumnDetail('Admin email', $settings->adminEmail);
        if ($passwordGenerated) {
            $this->components->twoColumnDetail('Admin password', $settings->adminPassword);
        }
        $this->components->twoColumnDetail('Relay host', $settings->domain);
        $this->components->twoColumnDetail('Relay public key', (string) $environment->get('PANEL_RELAY_KEY'));
        $this->components->twoColumnDetail('Open ports', '80, 443, 21115-21119/tcp, 21116/udp');

        if ($passwordGenerated) {
            $this->components->warn('Store the generated admin password now; it is not saved anywhere else.');
        }
    }
}
