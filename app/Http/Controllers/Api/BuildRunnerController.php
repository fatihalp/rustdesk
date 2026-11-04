<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Models\ClientBuild;
use App\Services\BuildArtifactStore;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class BuildRunnerController extends Controller
{
    public function manifest(ClientBuild $build): JsonResponse
    {
        $app = $build->clientApp;
        $build->update(['status' => ClientBuild::BUILDING, 'started_at' => $build->started_at ?? now()]);

        return response()->json([
            'platform' => $build->platform,
            'manifest' => $app->manifest(),
            'logo_url' => $app->logoUrl() ? url($app->logoUrl()) : null,
            'slug' => $app->slug,
        ]);
    }

    public function status(Request $request, ClientBuild $build): JsonResponse
    {
        $data = $request->validate([
            'status' => 'required|in:building,failed',
            'error' => 'nullable|string|max:2000',
            'github_run_url' => 'nullable|url|max:500',
        ]);

        $build->update([
            'status' => $data['status'],
            'error' => $data['error'] ?? null,
            'github_run_url' => $data['github_run_url'] ?? $build->github_run_url,
            'finished_at' => $data['status'] === ClientBuild::FAILED ? now() : null,
        ]);

        return response()->json(['ok' => true]);
    }

    public function artifact(Request $request, ClientBuild $build, BuildArtifactStore $artifacts): JsonResponse
    {
        $data = $request->validate([
            'file' => 'required|file',
            'version' => 'nullable|string|max:50',
            'signed' => 'nullable|boolean',
            'github_run_url' => 'nullable|url|max:500',
        ]);

        $file = $request->file('file');
        $artifacts->store($build, $file, $file->getClientOriginalName(), [
            'version' => $data['version'] ?? $build->version,
            'signed' => $request->boolean('signed'),
            'github_run_url' => $data['github_run_url'] ?? $build->github_run_url,
        ]);

        return response()->json(['ok' => true, 'sha256' => $build->sha256]);
    }
}
