<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Database\Eloquent\Relations\HasManyThrough;

class Customer extends Model
{
    protected $fillable = ['name', 'company', 'email', 'phone', 'notes'];

    public function clientApps(): HasMany
    {
        return $this->hasMany(ClientApp::class);
    }

    public function devices(): HasManyThrough
    {
        return $this->hasManyThrough(Device::class, ClientApp::class);
    }
}
