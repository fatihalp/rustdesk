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
            str_contains($os, 'windows') => 'windows',
            str_contains($os, 'mac') || str_contains($os, 'darwin') => 'macos',
            str_contains($os, 'linux') || $os !== '' => 'linux',
            default => null,
        };
    }

    public function connectCommand(): ?string
    {
        if (! $this->unattended_password) {
            return null;
        }

        return sprintf('supportdesk --connect %s --password %s', $this->supportdesk_id, $this->unattended_password);
    }

    public function connectUri(): ?string
    {
        if (! $this->unattended_password) {
            return null;
        }

        return sprintf('supportdesk://connection/new/%s?password=%s', $this->supportdesk_id, rawurlencode($this->unattended_password));
    }
}
