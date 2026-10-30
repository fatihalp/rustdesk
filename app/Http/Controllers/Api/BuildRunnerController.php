<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Models\ClientBuild;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Storage;
use Symfony\Component\HttpFoundation\StreamedResponse;

/** Endpoints called by the GitHub Actions packaging workflow. */
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

    public function artifact(Request $request, ClientBuild $build): JsonResponse
    {
        $request->validate([
            'file' => 'required|file',
            'version' => 'nullable|string|max:50',
            'signed' => 'nullable|boolean',
            'github_run_url' => 'nullable|url|max:500',
        ]);

        $file = $request->file('file');
        $disk = Storage::disk(config('panel.builds.disk'));
        $name = $this->safeName($file->getClientOriginalName());
        $path = $disk->putFileAs(
            trim(config('panel.builds.directory'), '/').'/'.$build->clientApp->slug.'/'.$build->id,
            $file,
            $name,
        );

        $build->update([
            'status' => ClientBuild::READY,
            'artifact_path' => $path,
            'artifact_name' => $name,
            'sha256' => hash_file('sha256', $file->getRealPath()),
            'size_bytes' => $file->getSize(),
            'version' => $request->input('version'),
            'signed' => $request->boolean('signed'),
            'github_run_url' => $request->input('github_run_url') ?? $build->github_run_url,
            'error' => null,
            'finished_at' => now(),
        ]);

        return response()->json(['ok' => true, 'sha256' => $build->sha256]);
    }

    public static function stream(ClientBuild $build): StreamedResponse
    {
        return Storage::disk(config('panel.builds.disk'))
            ->download($build->artifact_path, $build->artifact_name);
    }

    private function safeName(string $name): string
    {
        return preg_replace('/[^A-Za-z0-9._ -]/', '_', basename($name)) ?: 'client.bin';
    }
}
