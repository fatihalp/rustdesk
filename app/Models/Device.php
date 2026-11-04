<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class Device extends Model
{
    protected $fillable = [
        'client_app_id', 'supportdesk_id', 'uuid', 'hostname', 'username', 'os',
        'platform', 'cpu', 'memory', 'client_version', 'ip_address',
        'unattended_password', 'active_connections', 'notes',
        'first_seen_at', 'last_seen_at',
    ];

    protected $hidden = ['unattended_password'];

    protected function casts(): array
    {
        return [
            'unattended_password' => 'encrypted',
            'active_connections' => 'integer',
            'first_seen_at' => 'datetime',
            'last_seen_at' => 'datetime',
        ];
    }

    public function clientApp(): BelongsTo
    {
        return $this->belongsTo(ClientApp::class);
    }

    public function isOnline(): bool
    {
        return $this->last_seen_at !== null
            && $this->last_seen_at->gt(now()->subSeconds(config('panel.online_seconds')));
    }

    public function scopeOnline(Builder $query): Builder
    {
        return $query->where('last_seen_at', '>', now()->subSeconds(config('panel.online_seconds')));
    }

    public static function platformFromOs(?string $os): ?string
    {
        $os = strtolower((string) $os);

        return match (true) {
            $os === '' => null,
            str_contains($os, 'windows') => 'windows',
            str_contains($os, 'mac') || str_contains($os, 'darwin') => 'macos',
            default => 'linux',
        };
    }

    public function hasUnattendedPassword(): bool
    {
        $raw = $this->getRawOriginal('unattended_password');

        return is_string($raw) && $raw !== '';
    }

    public function connectUri(): ?string
    {
        if (! $this->unattended_password) {
            return null;
        }

        return sprintf(
            '%s://connection/new/%s?password=%s',
            config('panel.support_url_scheme'),
            $this->supportdesk_id,
            rawurlencode($this->unattended_password),
        );
    }

    public function connectCommand(): ?string
    {
        if (! $this->unattended_password) {
            return null;
        }

        return sprintf(
            '%s --connect %s --password %s',
            config('panel.support_url_scheme'),
            escapeshellarg($this->supportdesk_id),
            escapeshellarg($this->unattended_password),
        );
    }
}
