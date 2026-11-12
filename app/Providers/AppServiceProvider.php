<?php

namespace App\Providers;

use Illuminate\Http\Resources\Json\JsonResource;
use Illuminate\Support\ServiceProvider;

class AppServiceProvider extends ServiceProvider
{
    public function register(): void
    {
        $this->app->singleton(\App\Services\RelayBridgeService::class);
        $this->app->singleton(\App\Services\DeviceSyncService::class);
        $this->app->singleton(\App\Services\BuildArtifactStore::class);
        $this->app->singleton(\App\Services\BuildService::class);
        $this->app->singleton(\App\Services\ClientPackagingService::class);
        $this->app->singleton(\App\Services\GitHubWorkflowDispatcher::class);
        $this->app->singleton(\App\Services\TenantProvisioner::class);
    }

    public function boot(): void
    {
        JsonResource::withoutWrapping();
    }
}
