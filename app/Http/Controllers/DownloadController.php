<?php

namespace App\Http\Controllers;

use App\Http\Controllers\Api\BuildRunnerController;
use App\Models\DownloadLink;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

class DownloadController extends Controller
{
    private const PLATFORMS = ['windows', 'macos'];

    public function show(Request $request, string $token)
    {
        $link = DownloadLink::with('clientApp')->where('token', $token)->first();
        abort_if(! $link, 404);

        $app = $link->clientApp;
        $usable = $link->isUsable();

        return view('download', [
            'app' => $app,
            'usable' => $usable,
            'link' => $link,
            'detected' => $this->detectPlatform($request->userAgent()),
            'builds' => collect(self::PLATFORMS)->mapWithKeys(
                fn ($p) => [$p => $usable ? $app->latestReadyBuild($p) : null]
            ),
        ]);
    }

    public function download(string $token, string $platform)
    {
        abort_unless(in_array($platform, self::PLATFORMS, true), 404);

        $link = DownloadLink::with('clientApp')->where('token', $token)->first();
        abort_if(! $link || ! $link->isUsable(), 404);

        $build = $link->clientApp->latestReadyBuild($platform);
        abort_if(! $build, 404);

        $claimed = DB::table('download_links')
            ->where('id', $link->id)
            ->where(fn ($q) => $q->whereNull('max_downloads')->orWhereColumn('download_count', '<', 'max_downloads'))
            ->update(['download_count' => DB::raw('download_count + 1'), 'last_downloaded_at' => now()]);
        abort_if($claimed === 0, 404);

        return BuildRunnerController::stream($build);
    }

    private function detectPlatform(?string $ua): string
    {
        $ua = strtolower((string) $ua);

        return match (true) {
            str_contains($ua, 'windows') => 'windows',
            str_contains($ua, 'mac os') || str_contains($ua, 'macintosh') => 'macos',
            default => 'windows',
        };
    }
}
