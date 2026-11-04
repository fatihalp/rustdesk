<?php

namespace App\Services;

use App\Models\ClientBuild;
use Illuminate\Support\Facades\Http;
use RuntimeException;

class GitHubWorkflowDispatcher
{
    public function isConfigured(): bool
    {
        return filled(config('panel.github.repository')) && filled(config('panel.github.token'));
    }

    public function dispatch(ClientBuild $build): void
    {
        $cfg = config('panel.github');

        $response = Http::withToken($cfg['token'])
            ->timeout(15)
            ->withHeaders(['Accept' => 'application/vnd.github+json', 'X-GitHub-Api-Version' => '2022-11-28'])
            ->post("https://api.github.com/repos/{$cfg['repository']}/actions/workflows/{$cfg['workflow']}/dispatches", [
                'ref' => $cfg['ref'],
                'inputs' => [
                    'build_id' => (string) $build->id,
                    'platform' => $build->platform,
                    'app_url' => config('panel.api_url'),
                ],
            ]);

        if (! $response->successful()) {
            throw new RuntimeException('GitHub dispatch failed: '.$response->status().' '.$response->body());
        }
    }
}
