<?php

namespace App\Services;

use App\Models\ClientApp;
use App\Models\ClientBuild;
use InvalidArgumentException;
use Throwable;

class BuildService
{
    public function __construct(
        private readonly GitHubWorkflowDispatcher $github,
        private readonly ClientPackagingService $packager,
    ) {}

    public function start(ClientApp $app, string $platform): ClientBuild
    {
        if (! in_array($platform, config('panel.builds.platforms'), true)) {
            throw new InvalidArgumentException("Unsupported platform: {$platform}");
        }

        $build = $app->builds()->create([
            'platform' => $platform,
            'status' => ClientBuild::PENDING,
            'version' => config('panel.client.version'),
            'started_at' => now(),
        ]);

        try {
            if ($this->github->isConfigured()) {
                $this->github->dispatch($build);
            } else {
                $build->update(['status' => ClientBuild::BUILDING]);
                $this->packager->package($build);
            }
        } catch (Throwable $e) {
            $build->markFailed($e->getMessage());
            throw $e;
        }

        $app->defaultDownloadLink();

        return $build->refresh();
    }
}
