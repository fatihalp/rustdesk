<?php

namespace App\Http\Controllers\Api\Portal;

use App\Http\Controllers\Controller;
use App\Http\Resources\DeviceConnectionResource;
use App\Http\Resources\DeviceResource;
use App\Models\Device;
use App\Services\RelayBridgeService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Validation\Rule;

class DeviceController extends Controller
{
    public function __construct(private readonly RelayBridgeService $relay) {}

    public function index(Request $request): JsonResponse
    {
        $devices = $request->user()->customer->devices()->latest('last_seen_at')->get();
        $status = $this->relay->status();

        return response()->json([
            'relay' => [
                'reachable' => $status['reachable'],
                'active_sessions' => count($status['sessions']),
            ],
            'devices' => DeviceResource::collection($devices),
        ]);
    }

    public function connectInfo(Request $request, Device $device): DeviceConnectionResource
    {
        $this->authorizeDevice($request, $device);

        return new DeviceConnectionResource($device);
    }

    public function update(Request $request, Device $device): JsonResponse
    {
        $this->authorizeDevice($request, $device);

        $device->update($request->validate([
            'hostname' => ['nullable', 'string', 'max:255'],
            'notes' => ['nullable', 'string', 'max:1000'],
        ]));

        return response()->json(['ok' => true, 'device' => new DeviceResource($device)]);
    }

    public function action(Request $request, Device $device): JsonResponse
    {
        $this->authorizeDevice($request, $device);

        $action = $request->validate(['action' => ['required', Rule::in(RelayBridgeService::ACTIONS)]])['action'];

        return response()->json([
            'ok' => $this->relay->command($device->supportdesk_id, $action),
            'action' => $action,
        ]);
    }

    public function destroy(Request $request, Device $device): JsonResponse
    {
        $this->authorizeDevice($request, $device);

        $this->relay->command($device->supportdesk_id, 'disconnect');
        $device->delete();

        return response()->json(['ok' => true]);
    }

    private function authorizeDevice(Request $request, Device $device): void
    {
        abort_unless(
            $device->client_app_id && $request->user()->customer->ownsClientApp($device->client_app_id),
            403,
            'Device does not belong to your account.',
        );
    }
}
