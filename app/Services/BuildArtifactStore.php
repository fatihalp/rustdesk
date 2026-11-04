<?php

namespace App\Services;

use App\Models\ClientBuild;
use Illuminate\Contracts\Filesystem\Filesystem;
use Illuminate\Http\File;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;
use Symfony\Component\HttpFoundation\StreamedResponse;

class BuildArtifactStore
{
    public function store(ClientBuild $build, File|UploadedFile $file, string $name, array $attributes = []): ClientBuild
    {
        $name = $this->safeName($name);
        $directory = trim(config('panel.builds.directory'), '/').'/'.$build->clientApp->slug.'/'.$build->id;
        $path = $this->disk()->putFileAs($directory, $file, $name);

        $build->update($attributes + [
            'status' => ClientBuild::READY,
            'artifact_path' => $path,
            'artifact_name' => $name,
            'sha256' => hash_file('sha256', $file->getRealPath()),
            'size_bytes' => $file->getSize(),
            'error' => null,
            'finished_at' => now(),
        ]);

        return $build;
    }

    public function download(ClientBuild $build): StreamedResponse
    {
        abort_unless($build->isDownloadable() && $this->disk()->exists($build->artifact_path), 404);

        return $this->disk()->download($build->artifact_path, $build->artifact_name);
    }

    private function disk(): Filesystem
    {
        return Storage::disk(config('panel.builds.disk'));
    }

    private function safeName(string $name): string
    {
        return preg_replace('/[^A-Za-z0-9._ -]/', '_', basename($name)) ?: 'client.bin';
    }
}
