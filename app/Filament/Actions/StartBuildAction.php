<?php

namespace App\Filament\Actions;

use App\Models\ClientApp;
use App\Services\BuildService;
use App\Services\GitHubWorkflowDispatcher;
use Filament\Notifications\Notification;
use Throwable;

class StartBuildAction
{
    private const LABELS = ['macos' => 'macOS', 'windows' => 'Windows'];

    public static function run(ClientApp $app, string $platform): void
    {
        try {
            app(BuildService::class)->start($app, $platform);
        } catch (Throwable $e) {
            Notification::make()->title('Build failed')->body($e->getMessage())->danger()->send();

            return;
        }

        $queued = app(GitHubWorkflowDispatcher::class)->isConfigured();
        Notification::make()
            ->title($queued ? 'Build queued' : self::LABELS[$platform].' build completed')
            ->success()
            ->send();
    }
}
