<?php

namespace App\Http\Controllers\Api\Portal;

use App\Http\Controllers\Controller;
use App\Http\Resources\ClientBuildResource;
use App\Services\BuildService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Validation\Rule;
use Throwable;

class BuildController extends Controller
{
    public function index(Request $request): JsonResponse
    {
        $builds = $request->user()->customer->defaultClientApp()->builds()->latest('id')->take(20)->get();

        return response()->json(['builds' => ClientBuildResource::collection($builds)]);
    }

    public function trigger(Request $request, BuildService $builds): JsonResponse
    {
        $data = $request->validate([
            'platform' => ['required', Rule::in(config('panel.builds.platforms'))],
        ]);

        try {
            $build = $builds->start($request->user()->customer->defaultClientApp(), $data['platform']);
        } catch (Throwable $e) {
            report($e);

            return response()->json(['ok' => false, 'message' => $e->getMessage()], 500);
        }

        return response()->json(['ok' => true, 'build' => new ClientBuildResource($build)]);
    }
}
