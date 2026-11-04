<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

class DownloadLink extends Model
{
    public const DEFAULT_LABEL = 'Default Download Link';

    protected $fillable = [
        'client_app_id', 'token', 'label', 'expires_at', 'max_downloads',
        'download_count', 'last_downloaded_at', 'revoked_at',
    ];

    protected function casts(): array
    {
        return [
            'expires_at' => 'datetime',
            'last_downloaded_at' => 'datetime',
            'revoked_at' => 'datetime',
        ];
    }

    protected static function booted(): void
    {
        static::creating(function (DownloadLink $link) {
            $link->token ??= Str::random(40);
        });
    }

    public function clientApp(): BelongsTo
    {
        return $this->belongsTo(ClientApp::class);
    }

    public function url(): string
    {
        return route('download.show', $this->token);
    }

    public function isUsable(): bool
    {
        return $this->revoked_at === null
            && ($this->expires_at === null || $this->expires_at->isFuture())
            && ($this->max_downloads === null || $this->download_count < $this->max_downloads)
            && ($this->clientApp?->is_active ?? false);
    }

    public function claimDownload(): bool
    {
        return static::whereKey($this->id)
            ->where(fn ($q) => $q->whereNull('max_downloads')->orWhereColumn('download_count', '<', 'max_downloads'))
            ->update(['download_count' => DB::raw('download_count + 1'), 'last_downloaded_at' => now()]) > 0;
    }
}
