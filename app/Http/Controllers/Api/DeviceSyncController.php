<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Services\DeviceSyncService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Http\Response;

class DeviceSyncController extends Controller
{
    private const SYSINFO_VERSION = '1';

    public function __construct(private readonly DeviceSyncService $sync) {}

    public function heartbeat(Request $request): JsonResponse
    {
        $id = (string) $request->json('id', '');
        if ($id === '') {
            return response()->json(['error' => 'id required'], 422);
        }

        $result = $this->sync->handleHeartbeat(
            $id,
            $request->json('uuid'),
            $request->json('conns'),
            $request->ip()
        );

        return response()->json($result['payload']);
    }

    public function sysinfoVersion(): Response
    {
        return $this->text(self::SYSINFO_VERSION);
    }

    public function sysinfo(Request $request): Response
    {
        $result = $this->sync->handleSysinfo($request->json()->all(), $request->ip());

        return $this->text($result['message'], $result['status']);
    }

    private function text(string $body, int $status = 200): Response
    {
        return response($body, $status, ['Content-Type' => 'text/plain']);
    }
}
