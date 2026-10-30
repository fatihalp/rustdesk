<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Support\Facades\Storage;
use Illuminate\Support\Str;

class ClientApp extends Model
{
    protected $fillable = [
        'customer_id', 'app_name', 'slug', 'logo_path', 'enroll_token',
        'is_active', 'unattended', 'notes',
    ];

    protected $hidden = ['enroll_token'];

    protected function casts(): array
    {
        return [
            'is_active' => 'boolean',
            'unattended' => 'boolean',
        ];
    }

    protected static function booted(): void
    {
        static::creating(function (ClientApp $app) {
            $app->enroll_token ??= Str::random(48);
            $app->slug = $app->slug ?: static::uniqueSlug($app->app_name);
        });
    }

    public static function uniqueSlug(string $name): string
    {
        $base = Str::slug($name) ?: 'client';
        $slug = $base;
        $i = 2;
        while (static::where('slug', $slug)->exists()) {
            $slug = $base.'-'.$i++;
        }

        return $slug;
    }

    public function customer(): BelongsTo
    {
        return $this->belongsTo(Customer::class);
    }

    public function builds(): HasMany
    {
        return $this->hasMany(ClientBuild::class);
    }

    public function downloadLinks(): HasMany
    {
        return $this->hasMany(DownloadLink::class);
    }

    public function devices(): HasMany
    {
        return $this->hasMany(Device::class);
    }

    public function latestReadyBuild(string $platform): ?ClientBuild
    {
        return $this->builds()
            ->where('platform', $platform)
            ->where('status', ClientBuild::READY)
            ->latest('finished_at')
            ->first();
    }

    public function logoUrl(): ?string
    {
        return $this->logo_path ? Storage::disk('public')->url($this->logo_path) : null;
    }

    public function manifest(): array
    {
        $relay = config('panel.relay');

        return [
            'app_name' => $this->app_name,
            'rendezvous_server' => $relay['rendezvous_server'],
            'relay_server' => $relay['relay_server'],
            'key' => $relay['key'],
            'api_server' => config('panel.api_url'),
            'enroll_token' => $this->enroll_token,
            'unattended' => $this->unattended,
        ];
    }
}
