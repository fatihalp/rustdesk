<?php

namespace App\Services;

use App\Models\ClientBuild;
use Illuminate\Support\Facades\Http;
use RuntimeException;

class GitHubWorkflowDispatcher
{
    /** Starts the branded packaging workflow for one build row. */
    public function dispatch(ClientBuild $build): void
    {
        $cfg = config('panel.github');
        if (empty($cfg['repository']) || empty($cfg['token'])) {
            throw new RuntimeException('GITHUB_REPOSITORY and GITHUB_TOKEN must be set to trigger builds.');
        }

        $response = Http::withToken($cfg['token'])
            ->withHeaders(['Accept' => 'application/vnd.github+json', 'X-GitHub-Api-Version' => '2022-11-28'])
            ->post("https://api.github.com/repos/{$cfg['repository']}/actions/workflows/{$cfg['workflow']}/dispatches", [
                'ref' => $cfg['ref'],
                'inputs' => [
                    'build_id' => (string) $build->id,
                    'platform' => $build->platform,
                    'app_url' => rtrim((string) config('panel.api_url'), '/'),
                ],
            ]);

        if (! $response->successful()) {
            throw new RuntimeException('GitHub dispatch failed: '.$response->status().' '.$response->body());
        }
    }

    public function queue(ClientBuild $build): void
    {
        try {
            $this->dispatch($build);
        } catch (\Throwable $e) {
            $build->update(['status' => ClientBuild::FAILED, 'error' => mb_substr($e->getMessage(), 0, 2000), 'finished_at' => now()]);
            throw $e;
        }
    }
}
