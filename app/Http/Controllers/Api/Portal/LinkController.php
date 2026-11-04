<?php

namespace App\Http\Controllers\Api\Portal;

use App\Http\Controllers\Controller;
use App\Http\Resources\DownloadLinkResource;
use App\Models\DownloadLink;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class LinkController extends Controller
{
    public function index(Request $request): JsonResponse
    {
        $app = $request->user()->customer->defaultClientApp();
        $links = $app->downloadLinks()->latest('id')->get()->each->setRelation('clientApp', $app);

        return response()->json([
            'short_url' => $app->shortUrl(),
            'links' => DownloadLinkResource::collection($links),
        ]);
    }

    public function store(Request $request): JsonResponse
    {
        $data = $request->validate([
            'label' => 'nullable|string|max:100',
            'max_downloads' => 'nullable|integer|min:1',
            'expires_days' => 'nullable|integer|min:1|max:365',
        ]);

        $link = $request->user()->customer->defaultClientApp()->downloadLinks()->create([
            'label' => $data['label'] ?? 'Customer Download Link',
            'max_downloads' => $data['max_downloads'] ?? null,
            'expires_at' => isset($data['expires_days']) ? now()->addDays($data['expires_days']) : null,
        ]);

        return response()->json(['ok' => true, 'link' => new DownloadLinkResource($link)]);
    }

    public function destroy(Request $request, DownloadLink $link): JsonResponse
    {
        abort_unless($request->user()->customer->ownsClientApp($link->client_app_id), 403);

        $link->delete();

        return response()->json(['ok' => true]);
    }
}
