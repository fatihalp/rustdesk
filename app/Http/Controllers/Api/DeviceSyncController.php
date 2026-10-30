<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Models\ClientApp;
use App\Models\Device;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Http\Response;

class DeviceSyncController extends Controller
{
    private const SYSINFO_VERSION = '1';

    public function heartbeat(Request $request): JsonResponse
    {
        $data = $request->json()->all();
        $id = (string) ($data['id'] ?? '');
        if ($id === '') {
            return response()->json(['error' => 'id required'], 422);
        }
        if ($id === '0') {
            return response()->json(new \stdClass);
        }

        $device = Device::where('supportdesk_id', $id)->first();
        $uuid = $data['uuid'] ?? null;
        if (! $device || ($device->uuid !== null && $device->uuid !== $uuid)) {
            return response()->json(['sysinfo' => true]);
        }

        $conns = $data['conns'] ?? [];
        $device->forceFill([
            'last_seen_at' => now(),
            'ip_address' => $request->ip(),
            'active_connections' => is_array($conns) ? count($conns) : 0,
        ])->save();

        return response()->json(new \stdClass);
    }

    public function sysinfoVersion(): Response
    {
        return response(self::SYSINFO_VERSION, 200, ['Content-Type' => 'text/plain']);
    }

    public function sysinfo(Request $request): Response
    {
        $data = $request->json()->all();
        $id = (string) ($data['id'] ?? '');
        $token = (string) ($data['enroll_token'] ?? '');
        if ($id === '' || $id === '0' || $token === '') {
            return response('INVALID_REQUEST', 422, ['Content-Type' => 'text/plain']);
        }

        $app = ClientApp::withoutGlobalScopes()
            ->where('enroll_token', $token)
            ->where('is_active', true)
            ->first();
        if (! $app) {
            return response('INVALID_TOKEN', 403, ['Content-Type' => 'text/plain']);
        }

        $uuid = isset($data['uuid']) ? (string) $data['uuid'] : null;
        $device = Device::where('supportdesk_id', $id)->first();
        if ($device && $device->uuid !== null && $device->uuid !== $uuid) {
            return response('UUID_MISMATCH', 409, ['Content-Type' => 'text/plain']);
        }

        $device ??= new Device(['supportdesk_id' => $id, 'first_seen_at' => now()]);
        $os = $this->text($data['os'] ?? null);
        $device->fill([
            'client_app_id' => $app->id,
            'uuid' => $uuid,
            'hostname' => $this->text($data['hostname'] ?? null),
            'username' => $this->text($data['username'] ?? null),
            'os' => $os,
            'platform' => Device::platformFromOs($os),
            'cpu' => $this->text($data['cpu'] ?? null),
            'memory' => $this->text($data['memory'] ?? null),
            'client_version' => $this->text($data['version'] ?? null),
            'ip_address' => $request->ip(),
            'last_seen_at' => now(),
        ]);

        $password = $data['unattended_password'] ?? null;
        if (is_string($password) && $password !== '' && strlen($password) <= 128) {
            $device->unattended_password = $password;
        }
        $device->save();

        return response('SYSINFO_UPDATED', 200, ['Content-Type' => 'text/plain']);
    }

    private function text(mixed $value): ?string
    {
        return is_scalar($value) ? mb_substr((string) $value, 0, 255) : null;
    }
}
