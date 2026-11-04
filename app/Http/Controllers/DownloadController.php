<?php

namespace App\Http\Controllers;

use App\Models\ClientApp;
use App\Models\ClientBuild;
use App\Models\DownloadLink;
use App\Services\BuildArtifactStore;
use Illuminate\Contracts\View\View;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\StreamedResponse;

class DownloadController extends Controller
{
    private const LABELS = ['macos' => 'macOS', 'windows' => 'Windows'];

    public function __construct(private readonly BuildArtifactStore $artifacts) {}

    public function short(string $slug): RedirectResponse
    {
        $app = ClientApp::where('slug', $slug)->where('is_active', true)->firstOrFail();

        return redirect()->route('download.show', $app->defaultDownloadLink()->token);
    }

    public function show(Request $request, string $token): View
    {
        $link = DownloadLink::with('clientApp')->where('token', $token)->firstOrFail();
        $app = $link->clientApp;
        $usable = $link->isUsable();
        $detected = $this->detectPlatform($request->userAgent());

        $builds = collect(self::LABELS)
            ->sortKeysUsing(fn ($a, $b) => ($b === $detected) <=> ($a === $detected))
            ->filter(fn ($label, $platform) => $usable && $app->latestReadyBuild($platform))
            ->all();

        return view('download', compact('app', 'link', 'usable', 'builds', 'detected'));
    }

    public function download(string $token, string $platform): StreamedResponse
    {
        abort_unless(isset(self::LABELS[$platform]), 404);

        $link = DownloadLink::with('clientApp')->where('token', $token)->firstOrFail();
        abort_unless($link->isUsable(), 404);

        $build = $link->clientApp->latestReadyBuild($platform);
        abort_unless($build && $link->claimDownload(), 404);

        return $this->artifacts->download($build);
    }

    public function adminDownload(ClientBuild $build): StreamedResponse
    {
        return $this->artifacts->download($build);
    }

    private function detectPlatform(?string $userAgent): string
    {
        $ua = strtolower((string) $userAgent);

        return str_contains($ua, 'mac os') || str_contains($ua, 'macintosh') ? 'macos' : 'windows';
    }
}
