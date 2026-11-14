<?php

namespace App\Providers;

use App\Services\Installation\EnvironmentFile;
use App\Services\Installation\ShellRunner;
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
        $this->app->singleton(ShellRunner::class);
        $this->app->singleton(EnvironmentFile::class, fn () => new EnvironmentFile(base_path('.env')));
    }

    public function boot(): void
    {
        JsonResource::withoutWrapping();
    }
}
