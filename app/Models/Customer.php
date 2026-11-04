<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Database\Eloquent\Relations\HasManyThrough;

class Customer extends Model
{
    use HasFactory;

    protected $fillable = ['name', 'company', 'email', 'phone', 'notes'];

    public function users(): HasMany
    {
        return $this->hasMany(User::class);
    }

    public function clientApps(): HasMany
    {
        return $this->hasMany(ClientApp::class);
    }

    public function devices(): HasManyThrough
    {
        return $this->hasManyThrough(Device::class, ClientApp::class);
    }

    public function defaultClientApp(): ClientApp
    {
        return $this->clientApps()->oldest('id')->first()
            ?? $this->clientApps()->create([
                'app_name' => $this->company ?: $this->name.' Support',
                'unattended' => true,
                'is_active' => true,
            ]);
    }

    public function ownsClientApp(int $clientAppId): bool
    {
        return $this->clientApps()->whereKey($clientAppId)->exists();
    }
}
